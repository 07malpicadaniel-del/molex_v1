use russh::client::{Config, Handle, Handler};
use russh::ChannelMsg;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use std::time::Duration;

// Importamos nuestros modelos separados
use crate::models::{ServerProfile, SystemMetrics, MolexError};

pub static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub fn get_rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("Error al iniciar Tokio"))
}

pub struct MolexSshHandler;

#[async_trait::async_trait]
impl Handler for MolexSshHandler {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _server_public_key: &russh_keys::key::PublicKey) -> Result<bool, Self::Error> {
        Ok(true) 
    }
}

#[derive(uniffi::Object)]
pub struct MolexVideoClient {
    pub profile: ServerProfile,
    // pub(crate) permite que wm.rs acceda a estas variables
    pub(crate) session: Mutex<Option<Arc<Mutex<Handle<MolexSshHandler>>>>>,
    pub(crate) last_frame_hash: Mutex<u64>,
}

impl MolexVideoClient {
    pub(crate) async fn get_session(&self) -> Result<Arc<Mutex<Handle<MolexSshHandler>>>, MolexError> {
        let mut session_guard = self.session.lock().await;
        
        if let Some(handle_arc) = session_guard.as_ref() {
            return Ok(handle_arc.clone());
        }

        let pass = self.profile.password.clone().unwrap_or_default();
        let config = Arc::new(Config::default());
        let addr = format!("{}:{}", self.profile.host, self.profile.port);
        
        let connect_future = russh::client::connect(config, addr, MolexSshHandler);
        let mut handle = tokio::time::timeout(Duration::from_secs(10), connect_future)
            .await.map_err(|_| MolexError::Generic("Timeout conectando IP".into()))?
            .map_err(|e| MolexError::Generic(e.to_string()))?;

        let auth_future = handle.authenticate_password(&self.profile.username, pass);
        let auth_res = tokio::time::timeout(Duration::from_secs(10), auth_future)
            .await.map_err(|_| MolexError::Generic("Timeout autenticación".into()))?
            .map_err(|e| MolexError::Generic(e.to_string()))?;

        if !auth_res {
            return Err(MolexError::Generic("Contraseña incorrecta".into()));
        }

        let handle_arc = Arc::new(Mutex::new(handle));
        *session_guard = Some(handle_arc.clone());
        
        Ok(handle_arc)
    }

    pub(crate) async fn run_command(&self, cmd: &str) -> Result<String, MolexError> {
        let mut channel = {
            let session_arc = self.get_session().await?;
            let session = session_arc.lock().await;
            session.channel_open_session().await.map_err(|e| MolexError::Generic(e.to_string()))?
        }; 

        channel.exec(true, cmd).await.map_err(|e| MolexError::Generic(e.to_string()))?;

        let mut output = String::new();
        let mut exit_code = None;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => output.push_str(&String::from_utf8_lossy(data)),
                ChannelMsg::ExtendedData { ref data, .. } => output.push_str(&String::from_utf8_lossy(data)),
                ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                _ => {}
            }
        }
        
        let _ = channel.close().await;

        let trimmed = output.trim().to_string();
        if trimmed.is_empty() {
            Ok(format!("[No output - Exit Code: {:?}]", exit_code))
        } else {
            Ok(trimmed)
        }
    }
}

// Bloque de exportación FFI
#[uniffi::export]
impl MolexVideoClient {
    #[uniffi::constructor]
    pub fn new(profile: ServerProfile) -> Arc<Self> {
        Arc::new(Self {
            profile,
            session: Mutex::new(None),
            last_frame_hash: Mutex::new(0),
        })
    }

    pub fn get_system_metrics(&self) -> Result<SystemMetrics, MolexError> {
        get_rt().block_on(async {
            let os_info = self.run_command("uname -sr").await.unwrap_or_else(|_| "Desconocido".into());
            let ram_usage = self.run_command("free -m | awk '/Mem:/ {print $3\"MB / \"$2\"MB\"}'").await.unwrap_or_else(|_| "0MB / 0MB".into());
            let cpu_load = self.run_command("top -bn1 | grep 'Cpu(s)' | awk '{print $2 + $4\"%\"}'").await.unwrap_or_else(|_| "0%".into());
            let gpu_info = self.run_command("lspci | grep -iE 'vga|3d' | cut -d':' -f3 | sed 's/^ //'").await.unwrap_or_else(|_| "GPU Desconocida".into());
            let network_status = self.run_command("ip -br a | grep UP | awk '{print $1, $3}'").await.unwrap_or_else(|_| "Red Desconocida".into());

            Ok(SystemMetrics { os_info, ram_usage, cpu_load, gpu_info, network_status })
        })
    }

    pub fn execute_command(&self, cmd: String) -> String {
        get_rt().block_on(async {
            match self.run_command(&cmd).await {
                Ok(output) => {
                    if output.starts_with("[No output") {
                        "Comando ejecutado con éxito".to_string() 
                    } else {
                        output
                    }
                },
                Err(e) => format!("Error SSH: {}", e),
            }
        })
    }

    // --- DELEGACIÓN A WM.RS ---
    pub fn get_screen_frame(&self, monitor_name: Option<String>) -> Result<String, MolexError> {
        get_rt().block_on(async {
            self.capture_frame_internal(monitor_name).await
        })
    }

    pub fn get_monitors(&self) -> Result<Vec<String>, MolexError> {
        get_rt().block_on(async {
            self.fetch_monitors_internal().await
        })
    }
}
