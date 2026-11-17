use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use russh::client::{Config, Handle, Handler};
use russh::ChannelMsg;
use tokio::sync::Mutex;

use crate::models::{
    CommandResult, DesktopMetrics, DeviceProfile, MolexError, MonitorInfo, ServerProfile,
    SystemMetrics,
};
use crate::shell::{
    build_cd_probe_command, parse_cd_target, strip_ansi_codes, wrap_command_with_cwd,
};

pub static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub fn get_rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("Error al iniciar Tokio"))
}

pub struct MolexSshHandler;

#[async_trait::async_trait]
impl Handler for MolexSshHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &russh_keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Object))]
pub struct MolexVideoClient {
    pub profile: DeviceProfile,
    pub(crate) session: Mutex<Option<Arc<Mutex<Handle<MolexSshHandler>>>>>,
    pub(crate) last_frame_hash: Mutex<u64>,
    pub(crate) current_dir: Mutex<String>,
    pub(crate) prev_dir: Mutex<String>,
}

pub type SshClient = MolexVideoClient;

impl MolexVideoClient {
    pub fn from_device(profile: DeviceProfile) -> Arc<Self> {
        Arc::new(Self {
            profile: profile.sanitized(),
            session: Mutex::new(None),
            last_frame_hash: Mutex::new(0),
            current_dir: Mutex::new(String::new()),
            prev_dir: Mutex::new(String::new()),
        })
    }

