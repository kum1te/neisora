//! Desktop-specific discovery. Never fetch remote images or run shell commands.
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

fn text(path: &Path) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 * 1024 {
        return None;
    }
    String::from_utf8(bytes).ok()
}
fn local(value: &str) -> Option<PathBuf> {
    if value.starts_with("file:") {
        let url = reqwest::Url::parse(value).ok()?;
        if url
            .host_str()
            .is_some_and(|host| !host.is_empty() && host != "localhost")
        {
            return None;
        }
        return url.to_file_path().ok();
    }
    let path = PathBuf::from(value);
    #[cfg(windows)]
    if !matches!(path.components().next(),Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(),std::path::Prefix::Disk(_)|std::path::Prefix::VerbatimDisk(_)))
    {
        return None;
    }
    path.is_absolute().then_some(path)
}
fn kde(config: &str) -> Option<PathBuf> {
    let mut image_section = false;
    for line in config.lines().map(str::trim) {
        if line.starts_with('[') {
            image_section = line.contains("[Wallpaper][org.kde.image][General]");
        }
        if image_section {
            if let Some(value) = line.strip_prefix("Image=") {
                if let Some(path) = local(value) {
                    return Some(path);
                }
            }
        }
    }
    None
}
#[cfg(target_os = "linux")]
fn setting(schema: &str, key: &str) -> Option<String> {
    use webkit2gtk::gio::{self, prelude::*};
    let schema = gio::SettingsSchemaSource::default()?.lookup(schema, true)?;
    if !schema.has_key(key) {
        return None;
    }
    let settings = gio::Settings::new_full(&schema, None::<&gio::SettingsBackend>, None);
    Some(settings.string(key).to_string())
}
#[cfg(not(target_os = "linux"))]
fn setting(_: &str, _: &str) -> Option<String> {
    None
}

pub async fn path() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        };
        let mut buffer = vec![0u16; 32768];
        // This query reads a local path; no registry expansion or remote shell is used.
        if unsafe {
            SystemParametersInfoW(
                SPI_GETDESKWALLPAPER,
                buffer.len() as u32,
                Some(buffer.as_mut_ptr().cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .is_ok()
        {
            let length = buffer.iter().position(|v| *v == 0).unwrap_or(buffer.len());
            if let Some(path) = local(&String::from_utf16_lossy(&buffer[..length])) {
                return Ok(path);
            }
        }
    }
    let config = dirs::config_dir().ok_or("Не найден каталог настроек рабочего стола")?;
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if desktop.contains("hypr") || desktop.is_empty() {
        if let Some(raw) = text(&config.join("illogical-impulse/config.json")) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(path) = value
                    .pointer("/background/wallpaperPath")
                    .and_then(|v| v.as_str())
                    .and_then(local)
                {
                    return Ok(path);
                }
            }
        }
    }
    if desktop.contains("gnome")
        || desktop.contains("unity")
        || desktop.contains("cinnamon")
        || desktop.contains("mate")
    {
        let schema = if desktop.contains("cinnamon") {
            "org.cinnamon.desktop.background"
        } else if desktop.contains("mate") {
            "org.mate.background"
        } else {
            "org.gnome.desktop.background"
        };
        let dark = setting("org.gnome.desktop.interface", "color-scheme").as_deref()
            == Some("prefer-dark");
        let keys = if desktop.contains("mate") {
            vec!["picture-filename"]
        } else if dark {
            vec!["picture-uri-dark", "picture-uri"]
        } else {
            vec!["picture-uri", "picture-uri-dark"]
        };
        for key in keys {
            if let Some(path) = setting(schema, key).and_then(|v| local(&v)) {
                return Ok(path);
            }
        }
    }
    if desktop.contains("kde") {
        if let Some(path) = text(&config.join("plasma-org.kde.plasma.desktop-appletsrc"))
            .as_deref()
            .and_then(kde)
        {
            return Ok(path);
        }
    }
    if desktop.contains("xfce") {
        if let Ok(mut cmd) = crate::runtime::command("xfconf-query") {
            cmd.args(["-c", "xfce4-desktop", "-l", "-v"]);
            if let Ok(output) = crate::process::output(cmd, Duration::from_secs(2), 64 * 1024).await
            {
                if output.status.success() {
                    for line in String::from_utf8_lossy(&output.stdout).lines() {
                        if let Some((key, value)) = line.split_once(char::is_whitespace) {
                            if key.ends_with("/last-image") || key.ends_with("/image-path") {
                                if let Some(path) = local(value.trim()) {
                                    return Ok(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Err("Не удалось определить обои этого рабочего стола".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn missing_desktop_schema_is_optional_and_does_not_abort() {
        assert!(setting("local.neisora.nonexistent.schema", "picture-uri").is_none());
    }
    #[cfg(unix)]
    #[test]
    fn kde_reads_only_the_image_wallpaper_section_and_decodes_local_paths() {
        let config = "[Unrelated]\nImage=/wrong.png\n[Containments][4][Wallpaper][org.kde.image][General]\nImage=file:///tmp/wall%20paper.jpg\n";
        assert_eq!(kde(config), Some(PathBuf::from("/tmp/wall paper.jpg")));
        assert!(kde("[Unrelated]\nImage=/wrong.png").is_none());
    }
    #[cfg(windows)]
    #[test]
    fn windows_discovery_rejects_unc_and_device_paths() {
        for path in [
            r"\\server\share\wall.jpg",
            r"\\?\UNC\server\share\wall.jpg",
            r"\\.\PIPE\test",
            "file://server/share/wall.jpg",
        ] {
            assert!(local(path).is_none());
        }
        assert_eq!(local(r"C:\wall.jpg"), Some(PathBuf::from(r"C:\wall.jpg")));
    }
    #[test]
    fn discovery_rejects_remote_images_relative_paths_and_delegate_syntax() {
        for value in [
            "https://example.org/a.png",
            "file://example.org/a.png",
            "../a.png",
            "https:evil",
            "label:hello",
        ] {
            assert!(local(value).is_none());
        }
        #[cfg(unix)]
        assert_eq!(
            local("/tmp/local.jpg"),
            Some(PathBuf::from("/tmp/local.jpg"))
        );
    }
}
