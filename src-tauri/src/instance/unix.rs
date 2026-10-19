//! One writer per profile; fixed local activation messages need no desktop session bus.
use std::os::unix::{fs::PermissionsExt, net::UnixDatagram};
use std::{
    fs::File,
    io,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};
use tauri::Manager;

pub struct ProfileLock {
    _file: File,
}
impl ProfileLock {
    pub fn acquire(directory: &Path) -> io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(directory.join("instance.lock"))?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != unsafe { libc::geteuid() }
        {
            return Err(io::Error::other("Небезопасный файл блокировки профиля"));
        }
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        use std::os::fd::AsRawFd;
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::WouldBlock {
                return Err(error);
            }
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Профиль уже открыт в другом экземпляре neisora",
            ));
        }
        // Never unlink the lock: replacing its inode would allow two owners.
        Ok(Self { _file: file })
    }
}

pub struct Instance {
    _lock: ProfileLock,
    socket: Option<UnixDatagram>,
    path: PathBuf,
}
impl Drop for Instance {
    fn drop(&mut self) {
        // The file lock is still held while removing our endpoint.
        let _ = std::fs::remove_file(&self.path);
    }
}
impl Instance {
    pub fn acquire(directory: &Path) -> io::Result<Option<Self>> {
        let lock = match ProfileLock::acquire(directory) {
            Ok(lock) => lock,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                let socket = UnixDatagram::unbound()?;
                socket.set_write_timeout(Some(std::time::Duration::from_millis(100)))?;
                // An owner may still be between locking and binding. Never take over its lock.
                for _ in 0..20 {
                    if socket
                        .send_to(&[1], directory.join("instance.sock"))
                        .is_ok()
                    {
                        return Ok(None);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
                return Err(io::Error::other(
                    "neisora уже запущена, но окно пока не отвечает",
                ));
            }
            Err(e) => return Err(e),
        };
        let path = directory.join("instance.sock");
        match std::fs::remove_file(&path) {
            Ok(()) => (),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            Err(e) => return Err(e),
        }
        let socket = UnixDatagram::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        socket.set_nonblocking(true)?;
        Ok(Some(Self {
            _lock: lock,
            socket: Some(socket),
            path,
        }))
    }
    pub fn listen(&mut self, app: tauri::AppHandle) {
        let socket = self
            .socket
            .take()
            .expect("Instance listener already started");
        tauri::async_runtime::spawn(async move {
            let Ok(socket) = tokio::net::UnixDatagram::from_std(socket) else {
                return;
            };
            let mut message = [0; 2];
            while let Ok(size) = socket.recv(&mut message).await {
                // Only one fixed activation byte is accepted. No arguments, URLs or commands.
                if size == 1 && message[0] == 1 {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.unminimize();
                        let _ = window.set_focus();
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn second_instance_only_sends_activation_and_owner_cleans_endpoint() {
        let dir = std::env::temp_dir().join(format!("neisora-activation-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = Instance::acquire(&dir).unwrap().unwrap();
        assert_eq!(
            std::fs::metadata(dir.join("instance.sock"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(Instance::acquire(&dir).unwrap().is_none());
        let mut message = [0; 2];
        assert_eq!(
            first.socket.as_ref().unwrap().recv(&mut message).unwrap(),
            1
        );
        assert_eq!(message[0], 1);
        drop(first);
        assert!(!dir.join("instance.sock").exists());
        assert!(Instance::acquire(&dir).unwrap().is_some());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn profile_is_exclusive_and_reusable_without_deleting_lock() {
        let dir = std::env::temp_dir().join(format!("neisora-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = ProfileLock::acquire(&dir).unwrap();
        assert!(ProfileLock::acquire(&dir).is_err());
        drop(first);
        assert!(ProfileLock::acquire(&dir).is_ok());
        assert!(dir.join("instance.lock").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn rejects_symlink_and_hardlink_without_touching_target() {
        let dir = std::env::temp_dir().join(format!("neisora-lock-link-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("unrelated");
        std::fs::write(&target, b"untouched").unwrap();
        std::os::unix::fs::symlink(&target, dir.join("instance.lock")).unwrap();
        assert!(ProfileLock::acquire(&dir).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"untouched");
        std::fs::remove_file(dir.join("instance.lock")).unwrap();
        std::fs::hard_link(&target, dir.join("instance.lock")).unwrap();
        assert!(ProfileLock::acquire(&dir).is_err());
        assert_eq!(std::fs::read(&target).unwrap(), b"untouched");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
