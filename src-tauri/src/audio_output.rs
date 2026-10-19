//! Persist only the desired device. Temporary disconnection must not overwrite it.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tokio::sync::Mutex;
#[derive(Clone, Serialize)]
pub struct Device {
    pub name: String,
    pub description: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub preferred: String,
    pub selected: String,
    pub fallback: bool,
    pub silent: bool,
    pub devices: Vec<Device>,
}
#[derive(Serialize, Deserialize)]
struct Saved {
    name: String,
}
pub struct Output {
    path: PathBuf,
    preferred: Mutex<String>,
    pub gate: Mutex<()>,
}
fn valid(name: &str) -> bool {
    !name.is_empty() && name.len() <= 1024 && !name.chars().any(char::is_control)
}
pub fn devices(value: Value) -> Result<Vec<Device>, String> {
    let values = value
        .as_array()
        .ok_or("Плеер не вернул список аудиовыходов")?;
    let mut result = vec![Device {
        name: "auto".into(),
        description: "Как в системе".into(),
    }];
    for item in values.iter().take(256) {
        let Some(name) = item["name"].as_str().filter(|name| valid(name)) else {
            continue;
        };
        if name == "auto"
            || name == "null"
            || name.starts_with("null/")
            || result.iter().any(|d| d.name == name)
        {
            continue;
        }
        let description = item["description"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(name)
            .chars()
            .take(512)
            .collect();
        result.push(Device {
            name: name.into(),
            description,
        });
    }
    Ok(result)
}
pub fn desired<'a>(preferred: &'a str, devices: &[Device]) -> &'a str {
    if devices.iter().any(|d| d.name == preferred) {
        preferred
    } else {
        "auto"
    }
}
impl Output {
    pub fn new(directory: PathBuf) -> Self {
        let path = directory.join("audio-output.json");
        let name = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.len() <= 8192)
            .and_then(|_| std::fs::read(&path).ok())
            .and_then(|raw| serde_json::from_slice::<Saved>(&raw).ok())
            .map(|s| s.name)
            .filter(|name| valid(name))
            .unwrap_or_else(|| "auto".into());
        Self {
            path,
            preferred: Mutex::new(name),
            gate: Mutex::new(()),
        }
    }
    pub async fn preferred(&self) -> String {
        self.preferred.lock().await.clone()
    }
    pub async fn save(&self, name: String) -> Result<(), String> {
        if !valid(&name) {
            return Err("Некорректный аудиовыход".into());
        }
        let raw = serde_json::to_vec(&Saved { name: name.clone() })
            .map_err(|_| "Не удалось сохранить аудиовыход")?;
        crate::api::private_write(&self.path, &raw)
            .map_err(|_| "Не удалось сохранить аудиовыход")?;
        *self.preferred.lock().await = name;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn desired_device_survives_store_restart() {
        let directory =
            std::env::temp_dir().join(format!("neisora-output-pref-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let first = Output::new(directory.clone());
        first.save("pulse/usb".into()).await.unwrap();
        let reopened = Output::new(directory.clone());
        assert_eq!(reopened.preferred().await, "pulse/usb");
        assert!(first.save("pulse/x\nrun".into()).await.is_err());
        assert_eq!(reopened.preferred().await, "pulse/usb");
        std::fs::remove_dir_all(directory).unwrap();
    }
    #[test]
    fn hotplug_falls_back_without_losing_desired_device() {
        let present=devices(serde_json::json!([{"name":"pulse/usb","description":"USB"},{"name":"pulse/usb"},{"name":"null"}])).unwrap();
        assert_eq!(present.len(), 2);
        assert_eq!(desired("pulse/usb", &present), "pulse/usb");
        let absent = devices(serde_json::json!([])).unwrap();
        assert_eq!(desired("pulse/usb", &absent), "auto");
        assert_eq!(desired("pulse/usb", &present), "pulse/usb");
        assert_eq!(desired("auto", &present), "auto");
        assert!(!valid("pulse/x\nrun"));
        assert!(!valid(&"x".repeat(1025)));
        assert!(devices(Value::Null).is_err());
    }
}
