//! Exclusive profile lock plus a local-only activation pipe accepting one fixed byte.
use sha1::{Digest, Sha1};
use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::Path,
};
use tauri::Manager;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions},
};
use windows_sys::Win32::{
    Foundation::ERROR_LOCK_VIOLATION,
    Storage::FileSystem::{
        GetFileInformationByHandle, LockFileEx, BY_HANDLE_FILE_INFORMATION,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        FILE_SHARE_WRITE, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    },
    System::IO::OVERLAPPED,
};
pub struct ProfileLock {
    _file: File,
}
impl ProfileLock {
    pub fn acquire(directory: &Path) -> io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
            .open(directory.join("instance.lock"))?;
        let metadata = file.metadata()?;
        let mut info = unsafe { std::mem::zeroed::<BY_HANDLE_FILE_INFORMATION>() };
        if !metadata.is_file()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            || unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0
            || info.nNumberOfLinks != 1
        {
            return Err(io::Error::other("Небезопасный файл блокировки профиля"));
        }
        let mut overlapped = unsafe { std::mem::zeroed::<OVERLAPPED>() };
        if unsafe {
            LockFileEx(
                file.as_raw_handle(),
                LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                0,
                1,
                0,
                &mut overlapped,
            )
        } == 0
        {
            let error = io::Error::last_os_error();
            return Err(
                if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
                    io::Error::new(io::ErrorKind::WouldBlock, "Профиль уже открыт")
                } else {
                    error
                },
            );
        }
        Ok(Self { _file: file })
    }
}
pub struct Instance {
    _lock: ProfileLock,
    pipe: Option<NamedPipeServer>,
}
impl Instance {
    pub fn acquire(directory: &Path) -> io::Result<Option<Self>> {
        let name = format!(
            r"\\.\pipe\neisora-activate-{:x}",
            Sha1::digest(directory.as_os_str().as_encoded_bytes())
        );
        match ProfileLock::acquire(directory) {
            Ok(lock) => {
                let pipe = tauri::async_runtime::block_on(async {
                    ServerOptions::new()
                        .first_pipe_instance(true)
                        .reject_remote_clients(true)
                        .max_instances(1)
                        .create(&name)
                })?;
                Ok(Some(Self {
                    _lock: lock,
                    pipe: Some(pipe),
                }))
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                tauri::async_runtime::block_on(async {
                    for _ in 0..20 {
                        if let Ok(mut client) = ClientOptions::new().open(&name) {
                            client.write_all(&[1]).await?;
                            return Ok(None);
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
                    }
                    Err(io::Error::other(
                        "neisora уже запущена, но окно пока не отвечает",
                    ))
                })
            }
            Err(e) => Err(e),
        }
    }
    pub fn listen(&mut self, app: tauri::AppHandle) {
        let mut pipe = self.pipe.take().expect("Instance listener already started");
        tauri::async_runtime::spawn(async move {
            while pipe.connect().await.is_ok() {
                let mut message = [0; 2];
                if matches!(
                    tokio::time::timeout(
                        std::time::Duration::from_millis(250),
                        pipe.read(&mut message)
                    )
                    .await,
                    Ok(Ok(1))
                ) && message[0] == 1
                {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                }
                if pipe.disconnect().is_err() {
                    break;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_lock_is_exclusive_reusable_and_refuses_hardlinks() {
        let root =
            std::env::temp_dir().join(format!("neisora-windows-lock-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        crate::platform::private_directory(&root).unwrap();
        let lock = ProfileLock::acquire(&root).unwrap();
        assert!(
            matches!(ProfileLock::acquire(&root),Err(e) if e.kind()==io::ErrorKind::WouldBlock)
        );
        drop(lock);
        drop(ProfileLock::acquire(&root).unwrap());
        let target = root.join("untouched");
        std::fs::write(&target, b"original").unwrap();
        std::fs::remove_file(root.join("instance.lock")).unwrap();
        std::fs::hard_link(&target, root.join("instance.lock")).unwrap();
        assert!(ProfileLock::acquire(&root).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
        std::fs::remove_dir_all(root).unwrap();
    }
}
