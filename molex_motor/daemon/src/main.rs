// Declaramos los módulos que acabamos de crear
mod virtual_device;
mod udp_server;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🚀 Iniciando Molex Daemon (Root)...");
    
    // Arrancamos el servidor UDP y bloqueamos el hilo para que se quede escuchando
    if let Err(e) = udp_server::run_server().await {
        eprintln!("❌ Error fatal en el servidor: {}", e);
    }

    Ok(())
}
