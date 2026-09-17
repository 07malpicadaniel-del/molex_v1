use russh::client::{Config, Handle, Handler};
use russh::ChannelMsg;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use std::time::Duration;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Instancia global de Tokio para ejecutar funciones asíncronas desde Kotlin FFI
static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn get_rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("Error al iniciar Tokio"))
}

// ==========================================
//  MODELOS DE DATOS (Exportados a Kotlin)
// ==========================================

#[derive(Clone, uniffi::Record)]
pub struct ServerProfile {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
}

#[derive(uniffi::Record)]
pub struct SystemMetrics {
    pub os_info: String,
    pub ram_usage: String,
    pub cpu_load: String,
    pub gpu_info: String,       // NUEVO
    pub network_status: String, // NUEVO
}

#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum MolexError {
    #[error("Error de conexión: {0}")]
    Generic(String),
}

// ==========================================
//  MOTOR SSH (Configuración Interna)
// ==========================================

pub struct MolexSshHandler;

#[async_trait::async_trait]
impl Handler for MolexSshHandler {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _server_public_key: &russh_keys::key::PublicKey) -> Result<bool, Self::Error> {
        Ok(true) // Aceptamos cualquier llave para máxima velocidad en LAN
    }
}

// ==========================================
//  CLIENTE DE VIDEO Y MÉTRICAS (UniFFI)
// ==========================================

#[derive(uniffi::Object)]
pub struct MolexVideoClient {
    pub profile: ServerProfile,
    session: Mutex<Option<Arc<Mutex<Handle<MolexSshHandler>>>>>,
    last_frame_hash: Mutex<u64>,
}

// Métodos internos asíncronos (No exportados)
impl MolexVideoClient {
    // Obtiene la sesión actual o crea una nueva si no existe
    async fn get_session(&self) -> Result<Arc<Mutex<Handle<MolexSshHandler>>>, MolexError> {
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

    // Ejecutador universal de comandos bash
    async fn run_command(&self, cmd: &str) -> Result<String, MolexError> {
        let mut channel = {
            let session_arc = self.get_session().await?;
            // 1. Aquí corregimos el warning quitando "mut"
            let session = session_arc.lock().await;
            session.channel_open_session().await.map_err(|e| MolexError::Generic(e.to_string()))?
        }; // Se libera el Mutex inmediatamente.

        channel.exec(true, cmd).await.map_err(|e| MolexError::Generic(e.to_string()))?;

        let mut output = String::new();
        let mut exit_code = None;

        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { ref data } => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                // 2. TRAMPA: Errores (Stderr)
                ChannelMsg::ExtendedData { ref data, .. } => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                // 3. TRAMPA: Código de finalización (Exit Code)
                ChannelMsg::ExitStatus { exit_status } => {
                    exit_code = Some(exit_status);
                }
                _ => {}
            }
        }

        let trimmed = output.trim().to_string();
        
        // 4. Si Linux no imprimió texto, enviamos nuestra trampa.
        if trimmed.is_empty() {
            Ok(format!("[No output - Exit Code: {:?}]", exit_code))
        } else {
            Ok(trimmed)
        }
    }
}

// Métodos públicos que Kotlin podrá utilizar FFI
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
            
            // Extracción de GPU (Filtra VGA o 3D controller)
            let gpu_info = self.run_command("lspci | grep -iE 'vga|3d' | cut -d':' -f3 | sed 's/^ //'").await.unwrap_or_else(|_| "GPU Desconocida".into());
            
            // Estado de red (Interfaces activas IP)
            let network_status = self.run_command("ip -br a | grep UP | awk '{print $1, $3}'").await.unwrap_or_else(|_| "Red Desconocida".into());

            Ok(SystemMetrics { os_info, ram_usage, cpu_load, gpu_info, network_status })
        })
    }

    pub fn get_screen_frame(&self) -> Result<String, MolexError> {
        get_rt().block_on(async {
            // Utilizamos 'grim' con las variables de Wayland asignadas al usuario
            let cmd = "WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/1000 grim -t jpeg -q 30 - | base64 -w 0";
            let base64_image = self.run_command(cmd).await?;

            if base64_image.is_empty() || base64_image.starts_with("[No output") {
                return Err(MolexError::Generic(format!("Error capturando pantalla: {}", base64_image)));
            }

            // Algoritmo LTPO de hash
            let mut hasher = DefaultHasher::new();
            base64_image.hash(&mut hasher);
            let current_hash = hasher.finish();

            let mut last_hash = self.last_frame_hash.lock().await;
            if *last_hash == current_hash {
                return Ok("SAME_FRAME".to_string());
            }
            
            *last_hash = current_hash;
            Ok(base64_image)
        })
    }

    pub fn execute_command(&self, cmd: String) -> String {
        get_rt().block_on(async {
            match self.run_command(&cmd).await {
                Ok(output) => output,
                Err(e) => format!("Error SSH: {}", e),
            }
        })
    }
}
