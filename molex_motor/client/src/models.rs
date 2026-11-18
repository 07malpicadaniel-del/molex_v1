use serde::{Deserialize, Serialize};

/// Perfil legado compatible 1:1 con los bindings UniFFI actuales de `app/` (Android).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Record))]
pub struct ServerProfile {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
}

/// Perfil extendido de dispositivo para gestión completa de red, Tailscale y Wake-on-LAN.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Record))]
pub struct DeviceProfile {
    pub id: String,
    pub name: String,
    pub host: String,
    pub tailscale_ip: Option<String>,
    pub mac_address: Option<String>,
    pub broadcast_ip: Option<String>,
    #[serde(default)]
    pub wol_relay_host: Option<String>,
    pub ssh_port: u16,
    pub video_port: u16,
    pub input_port: u16,
    pub username: String,
    pub password: Option<String>,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            host: String::new(),
            tailscale_ip: None,
            mac_address: None,
            broadcast_ip: Some("255.255.255.255".to_string()),
            wol_relay_host: None,
            ssh_port: 22,
            video_port: 9091,
            input_port: 9090,
            username: String::new(),
            password: None,
        }
    }
}

impl DeviceProfile {
    /// Normaliza espacios en blanco y aplica valores por defecto seguros en puertos/broadcast.
    pub fn sanitized(&self) -> Self {
        let clean_opt = |opt: &Option<String>| -> Option<String> {
            opt.as_ref()
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };

        let host = self.host.trim().to_string();
        let tailscale_ip = clean_opt(&self.tailscale_ip);
        let effective_host = if !host.is_empty() {
            host
        } else {
            tailscale_ip.clone().unwrap_or_default()
        };

        let name = self.name.trim().to_string();
        let fallback_name = if !name.is_empty() {
            name
        } else if !effective_host.is_empty() {
            effective_host.clone()
        } else {
            "Dispositivo Molex".to_string()
        };

        Self {
            id: self.id.trim().to_string(),
            name: fallback_name,
            host: effective_host,
            tailscale_ip,
            mac_address: clean_opt(&self.mac_address).map(|m| m.to_uppercase()),
            broadcast_ip: clean_opt(&self.broadcast_ip)
                .or_else(|| Some("255.255.255.255".to_string())),
            wol_relay_host: clean_opt(&self.wol_relay_host),
            ssh_port: if self.ssh_port == 0 { 22 } else { self.ssh_port },
            video_port: if self.video_port == 0 { 9091 } else { self.video_port },
            input_port: if self.input_port == 0 { 9090 } else { self.input_port },
            username: self.username.trim().to_string(),
            password: clean_opt(&self.password),
        }
    }

    /// Devuelve la IP/host preferido para conexiones activas (host primario o IP Tailscale).
    pub fn preferred_host(&self) -> String {
        let primary = self.host.trim();
        if !primary.is_empty() {
            primary.to_string()
        } else {
            self.tailscale_ip
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or_default()
                .to_string()
        }
    }

    /// Devuelve todos los hosts candidatos (sin duplicados) para sondeo WoL / conexión.
    pub fn candidate_hosts(&self) -> Vec<String> {
        let mut hosts = Vec::new();
        let primary = self.host.trim();
        if !primary.is_empty() {
            hosts.push(primary.to_string());
        }
        if let Some(ts) = self.tailscale_ip.as_deref().map(str::trim) {
            if !ts.is_empty() && !hosts.iter().any(|h| h == ts) {
                hosts.push(ts.to_string());
            }
        }
        hosts
    }

    /// Devuelve la dirección de broadcast configurada o `255.255.255.255`.
    pub fn effective_broadcast_ip(&self) -> &str {
        self.broadcast_ip
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("255.255.255.255")
    }
}

impl From<ServerProfile> for DeviceProfile {
    fn from(profile: ServerProfile) -> Self {
        let host = profile.host.trim().to_string();
        let tailscale_ip = if host.starts_with("100.") {
            Some(host.clone())
        } else {
            None
        };
        Self {
            id: format!("legacy-{}", host),
            name: host.clone(),
            host,
            tailscale_ip,
            mac_address: None,
            broadcast_ip: Some("255.255.255.255".to_string()),
            wol_relay_host: None,
            ssh_port: if profile.port == 0 { 22 } else { profile.port },
            video_port: 9091,
            input_port: 9090,
            username: profile.username.trim().to_string(),
            password: profile.password.filter(|p| !p.trim().is_empty()),
        }
    }
}

impl From<&DeviceProfile> for ServerProfile {
    fn from(device: &DeviceProfile) -> Self {
        Self {
            host: device.preferred_host(),
            port: device.ssh_port,
            username: device.username.clone(),
            password: device.password.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Record))]
pub struct SystemMetrics {
    pub os_info: String,
    pub ram_usage: String,
    pub cpu_load: String,
    pub gpu_info: String,
    pub network_status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Record))]
pub struct DesktopMetrics {
    pub os_info: String,
    pub hostname: String,
    pub uptime: String,
    pub cpu_model: String,
    pub cpu_percent: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_usage: String,
    pub gpu_info: String,
    pub gpu_temp_c: Option<f32>,
    pub gpu_util_percent: Option<f32>,
    pub gpu_vram_usage: Option<String>,
    pub network_status: String,
    pub compositor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Record))]
pub struct MonitorInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub focused: bool,
    pub compositor: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandResult {
    pub output: String,
    pub cwd: String,
    pub exit_code: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Record))]
pub struct TailscalePeer {
    pub id: String,
    pub hostname: String,
    pub dns_name: String,
    pub os: String,
    pub tailscale_ips: Vec<String>,
    pub primary_ip: Option<String>,
    pub online: bool,
    pub active: bool,
    pub cur_addr: String,
    pub relay: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Record))]
pub struct TailscaleStatus {
    pub backend_state: String,
    pub is_running: bool,
    pub auth_url: Option<String>,
    pub self_hostname: String,
    pub self_ips: Vec<String>,
    pub peers: Vec<TailscalePeer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeStage {
    PacketSent,
    WaitingForNetwork,
    VpnOnlineWaitingDaemon,
    Ready,
    Timeout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeProgress {
    pub device_id: String,
    pub stage: WakeStage,
    pub attempt: u32,
    pub max_attempts: u32,
    pub tailscale_online: bool,
    pub ssh_ready: bool,
    pub video_ready: bool,
    pub resolved_host: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WakeResult {
    pub device_id: String,
    pub resolved_host: String,
    pub via_tailscale: bool,
    pub ssh_ready: bool,
    pub video_ready: bool,
    pub elapsed_secs: u64,
}

#[derive(Debug, thiserror::Error, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Error))]
pub enum MolexError {
    #[error("Error de conexión: {0}")]
    Generic(String),
}
