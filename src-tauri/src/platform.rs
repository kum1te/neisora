//! Filesystem and process primitives with explicit native security behavior.
use std::{io, path::Path};
#[cfg(windows)]
pub mod windows;

pub fn private_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
    }
    #[cfg(windows)]
    {
        windows::private_directory(path)
    }
}
pub fn replace(from: &Path, to: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::rename(from, to)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        if unsafe {
            MoveFileExW(
                windows::wide(from).as_ptr(),
                windows::wide(to).as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
}
pub fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // kill(pid, 0) also succeeds for a zombie. Container PID 1 may leave
        // a terminated app unreaped; its abandoned IPC/cookie files are stale.
        // If /proc is unavailable, retain the conservative Unix behavior.
        #[cfg(target_os = "linux")]
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            if matches!(
                stat.rsplit_once(") ")
                    .and_then(|(_, fields)| fields.as_bytes().first()),
                Some(b'Z' | b'X')
            ) {
                return false;
            }
        }
        !(unsafe { libc::kill(pid as i32, 0) } == -1
            && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH))
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, STILL_ACTIVE},
            System::Threading::{
                GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        };
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return io::Error::last_os_error().raw_os_error()
                != Some(ERROR_INVALID_PARAMETER as i32);
        }
        let mut code = 0;
        let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
        unsafe { CloseHandle(handle) };
        ok == 0 || code == STILL_ACTIVE as u32
    }
}

#[allow(clippy::unnecessary_cast)] // statvfs f_fsid differs between Unix ABIs.
pub fn volume(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        let path = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
            return None;
        }
        Some(unsafe { stat.assume_init() }.f_fsid as u64)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};
        let mut root = vec![0u16; 32768];
        let mut serial = 0;
        if unsafe {
            GetVolumePathNameW(
                windows::wide(path).as_ptr(),
                root.as_mut_ptr(),
                root.len() as u32,
            )
        } == 0
        {
            return None;
        }
        let ok = unsafe {
            GetVolumeInformationW(
                root.as_ptr(),
                std::ptr::null_mut(),
                0,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        (ok != 0).then_some(serial as u64)
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn unreaped_process_does_not_preserve_abandoned_session_files() {
        use std::{process::Command, time::Duration};
        assert!(process_alive(std::process::id()));
        let mut child = Command::new("sh").args(["-c", "exit 0"]).spawn().unwrap();
        let pid = child.id();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while process_alive(pid) && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        // Do not reap until after the checks: this reproduces a container's
        // dead parent that still passes the old kill(pid, 0) liveness check.
        let dead = !process_alive(pid);
        let still_has_pid = unsafe { libc::kill(pid as i32, 0) } == 0;
        let directory =
            std::env::temp_dir().join(format!("neisora-zombie-{}-{pid}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let abandoned = directory.join(format!("mpv-{pid}.sock"));
        let snapshot = directory.join(format!("stream-session-{pid}-123.txt"));
        let live = directory.join(format!("mpv-{}.sock", std::process::id()));
        for path in [&abandoned, &snapshot, &live] {
            std::fs::write(path, b"fixture").unwrap();
        }
        crate::api::cleanup_temporary_files(&directory, false);
        let cleaned = !abandoned.exists() && !snapshot.exists() && live.exists();
        child.wait().unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        assert!(
            dead && still_has_pid,
            "test must exercise a real unreaped child"
        );
        assert!(
            cleaned,
            "remove dead-session files and preserve the live instance"
        );
    }
}
