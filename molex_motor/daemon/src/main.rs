mod udp_server;
mod video_server;
mod virtual_device;

use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Iniciando Molex Daemon v2 (Zero-Latency Engine)...");

    let active_output = Arc::new(RwLock::new("DP-4".to_string()));

    let input_task = tokio::spawn(async {
        loop {
            let failed = match udp_server::start_server("0.0.0.0:9090").await {
                Ok(_) => false,
                Err(e) => {
                    eprintln!("❌ Error en servidor UDP de Input: {}", e);
                    true
                }
            };

            if failed {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    });

    let active_output_clone = active_output.clone();
    let video_task = tokio::spawn(async move {
        loop {
            let failed = match video_server::start_video_server("0.0.0.0:9091", active_output_clone.clone()).await {
                Ok(_) => false,
                Err(e) => {
                    eprintln!("❌ Error en servidor TCP de Video: {}", e);
                    true
                }
            };

            if failed {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
    });

    let _ = tokio::join!(input_task, video_task);

    Ok(())
}
