uniffi::setup_scaffolding!();

// Importamos nuestros dos módulos separados
pub mod ssh_video;
pub mod udp_input;

// Los exponemos para que UniFFI pueda generar el puente de Kotlin
pub use ssh_video::*;
pub use udp_input::*;
