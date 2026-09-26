mod udp_server;
mod video_server;
mod virtual_device;

use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Iniciando Molex Daemon v2 (Zero-Latency Engine)...");

    // Monitor por defecto configurado a tu pantalla principal ultrawide (DP-4)
    let active_output = Arc::new(RwLock::new("DP-4".to_string()));

    let input_task = tokio::spawn(async {
        if let Err(e) = udp_server::start_server("0.0.0.0:9090").await {
            eprintln!("❌ Error en servidor UDP de Input: {}", e);
        }
    });

    let active_output_clone = active_output.clone();
    let video_task = tokio::spawn(async move {
        if let Err(e) = video_server::start_video_server("0.0.0.0:9091", active_output_clone).await {
            eprintln!("❌ Error en servidor TCP de Video: {}", e);
        }
    });

    tokio::select! {
        _ = input_task => {},
        _ = video_task => {},
    }

    Ok(())
}
