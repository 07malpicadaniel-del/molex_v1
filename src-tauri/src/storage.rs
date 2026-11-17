use std::fs;
use std::path::PathBuf;

use molex_core::DeviceProfile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopSettings {
    pub target_fps: u32,
    pub auto_connect_after_wol: bool,
    pub hide_local_cursor_in_stream: bool,
    pub sticky_super_default: bool,
    pub decoder_threads: u32,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            target_fps: 60,
            auto_connect_after_wol: true,
            hide_local_cursor_in_stream: false,
            sticky_super_default: false,
            decoder_threads: 4,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersistedData {
    pub devices: Vec<DeviceProfile>,
    pub settings: DesktopSettings,
}

pub struct PersistedStore {
    path: PathBuf,
    data: PersistedData,
}

impl PersistedStore {
    pub fn load_default() -> Self {
        let base_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("molex");
        let path = base_dir.join("desktop_state.json");
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PersistedData>(&raw).ok())
            .unwrap_or_default();

        Self { path, data }
    }

    fn persist(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp_path = self.path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(&self.data).map_err(|e| e.to_string())?;
        fs::write(&tmp_path, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &self.path).map_err(|e| e.to_string())?;
        Ok(())
    }

    #[cfg(test)]
    pub fn load_from_path(path: PathBuf) -> Self {
        let data = fs::read_to_string(&path)
            .ok()
            .and_then(|raw| serde_json::from_str::<PersistedData>(&raw).ok())
            .unwrap_or_default();
        Self { path, data }
    }

    pub fn list_devices(&self) -> Vec<DeviceProfile> {
        self.data.devices.clone()
    }

    pub fn upsert_device(&mut self, mut device: DeviceProfile) -> Result<Vec<DeviceProfile>, String> {
        if device.id.trim().is_empty() {
            device.id = uuid::Uuid::new_v4().to_string();
        }
        let clean = device.sanitized();
        if let Some(pos) = self.data.devices.iter().position(|d| d.id == clean.id) {
            self.data.devices[pos] = clean;
        } else {
            self.data.devices.push(clean);
        }
        self.persist()?;
        Ok(self.data.devices.clone())
    }

    pub fn delete_device(&mut self, device_id: &str) -> Result<Vec<DeviceProfile>, String> {
        self.data.devices.retain(|d| d.id != device_id);
        self.persist()?;
        Ok(self.data.devices.clone())
    }

    pub fn get_settings(&self) -> DesktopSettings {
        self.data.settings.clone()
    }

    pub fn save_settings(&mut self, mut settings: DesktopSettings) -> Result<DesktopSettings, String> {
        settings.target_fps = settings.target_fps.clamp(15, 144);
        settings.decoder_threads = settings.decoder_threads.clamp(1, 16);
        self.data.settings = settings.clone();
        self.persist()?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_devices_and_clamps_settings() {
        let tmp = std::env::temp_dir().join(format!("molex_test_{}.json", uuid::Uuid::new_v4()));
        let mut store = PersistedStore::load_from_path(tmp.clone());

        let dev = DeviceProfile {
            name: "Arch Box".into(),
            host: "100.64.0.5".into(),
            mac_address: Some("aa:bb:cc:dd:ee:ff".into()),
            username: "daniel".into(),
            ..Default::default()
        };
        let list = store.upsert_device(dev).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].mac_address.as_deref(), Some("AA:BB:CC:DD:EE:FF"));

        let saved_settings = store
            .save_settings(DesktopSettings {
                target_fps: 240,
                decoder_threads: 99,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(saved_settings.target_fps, 144);
        assert_eq!(saved_settings.decoder_threads, 16);

        let reloaded = PersistedStore::load_from_path(tmp.clone());
        assert_eq!(reloaded.list_devices().len(), 1);
        assert_eq!(reloaded.get_settings().target_fps, 144);

        let _ = fs::remove_file(tmp);
    }
}
