//! Windows Job Objects own helpers. The application's outer job is inherited by all children.
use std::{
    collections::HashMap,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::{Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
use windows_sys::Win32::System::{
    JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
    Threading::{GetCurrentProcess, CREATE_NO_WINDOW, CREATE_SUSPENDED},
};
fn resume(pid: u32) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD,
                THREADENTRY32,
            },
            Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME},
        },
    };
    unsafe {
        let raw = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
        if raw == INVALID_HANDLE_VALUE {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let snapshot = OwnedHandle::from_raw_handle(raw);
        let mut entry = std::mem::zeroed::<THREADENTRY32>();
        entry.dwSize = std::mem::size_of::<THREADENTRY32>() as u32;
        let mut found = Thread32First(snapshot.as_raw_handle(), &mut entry);
        while found != 0 {
            if entry.th32OwnerProcessID == pid {
                let raw = OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID);
                if raw.is_null() {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                let thread = OwnedHandle::from_raw_handle(raw);
                if ResumeThread(thread.as_raw_handle()) == u32::MAX {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                return Ok(());
            }
            found = Thread32Next(snapshot.as_raw_handle(), &mut entry);
        }
        Err("Не найден поток системного компонента".into())
    }
}
fn new_job() -> Result<OwnedHandle, String> {
    unsafe {
        let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if raw.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let handle = OwnedHandle::from_raw_handle(raw);
        let mut info = std::mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            raw,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&info) as u32,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(handle)
    }
}
pub fn initialize() -> Result<(), String> {
    static ROOT: OnceLock<Result<OwnedHandle, String>> = OnceLock::new();
    ROOT.get_or_init(|| {
        let job = new_job()?;
        if unsafe { AssignProcessToJobObject(job.as_raw_handle(), GetCurrentProcess()) } == 0 {
            return Err("Не удалось изолировать дочерние процессы Windows".into());
        }
        Ok(job)
    })
    .as_ref()
    .map(|_| ())
    .map_err(Clone::clone)
}
pub fn parent_death(cmd: &mut Command) {
    cmd.creation_flags(CREATE_NO_WINDOW);
}
#[derive(Default)]
struct Jobs {
    closed: bool,
    active: HashMap<u64, Arc<OwnedHandle>>,
}
fn jobs() -> &'static Mutex<Jobs> {
    static JOBS: OnceLock<Mutex<Jobs>> = OnceLock::new();
    JOBS.get_or_init(Mutex::default)
}
struct Group(u64);
impl Drop for Group {
    fn drop(&mut self) {
        if let Some(job) = jobs().lock().unwrap().active.remove(&self.0) {
            unsafe { TerminateJobObject(job.as_raw_handle(), 1) };
        }
    }
}
pub fn terminate_active() {
    for (_, job) in jobs().lock().unwrap().active.drain() {
        unsafe { TerminateJobObject(job.as_raw_handle(), 1) };
    }
}
pub fn terminate_all() {
    let mut jobs = jobs().lock().unwrap();
    jobs.closed = true;
    for (_, job) in jobs.active.drain() {
        unsafe { TerminateJobObject(job.as_raw_handle(), 1) };
    }
}
async fn read(stream: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    stream
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("Ответ компонента слишком большой".into());
    }
    Ok(bytes)
}
pub async fn output(
    mut cmd: Command,
    timeout: Duration,
    stdout_limit: usize,
) -> Result<Output, String> {
    initialize()?;
    // Assign the job before the first instruction can spawn descendants.
    cmd.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let (mut child, _group) = {
        let mut jobs = jobs().lock().unwrap();
        if jobs.closed {
            return Err("Приложение закрывается".into());
        }
        let job = Arc::new(new_job()?);
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let id = SERIAL.fetch_add(1, Ordering::Relaxed);
        if unsafe {
            AssignProcessToJobObject(
                job.as_raw_handle(),
                child.raw_handle().ok_or("Нет дескриптора процесса")?,
            )
        } == 0
        {
            let _ = child.start_kill();
            return Err("Не удалось изолировать процесс компонента".into());
        }
        resume(child.id().ok_or("Компонент завершился")?)?;
        jobs.active.insert(id, job);
        (child, Group(id))
    };
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    tokio::time::timeout(timeout, async {
        let (stdout, stderr, status) = tokio::try_join!(
            read(stdout, stdout_limit),
            read(stderr, 256 * 1024),
            async { child.wait().await.map_err(|e| e.to_string()) }
        )?;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    })
    .await
    .map_err(|_| "Системный компонент не ответил вовремя".to_owned())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn windows_helpers_have_bounded_output_and_timeout() {
        let mut normal = Command::new("cmd.exe");
        normal.args(["/D", "/C", "echo working"]);
        let result = output(normal, Duration::from_secs(5), 1024).await.unwrap();
        assert!(result.status.success());
        assert!(String::from_utf8_lossy(&result.stdout).contains("working"));
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/D", "/C", "echo 0123456789"]);
        assert!(output(cmd, Duration::from_secs(5), 4).await.is_err());
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/D", "/C", "ping -n 10 127.0.0.1 >nul"]);
        assert!(output(cmd, Duration::from_millis(300), 1024).await.is_err());
    }
}
