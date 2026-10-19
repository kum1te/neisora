//! mpv JSON IPC uses local Unix sockets or Windows named pipes; never TCP.
use std::path::{Path, PathBuf};
#[cfg(unix)]
pub type Stream = tokio::net::UnixStream;
#[cfg(windows)]
pub type Stream = tokio::net::windows::named_pipe::NamedPipeClient;

pub fn endpoint(directory: &Path) -> PathBuf {
    #[cfg(unix)]
    {
        directory.join(format!("mpv-{}.sock", std::process::id()))
    }
    #[cfg(windows)]
    {
        use sha1::{Digest, Sha1};
        let profile = Sha1::digest(directory.as_os_str().as_encoded_bytes());
        PathBuf::from(format!(
            r"\\.\pipe\neisora-mpv-{:x}-{}",
            profile,
            std::process::id()
        ))
    }
}
pub async fn connect(path: &Path) -> std::io::Result<Stream> {
    #[cfg(unix)]
    {
        Stream::connect(path).await
    }
    #[cfg(windows)]
    {
        tokio::net::windows::named_pipe::ClientOptions::new().open(path)
    }
}
pub fn present(path: &Path) -> bool {
    #[cfg(unix)]
    {
        path.exists()
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            Foundation::{ERROR_PIPE_BUSY, ERROR_SEM_TIMEOUT},
            System::Pipes::WaitNamedPipeW,
        };
        let available =
            unsafe { WaitNamedPipeW(crate::platform::windows::wide(path).as_ptr(), 1) } != 0;
        available
            || matches!(std::io::Error::last_os_error().raw_os_error(),Some(code) if code==ERROR_PIPE_BUSY as i32||code==ERROR_SEM_TIMEOUT as i32)
    }
}
pub fn cleanup(path: &Path) {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(path);
    }
    #[cfg(windows)]
    {
        let _ = path;
    } // Closing the server releases its name automatically.
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn windows_pipe_supports_duplex_audio_ipc_without_tcp() {
        let name = endpoint(&std::env::temp_dir().join("neisora-pipe-test"));
        assert!(name.to_string_lossy().starts_with(r"\\.\pipe\neisora-mpv-"));
        let mut server = tokio::net::windows::named_pipe::ServerOptions::new()
            .first_pipe_instance(true)
            .reject_remote_clients(true)
            .create(&name)
            .unwrap();
        assert!(present(&name));
        let job = tokio::spawn(async move {
            server.connect().await.unwrap();
            let mut request = [0u8; 4];
            server.read_exact(&mut request).await.unwrap();
            assert_eq!(&request, b"ping");
            server.write_all(b"pong").await.unwrap();
        });
        let client = connect(&name).await.unwrap();
        let (mut read, mut write) = tokio::io::split(client);
        write.write_all(b"ping").await.unwrap();
        let mut reply = [0u8; 4];
        read.read_exact(&mut reply).await.unwrap();
        assert_eq!(&reply, b"pong");
        job.await.unwrap();
    }
}
