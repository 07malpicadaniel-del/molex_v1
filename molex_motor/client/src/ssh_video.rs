use russh::client::{Config, Handle, Handler};
use russh::ChannelMsg;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;
use std::time::Duration;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// Instancia global de Tokio para ejecutar funciones asíncronas desde Kotlin
static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn get_rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| tokio::runtime::Runtime::new().expect("Error al iniciar Tokio"))
}

// ==========================================
// 🧩 MODELOS DE DATOS (Exportados a Kotlin)
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
}

#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum MolexError {
    #[error("Error de conexión: {0}")]
    Generic(String),
}

// ==========================================
// 🔌 MOTOR SSH (Configuración Interna)
// ==========================================

pub struct MolexSshHandler;

#[async_trait::async_trait]
impl Handler for MolexSshHandler {
    type Error = russh::Error;
    async fn check_server_key(&mut self, _server_public_key: &russh_keys::key::PublicKey) -> Result<bool, Self::Error> {
        Ok(true) // Aceptamos cualquier llave para máxima velocidad
    }
}

// ==========================================
// 📡 CLIENTE DE VIDEO Y MÉTRICAS (UniFFI)
// ==========================================

#[derive(uniffi::Object)]
pub struct MolexVideoClient {
    pub profile: ServerProfile,
    // SOLUCIÓN: Envolvemos el Handle en Arc<Mutex<...>> para poder compartirlo y clonarlo con seguridad
    session: Mutex<Option<Arc<Mutex<Handle<MolexSshHandler>>>>>,
    last_frame_hash: Mutex<u64>,
}

impl MolexVideoClient {
    // Obtiene la sesión actual o crea una nueva si no existe
    async fn get_session(&self) -> Result<Arc<Mutex<Handle<MolexSshHandler>>>, MolexError> {
        let mut session_guard = self.session.lock().await;
        
        // Si ya hay una sesión activa, devolvemos un clon del puntero (Arc)
        if let Some(handle_arc) = session_guard.as_ref() {
            return Ok(handle_arc.clone());
        }

        let pass = self.profile.password.clone().unwrap_or_default();
        let config = Arc::new(Config::default());
        let addr = format!("{}:{}", self.profile.host, self.profile.port);
        
        let connect_future = russh::client::connect(config, addr, MolexSshHandler);
        let mut handle = tokio::time::timeout(Duration::from_secs(3), connect_future)
            .await.map_err(|_| MolexError::Generic("Timeout conectando IP".into()))?
            .map_err(|e| MolexError::Generic(e.to_string()))?;

        let auth_future = handle.authenticate_password(&self.profile.username, pass);
        let auth_res = tokio::time::timeout(Duration::from_secs(3), auth_future)
            .await.map_err(|_| MolexError::Generic("Timeout autenticación".into()))?
            .map_err(|e| MolexError::Generic(e.to_string()))?;

        if !auth_res {
            return Err(MolexError::Generic("Contraseña incorrecta".into()));
        }

        // Guardamos la nueva conexión en el puntero inteligente
        let handle_arc = Arc::new(Mutex::new(handle));
        *session_guard = Some(handle_arc.clone());
        
        Ok(handle_arc)
    }

    // Ejecutador universal de comandos bash
    async fn run_command(&self, cmd: &str) -> Result<String, MolexError> {
        let session_arc = self.get_session().await?;
        let session = session_arc.lock().await; // Bloqueamos el Mutex para usar la sesión SSH
        
        let mut channel = session.channel_open_session().await.map_err(|e| MolexError::Generic(e.to_string()))?;
        channel.exec(true, cmd).await.map_err(|e| MolexError::Generic(e.to_string()))?;

        let mut output = String::new();
        while let Some(msg) = channel.wait().await {
            if let ChannelMsg::Data { data } = msg {
                output.push_str(&String::from_utf8_lossy(&data));
            }
        }
        Ok(output.trim().to_string())
    }
}

// Métodos públicos que Kotlin podrá utilizar
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
            
            Ok(SystemMetrics { os_info, ram_usage, cpu_load })
        })
    }

    pub fn get_screen_frame(&self) -> Result<String, MolexError> {
        get_rt().block_on(async {
            // Utilizamos 'grim' que es el capturador nativo de Wayland
            let cmd = "WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/1000 grim -t jpeg -q 30 - | base64 -w 0";
            let base64_image = self.run_command(cmd).await?;

            if base64_image.is_empty() {
                return Err(MolexError::Generic("Error capturando pantalla (¿grim está instalado?)".into()));
            }

            // Algoritmo LTPO
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
}
