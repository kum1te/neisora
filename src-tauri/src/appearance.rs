use crate::api::private_write;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub accent: String,
    pub background: String,
    pub surface: String,
    pub text: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VisualizerPreferences {
    pub mode: String,
    pub intensity: f64,
    pub speed: f64,
}
impl Default for VisualizerPreferences {
    fn default() -> Self {
        Self {
            mode: "ribbon".into(),
            intensity: 0.55,
            speed: 1.0,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub selected: String,
    pub custom: Vec<Theme>,
    pub follow_wallpaper: bool,
    pub visualizer: VisualizerPreferences,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            selected: "violet".into(),
            custom: vec![],
            follow_wallpaper: false,
            visualizer: VisualizerPreferences::default(),
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceState {
    pub preferences: Preferences,
    pub wallpaper: Option<Theme>,
}
pub struct Appearance {
    directory: PathBuf,
    pub preferences: Mutex<Preferences>,
    wallpaper: Mutex<Option<(String, Theme)>>,
}
fn valid_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
}
fn valid_preferences(p: &Preferences) -> bool {
    ["off", "ribbon", "rings", "waves"].contains(&p.visualizer.mode.as_str())
        && p.visualizer.intensity.is_finite()
        && (0.0..=1.0).contains(&p.visualizer.intensity)
        && p.visualizer.speed.is_finite()
        && (0.5..=2.0).contains(&p.visualizer.speed)
        && p.selected.len() <= 100
        && p.custom.len() <= 40
        && p.custom.iter().all(|t| {
            t.name.len() <= 100
                && t.id.len() <= 100
                && [&t.accent, &t.background, &t.surface, &t.text]
                    .iter()
                    .all(|s| valid_color(s))
        })
}
impl Appearance {
    pub fn new(directory: PathBuf) -> Self {
        let prefs = std::fs::read(directory.join("appearance.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(valid_preferences)
            .unwrap_or_default();
        Self {
            directory,
            preferences: Mutex::new(prefs),
            wallpaper: Mutex::new(None),
        }
    }
    pub async fn save(&self, preferences: Preferences) -> Result<(), String> {
        if !valid_preferences(&preferences) {
            return Err("Проверь название и цвета темы".into());
        }
        let mut current = self.preferences.lock().await;
        private_write(
            &self.directory.join("appearance.json"),
            &serde_json::to_vec(&preferences).map_err(|e| e.to_string())?,
        )?;
        *current = preferences;
        Ok(())
    }
    pub async fn state(&self) -> AppearanceState {
        let preferences = self.preferences.lock().await.clone();
        AppearanceState {
            preferences,
            wallpaper: None,
        }
    }
    pub async fn wallpaper_theme(&self) -> Result<Theme, String> {
        let path = crate::wallpaper::path().await?;
        let path = std::fs::canonicalize(path).map_err(|_| "Файл обоев недоступен")?;
        if !path.is_file() {
            return Err("Обои должны быть локальным изображением".into());
        }
        let decoder = match path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "jpg" | "jpeg" => "jpeg",
            "png" => "png",
            "webp" => "webp",
            "gif" => "gif",
            "avif" => "avif",
            "bmp" => "bmp",
            _ => return Err("Этот формат обоев пока не поддерживается".into()),
        };
        let modified = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|t| t.as_nanos())
            .unwrap_or_default();
        let key = format!("{}:{modified}", path.display());
        let mut cached = self.wallpaper.lock().await;
        if let Some((old, theme)) = cached.as_ref() {
            if old == &key {
                return Ok(theme.clone());
            }
        }
        let converter =
            crate::runtime::resolve("magick").or_else(|_| crate::runtime::resolve("convert"))?;
        let mut cmd = tokio::process::Command::new(converter);
        cmd.args([
            "-limit", "memory", "128MiB", "-limit", "map", "128MiB", "-limit", "disk", "0",
            "-limit", "thread", "1", "-limit", "time", "5",
        ])
        .arg(format!("{decoder}:{}[0]", path.display()))
        .args([
            "-thumbnail",
            "64x64!",
            "-colors",
            "8",
            "-depth",
            "8",
            "-format",
            "%c",
            "histogram:info:-",
        ])
        .kill_on_drop(true);
        let output = crate::process::output(cmd, Duration::from_secs(6), 256 * 1024).await?;
        if !output.status.success() {
            return Err("Не удалось извлечь цвета обоев".into());
        }
        let mut best = None;
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let count = line
                .trim()
                .split(':')
                .next()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.0);
            let Some(hex) = line
                .split_whitespace()
                .find(|s| s.starts_with('#') && s.len() >= 7)
            else {
                continue;
            };
            let (Ok(r), Ok(g), Ok(b)) = (
                u8::from_str_radix(&hex[1..3], 16),
                u8::from_str_radix(&hex[3..5], 16),
                u8::from_str_radix(&hex[5..7], 16),
            ) else {
                continue;
            };
            let hi = r.max(g).max(b) as f64;
            let lo = r.min(g).min(b) as f64;
            if hi < 45.0 || lo > 235.0 {
                continue;
            }
            let score = count.sqrt() * ((hi - lo) / 255.0 + 0.1);
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, [r, g, b]));
            }
        }
        let rgb = best.map(|(_, c)| c).unwrap_or([180, 180, 190]);
        let theme = Theme {
            id: "wallpaper".into(),
            name: "Из обоев".into(),
            accent: color(rgb, 0.35, 0.65),
            background: color(rgb, 0.0, 0.065),
            surface: color(rgb, 0.025, 0.085),
            text: "#f3f2f5".into(),
        };
        *cached = Some((key, theme.clone()));
        Ok(theme)
    }
}
fn color(rgb: [u8; 3], white: f64, amount: f64) -> String {
    let c = rgb.map(|x| ((x as f64 * amount + 255.0 * white).clamp(0.0, 255.0)) as u8);
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_invalid_theme_falls_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("neisora-theme-audit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = Preferences::default();
        p.visualizer.speed = 999.0;
        std::fs::write(dir.join("appearance.json"), serde_json::to_vec(&p).unwrap()).unwrap();
        let loaded = Appearance::new(dir.clone());
        assert_eq!(loaded.preferences.try_lock().unwrap().visualizer.speed, 1.0);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