    async fn authenticate_handle(
        &self,
        handle: &mut Handle<MolexSshHandler>,
    ) -> Result<bool, MolexError> {
        if let Some(ref pass) = self.profile.password {
            if !pass.is_empty() {
                let auth_future = handle.authenticate_password(&self.profile.username, pass);
                let ok = tokio::time::timeout(Duration::from_secs(10), auth_future)
                    .await
                    .map_err(|_| MolexError::Generic("Timeout autenticación".into()))?
                    .map_err(|e| MolexError::Generic(e.to_string()))?;
                if ok {
                    return Ok(true);
                }
            }
        }

        // Intentar llaves públicas SSH locales (~/.ssh/id_ed25519, id_ecdsa, id_rsa)
        if let Some(home) = std::env::var_os("HOME") {
            let ssh_dir = PathBuf::from(home).join(".ssh");
            for key_name in ["id_ed25519", "id_ecdsa", "id_rsa"] {
                let key_path = ssh_dir.join(key_name);
                if key_path.exists() {
                    if let Ok(key_pair) = russh_keys::load_secret_key(&key_path, None) {
                        let key_arc = Arc::new(key_pair);
                        let auth_future =
                            handle.authenticate_publickey(&self.profile.username, key_arc);
                        if let Ok(Ok(true)) =
                            tokio::time::timeout(Duration::from_secs(8), auth_future).await
                        {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }

    pub(crate) async fn get_session(&self) -> Result<Arc<Mutex<Handle<MolexSshHandler>>>, MolexError> {
        let mut session_guard = self.session.lock().await;

        if let Some(handle_arc) = session_guard.as_ref() {
            let is_closed = {
                let handle = handle_arc.lock().await;
                handle.is_closed()
            };
            if !is_closed {
                return Ok(handle_arc.clone());
            }
            *session_guard = None;
        }

        let host = self.profile.preferred_host();
        if host.is_empty() {
            return Err(MolexError::Generic(
                "El perfil no tiene host ni tailscale_ip configurado".into(),
            ));
        }

        let config = Arc::new(Config::default());
        let addr = format!("{}:{}", host, self.profile.ssh_port);

        let connect_future = russh::client::connect(config, addr, MolexSshHandler);
        let mut handle = tokio::time::timeout(Duration::from_secs(10), connect_future)
            .await
            .map_err(|_| MolexError::Generic("Timeout conectando IP".into()))?
            .map_err(|e| MolexError::Generic(e.to_string()))?;

        let auth_ok = self.authenticate_handle(&mut handle).await?;
        if !auth_ok {
            return Err(MolexError::Generic(
                "Autenticación SSH fallida (contraseña incorrecta o llave SSH no autorizada)"
                    .into(),
            ));
        }

        let handle_arc = Arc::new(Mutex::new(handle));
        *session_guard = Some(handle_arc.clone());

        Ok(handle_arc)
    }

    pub async fn connect_async(&self) -> Result<(), MolexError> {
        let _ = self.get_session().await?;
        let mut cwd = self.current_dir.lock().await;
        if cwd.is_empty() {
            if let Ok((out, Some(0))) = self.run_command_with_exit_code("pwd").await {
                let resolved = out.trim().to_string();
                if !resolved.is_empty() {
                    *cwd = resolved;
                }
            }
        }
        Ok(())
    }

    pub async fn disconnect_async(&self) {
        let mut session_guard = self.session.lock().await;
        if let Some(handle_arc) = session_guard.take() {
            let handle = handle_arc.lock().await;
            let _ = handle
                .disconnect(russh::Disconnect::ByApplication, "", "en")
                .await;
        }
    }

    pub async fn current_dir_async(&self) -> String {
        self.current_dir.lock().await.clone()
    }

    pub(crate) async fn run_command_with_exit_code(
        &self,
        cmd: &str,
    ) -> Result<(String, Option<u32>), MolexError> {
        let mut attempts = 0u8;
        let mut channel = loop {
            attempts += 1;
            let session_arc = self.get_session().await?;
            let open_res = {
                let session = session_arc.lock().await;
                session.channel_open_session().await
            };
            match open_res {
                Ok(ch) => break ch,
                Err(e) if attempts == 1 => {
                    let mut guard = self.session.lock().await;
                    *guard = None;
                    let _ = e;
                }
                Err(e) => return Err(MolexError::Generic(e.to_string())),
            }
        };

        channel
            .exec(true, cmd)
            .await
            .map_err(|e| MolexError::Generic(e.to_string()))?;

        let mut output = String::new();
        let mut exit_code = None;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => output.push_str(&String::from_utf8_lossy(data)),
                ChannelMsg::ExtendedData { ref data, .. } => {
                    output.push_str(&String::from_utf8_lossy(data))
                }
                ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                _ => {}
            }
        }

        let _ = channel.close().await;
        Ok((output, exit_code))
    }

    pub(crate) async fn run_command(&self, cmd: &str) -> Result<String, MolexError> {
        let (output, exit_code) = self.run_command_with_exit_code(cmd).await?;
        let trimmed = output.trim().to_string();
        if trimmed.is_empty() {
            Ok(format!("[No output - Exit Code: {:?}]", exit_code))
        } else {
            Ok(trimmed)
        }
    }

    pub async fn execute_command_async(&self, cmd: String) -> Result<CommandResult, MolexError> {
        let trimmed = cmd.trim();
        if trimmed.is_empty() {
            let cwd = self.current_dir.lock().await.clone();
            return Ok(CommandResult {
                output: String::new(),
                cwd,
                exit_code: Some(0),
            });
        }

        let mut cwd = self.current_dir.lock().await;
        let mut prev_dir = self.prev_dir.lock().await;

        if let Some(target) = parse_cd_target(trimmed) {
            let probe = build_cd_probe_command(&cwd, &prev_dir, &target);
            let (raw_output, exit_code) = self.run_command_with_exit_code(&probe).await?;
            let clean = strip_ansi_codes(raw_output.trim());
            if exit_code == Some(0) && !clean.is_empty() {
                if !cwd.is_empty() && *cwd != clean {
                    *prev_dir = cwd.clone();
                }
                *cwd = clean.clone();
                return Ok(CommandResult {
                    output: format!("Directorio actual: {}", clean),
                    cwd: clean,
                    exit_code,
                });
            } else {
                let err_msg = if clean.is_empty() {
                    format!(
                        "Error al cambiar al directorio '{}' (Exit Code: {:?})",
                        target, exit_code
                    )
                } else {
                    clean
                };
                return Ok(CommandResult {
                    output: err_msg,
                    cwd: cwd.clone(),
                    exit_code,
                });
            }
        }

        let wrapped = wrap_command_with_cwd(&cwd, trimmed);
        let (raw_output, exit_code) = self.run_command_with_exit_code(&wrapped).await?;
        let clean = strip_ansi_codes(raw_output.trim());
        let output = if clean.is_empty() {
            if exit_code == Some(0) {
                "Comando ejecutado con éxito".to_string()
            } else {
                format!("[Sin salida - Exit Code: {:?}]", exit_code)
            }
        } else {
            clean
        };

        Ok(CommandResult {
            output,
            cwd: cwd.clone(),
            exit_code,
        })
    }

    pub async fn get_desktop_metrics_async(&self) -> Result<DesktopMetrics, MolexError> {
        let script = concat!(
            "echo \"__OS__=$(uname -sr)\"; ",
            "echo \"__HOST__=$(hostname 2>/dev/null || uname -n)\"; ",
            "echo \"__UPTIME__=$(uptime -p 2>/dev/null | sed 's/^up //' || awk '{printf \"%dh %dm\", $1/3600, ($1%3600)/60}' /proc/uptime)\"; ",
            "echo \"__CPU_MODEL__=$(awk -F: '/model name/{gsub(/^ +/,\"\",$2); print $2; exit}' /proc/cpuinfo)\"; ",
            "CPU_PCT=$((grep '^cpu ' /proc/stat; sleep 0.12; grep '^cpu ' /proc/stat) | awk 'NR==1{u1=$2+$3+$4+$6+$7+$8; t1=u1+$5} NR==2{u2=$2+$3+$4+$6+$7+$8; t2=u2+$5; printf \"%.1f\", (t2>t1 ? ((u2-u1)*100.0)/(t2-t1) : 0)}'); ",
            "echo \"__CPU_PCT__=$CPU_PCT\"; ",
            "echo \"__MEM__=$(free -m | awk '/Mem:/ {print $3\" \"$2}')\"; ",
            "if command -v nvidia-smi >/dev/null 2>&1; then ",
            "  echo \"__GPU_NV__=$(nvidia-smi --query-gpu=name,temperature.gpu,utilization.gpu,memory.used,memory.total --format=csv,noheader,nounits 2>/dev/null | head -n 1)\"; ",
            "fi; ",
            "echo \"__GPU_PCI__=$(lspci 2>/dev/null | grep -iE 'vga|3d' | head -n 1 | cut -d':' -f3 | sed 's/^ //')\"; ",
            "echo \"__NET__=$(ip -br a 2>/dev/null | grep -E 'UP|UNKNOWN' | awk '{print $1\" \"$3}' | tr '\\n' ';')\"; ",
            "XDG_R=\"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}\"; ",
            "if [ -d \"$XDG_R/hypr\" ] && [ -n \"$(ls -A \"$XDG_R/hypr\" 2>/dev/null)\" ]; then COMP=\"Hyprland\"; ",
            "elif ls \"$XDG_R\"/niri.*.sock >/dev/null 2>&1; then COMP=\"Niri\"; ",
            "elif ls \"$XDG_R\"/wayland-[0-9]* >/dev/null 2>&1; then COMP=\"Wayland\"; ",
            "else COMP=\"Sin sesión Wayland\"; fi; ",
            "echo \"__COMP__=$COMP\""
        );

        let raw = self.run_command(script).await?;
        let clean = strip_ansi_codes(&raw);

        let mut os_info = "Linux".to_string();
        let mut hostname = self.profile.name.clone();
        let mut uptime = "Desconocido".to_string();
        let mut cpu_model = "CPU".to_string();
        let mut cpu_percent = 0.0f32;
        let mut ram_used_mb = 0u64;
        let mut ram_total_mb = 0u64;
        let mut gpu_nv: Option<String> = None;
        let mut gpu_pci = "GPU Desconocida".to_string();
        let mut network_status = "Red Desconocida".to_string();
        let mut compositor = "Wayland".to_string();

        for line in clean.lines() {
            if let Some(v) = line.strip_prefix("__OS__=") {
                if !v.trim().is_empty() {
                    os_info = v.trim().to_string();
                }
            } else if let Some(v) = line.strip_prefix("__HOST__=") {
                if !v.trim().is_empty() {
                    hostname = v.trim().to_string();
                }
            } else if let Some(v) = line.strip_prefix("__UPTIME__=") {
                if !v.trim().is_empty() {
                    uptime = v.trim().to_string();
                }
            } else if let Some(v) = line.strip_prefix("__CPU_MODEL__=") {
                if !v.trim().is_empty() {
                    cpu_model = v.trim().to_string();
                }
            } else if let Some(v) = line.strip_prefix("__CPU_PCT__=") {
                cpu_percent = v.trim().parse::<f32>().unwrap_or(0.0);
            } else if let Some(v) = line.strip_prefix("__MEM__=") {
                let mut parts = v.split_whitespace();
                ram_used_mb = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                ram_total_mb = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("__GPU_NV__=") {
                if !v.trim().is_empty() {
                    gpu_nv = Some(v.trim().to_string());
                }
            } else if let Some(v) = line.strip_prefix("__GPU_PCI__=") {
                if !v.trim().is_empty() {
                    gpu_pci = v.trim().to_string();
                }
            } else if let Some(v) = line.strip_prefix("__NET__=") {
                let formatted = v
                    .trim()
                    .trim_end_matches(';')
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                if !formatted.is_empty() {
                    network_status = formatted;
                }
            } else if let Some(v) = line.strip_prefix("__COMP__=") {
                if !v.trim().is_empty() {
                    compositor = v.trim().to_string();
                }
            }
        }

        let mut gpu_info = gpu_pci;
        let mut gpu_temp_c = None;
        let mut gpu_util_percent = None;
        let mut gpu_vram_usage = None;

        if let Some(nv) = gpu_nv {
            let cols: Vec<&str> = nv.split(',').map(str::trim).collect();
            if cols.len() >= 5 {
                if !cols[0].is_empty() {
                    gpu_info = cols[0].to_string();
                }
                gpu_temp_c = cols[1].parse::<f32>().ok();
                gpu_util_percent = cols[2].parse::<f32>().ok();
                if let (Ok(used), Ok(total)) = (cols[3].parse::<u64>(), cols[4].parse::<u64>()) {
                    gpu_vram_usage = Some(format!("{}MB / {}MB", used, total));
                }
            }
        }

        let ram_usage = format!("{}MB / {}MB", ram_used_mb, ram_total_mb);

        Ok(DesktopMetrics {
            os_info,
            hostname,
            uptime,
            cpu_model,
            cpu_percent,
            ram_used_mb,
            ram_total_mb,
            ram_usage,
            gpu_info,
            gpu_temp_c,
            gpu_util_percent,
            gpu_vram_usage,
            network_status,
            compositor,
        })
    }

    pub async fn get_system_metrics_async(&self) -> Result<SystemMetrics, MolexError> {
        let dm = self.get_desktop_metrics_async().await?;
        let gpu_summary = match (dm.gpu_temp_c, dm.gpu_util_percent, dm.gpu_vram_usage) {
            (Some(temp), Some(util), Some(vram)) => {
                format!("{} ({:.0}% · {:.0}°C · {})", dm.gpu_info, util, temp, vram)
            }
            _ => dm.gpu_info,
        };
        Ok(SystemMetrics {
            os_info: dm.os_info,
            ram_usage: dm.ram_usage,
            cpu_load: format!("{:.1}%", dm.cpu_percent),
            gpu_info: gpu_summary,
            network_status: dm.network_status,
        })
    }

    pub async fn get_monitors_async(&self) -> Result<Vec<String>, MolexError> {
        self.fetch_monitors_internal().await
    }

    pub async fn get_monitors_detailed_async(&self) -> Result<Vec<MonitorInfo>, MolexError> {
        self.fetch_monitors_detailed_internal().await
    }
}

// Bloque de exportación FFI condicional para Android (UniFFI)
#[cfg_attr(feature = "uniffi-bindings", uniffi::export)]
impl MolexVideoClient {
    #[cfg_attr(feature = "uniffi-bindings", uniffi::constructor)]
    pub fn new(profile: ServerProfile) -> Arc<Self> {
        Self::from_device(profile.into())
    }

    pub fn get_system_metrics(&self) -> Result<SystemMetrics, MolexError> {
        get_rt().block_on(self.get_system_metrics_async())
    }

    pub fn execute_command(&self, cmd: String) -> String {
        get_rt().block_on(async {
            match self.execute_command_async(cmd).await {
                Ok(res) => res.output,
                Err(e) => format!("Error SSH: {}", e),
            }
        })
    }

    pub fn get_screen_frame(&self, monitor_name: Option<String>) -> Result<String, MolexError> {
        get_rt().block_on(self.capture_frame_internal(monitor_name))
    }

    pub fn get_monitors(&self) -> Result<Vec<String>, MolexError> {
        get_rt().block_on(self.fetch_monitors_internal())
    }
}
