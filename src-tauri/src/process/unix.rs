//! Own the entire external job, including helpers (e.g. yt-dlp's Node runtime).
use std::{
    collections::HashSet,
    os::unix::process::CommandExt,
    process::{Output, Stdio},
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
#[derive(Default)]
struct Jobs {
    closed: bool,
    groups: HashSet<u32>,
}
fn jobs() -> &'static Mutex<Jobs> {
    static JOBS: OnceLock<Mutex<Jobs>> = OnceLock::new();
    JOBS.get_or_init(Mutex::default)
}
struct Group(u32);
impl Drop for Group {
    fn drop(&mut self) {
        let mut jobs = jobs().lock().unwrap();
        if jobs.groups.remove(&self.0) {
            kill(self.0);
        }
    }
}
fn kill(pid: u32) {
    // Negative PID targets only the process group created for this job.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}
/// Linux also stops the direct helper when the app is killed without Rust cleanup.
pub fn parent_death(cmd: &mut Command) {
    #[cfg(target_os = "linux")]
    {
        let parent = std::process::id() as libc::pid_t;
        unsafe {
            cmd.as_std_mut().pre_exec(move || {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                // The parent could have died between fork and prctl.
                if libc::getppid() != parent {
                    libc::kill(libc::getpid(), libc::SIGKILL);
                }
                Ok(())
            });
        }
    }
}
pub fn terminate_active() {
    let mut jobs = jobs().lock().unwrap();
    for pid in jobs.groups.drain() {
        kill(pid);
    }
}
pub fn terminate_all() {
    let mut jobs = jobs().lock().unwrap();
    jobs.closed = true;
    for pid in jobs.groups.drain() {
        kill(pid);
    }
}

async fn read(stream: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    stream
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "Не удалось прочитать ответ компонента")?;
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
    cmd.as_std_mut().process_group(0);
    parent_death(&mut cmd);
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let (mut child, _group) = {
        let mut jobs = jobs().lock().unwrap();
        if jobs.closed {
            return Err("Приложение закрывается".into());
        }
        let child = cmd
            .spawn()
            .map_err(|_| "Не удалось запустить системный компонент")?;
        let pid = child.id().ok_or("Компонент уже завершился")?;
        jobs.groups.insert(pid);
        (child, Group(pid))
    };
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let result = tokio::time::timeout(timeout, async {
        let (stdout, stderr, status) = tokio::try_join!(
            read(stdout, stdout_limit),
            read(stderr, 256 * 1024),
            async {
                child
                    .wait()
                    .await
                    .map_err(|_| "Не удалось дождаться компонента".to_owned())
            }
        )?;
        Ok(Output {
            status,
            stdout,
            stderr,
        })
    })
    .await
    .map_err(|_| "Системный компонент не ответил вовремя".to_owned())?;
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn caps_output_and_kills_descendants_on_timeout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf '0123456789'"]);
        assert!(output(cmd, Duration::from_secs(2), 4).await.is_err());
        let path = std::env::temp_dir().join(format!("neisora-child-{}", std::process::id()));
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30 & echo $! > \"$1\"; wait", "neisora-test"])
            .arg(&path);
        assert!(output(cmd, Duration::from_millis(250), 1024).await.is_err());
        let pid = std::fs::read_to_string(&path).unwrap();
        let _ = std::fs::remove_file(path);
        tokio::time::sleep(Duration::from_millis(50)).await;
        // A zombie is already terminated and waits only for its reaper.
        let state =
            std::fs::read_to_string(format!("/proc/{}/stat", pid.trim())).unwrap_or_default();
        assert!(
            state.is_empty()
                || state
                    .rsplit(')')
                    .next()
                    .unwrap()
                    .trim_start()
                    .starts_with('Z')
        );
    }
}
