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
    pub gpu_info: String,       
    pub network_status: String, 
}

#[derive(Debug, uniffi::Error, thiserror::Error)]
pub enum MolexError {
    #[error("Error de conexión: {0}")]
    Generic(String),
}
