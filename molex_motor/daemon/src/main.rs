mod udp_server;
mod virtual_device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Iniciando Molex Daemon (Kernel Injector)...");
    
    // Escuchamos en el puerto 9090 (El que configuramos ayer en el cliente UDP)
    udp_server::start_server("0.0.0.0:9090").await?;
    
    Ok(())
}
