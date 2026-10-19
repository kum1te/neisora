//! Resolve packaged helpers first, then executables in absolute PATH directories.
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{path::PathBuf, sync::OnceLock};
static RESOURCES: OnceLock<PathBuf> = OnceLock::new();
pub fn init(path: PathBuf) {
    let _ = RESOURCES.set(path);
}
fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = RESOURCES.get() {
        roots.push(root.join("runtime"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(root) = exe.parent() {
            roots.push(root.join("runtime"));
            roots.push(root.join("../lib/neisora/runtime"));
        }
    }
    roots
}
pub fn resolve(name: &str) -> Result<PathBuf, String> {
    #[cfg(windows)]
    let packaged = match name {
        "python3" => "python/python.exe".to_owned(),
        "yt-dlp" => "yt-dlp".to_owned(),
        _ => format!("{name}.exe"),
    };
    #[cfg(unix)]
    let packaged = if name == "python3" {
        "python/bin/python3.10".to_owned()
    } else {
        name.to_owned()
    };
    let mut candidates: Vec<_> = roots().iter().map(|p| p.join(&packaged)).collect();
    #[cfg(windows)]
    let executable = if name == "python3" {
        "python.exe".to_owned()
    } else {
        format!("{name}.exe")
    };
    #[cfg(unix)]
    let executable = name.to_owned();
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(
            std::env::split_paths(&path)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(&executable)),
        );
    }
    candidates.into_iter().find(|p| p.is_file() && p.metadata().is_ok_and(|m| {
        #[cfg(unix)] { m.permissions().mode() & 0o111 != 0 }
        #[cfg(windows)] { let _ = m; true }
    })).ok_or_else(|| format!("Не найден {name}. Переустанови neisora или установи компонент через пакетный менеджер."))
}
pub fn command(name: &str) -> Result<tokio::process::Command, String> {
    #[cfg(windows)]
    crate::process::initialize()?;
    let executable = resolve(name)?;
    #[cfg(windows)]
    let python = if name == "yt-dlp" && executable.extension().is_none() {
        Some(resolve("python3")?)
    } else {
        None
    };
    #[cfg(unix)]
    let python = resolve("python3")
        .ok()
        .filter(|p| p.ends_with("python/bin/python3.10"));
    let mut command = if let Some(python) = python.as_ref().filter(|_| name == "yt-dlp") {
        let mut command = tokio::process::Command::new(python);
        command.arg(executable);
        command
    } else {
        tokio::process::Command::new(executable)
    };
    #[cfg(unix)]
    if let Some(python) = python.filter(|_| name == "python3" || name == "yt-dlp") {
        let home = python.parent().unwrap().parent().unwrap();
        command
            .env("PYTHONHOME", home)
            .env_remove("PYTHONPATH")
            .env("PYTHONNOUSERSITE", "1");
        // The interpreter/OpenSSL come from Ubuntu, but trust must follow the host
        // distribution (Fedora and Arch keep their CA bundle in different places).
        if std::env::var_os("SSL_CERT_FILE").is_none() {
            for path in [
                "/etc/ssl/certs/ca-certificates.crt",
                "/etc/pki/tls/certs/ca-bundle.crt",
                "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
                "/etc/ssl/cert.pem",
                "/etc/ca-certificates/extracted/tls-ca-bundle.pem",
            ] {
                if std::path::Path::new(path).is_file() {
                    command.env("SSL_CERT_FILE", path);
                    break;
                }
            }
        }
    }
    crate::process::parent_death(&mut command);
    Ok(command)
}

/// Called before GTK or async workers start. Other Linux GPUs keep WebKit defaults.
pub fn configure_graphics() {
    let mode = std::env::var("NEISORA_RENDERER").unwrap_or_else(|_| "auto".into());
    match mode.as_str() {
        "native" => std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER"),
        "safe" => std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
        _ if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
            && std::env::var_os("WAYLAND_DISPLAY").is_some()
            && std::path::Path::new("/sys/module/nvidia").exists() =>
        {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1")
        }
        _ => (),
    }
}
pub fn safe_renderer() -> bool {
    std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_ok_and(|v| v == "1")
}

#[cfg(all(test, windows))]
mod windows_tests {
    #[tokio::test]
    async fn packaged_zipapp_runs_through_private_python_and_job() {
        super::init(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources"));
        let mut command = super::command("yt-dlp").unwrap();
        command.arg("--version");
        let output = crate::process::output(command, std::time::Duration::from_secs(15), 65536)
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "2026.08.19");
    }
}
