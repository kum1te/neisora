//! Desktop actions are dispatched on the UI thread, without invoking a shell.
use tauri::AppHandle;
pub async fn copy(app: &AppHandle, text: String) -> Result<(), String> {
    let (send, receive) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = send.send(copy_text(&handle, &text));
    })
    .map_err(|_| "Не удалось обратиться к буферу обмена")?;
    receive.await.map_err(|_| "Окно приложения закрыто")?
}
pub async fn open(app: &AppHandle, url: String) -> Result<(), String> {
    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = send.send(open_url(&url));
    })
    .map_err(|_| "Не удалось открыть браузер")?;
    receive.await.map_err(|_| "Окно приложения закрыто")?
}
#[cfg(target_os = "linux")]
fn copy_text(_app: &AppHandle, text: &str) -> Result<(), String> {
    let display = gtk::gdk::Display::default().ok_or("Нет доступа к буферу обмена")?;
    let clipboard = gtk::Clipboard::default(&display).ok_or("Буфер обмена недоступен")?;
    clipboard.set_text(text);
    clipboard.store();
    Ok(())
}
#[cfg(target_os = "linux")]
fn open_url(url: &str) -> Result<(), String> {
    gtk::show_uri_on_window(None::<&gtk::Window>, url, 0)
        .map_err(|_| "Не удалось открыть браузер по умолчанию".into())
}
#[cfg(windows)]
fn copy_text(_app: &AppHandle, text: &str) -> Result<(), String> {
    use tauri::Manager;
    use windows_sys::Win32::{
        Foundation::GlobalFree,
        System::{
            DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData},
            Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE},
            Ole::CF_UNICODETEXT,
        },
    };
    let window = _app
        .get_webview_window("main")
        .ok_or("Окно приложения закрыто")?;
    let owner = window.hwnd().map_err(|_| "Окно приложения закрыто")?.0;
    // Windows takes ownership of movable global memory only after SetClipboardData succeeds.
    let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2);
        if memory.is_null() {
            return Err("Не удалось подготовить буфер обмена".into());
        }
        let data = GlobalLock(memory);
        if data.is_null() {
            GlobalFree(memory);
            return Err("Буфер обмена недоступен".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), data.cast::<u16>(), wide.len());
        GlobalUnlock(memory);
        if OpenClipboard(owner as _) == 0 {
            GlobalFree(memory);
            return Err("Буфер обмена занят другим приложением".into());
        }
        let ok =
            EmptyClipboard() != 0 && !SetClipboardData(CF_UNICODETEXT as u32, memory).is_null();
        CloseClipboard();
        if !ok {
            GlobalFree(memory);
            return Err("Не удалось скопировать текст".into());
        }
    }
    Ok(())
}
#[cfg(windows)]
fn open_url(url: &str) -> Result<(), String> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let wide: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        Err("Не удалось открыть браузер по умолчанию".into())
    } else {
        Ok(())
    }
}
#[cfg(not(any(target_os = "linux", windows)))]
fn copy_text(_app: &AppHandle, _text: &str) -> Result<(), String> {
    Err("Буфер обмена пока не поддерживается на этой платформе".into())
}
#[cfg(not(any(target_os = "linux", windows)))]
fn open_url(_url: &str) -> Result<(), String> {
    Err("Открытие браузера пока не поддерживается на этой платформе".into())
}
