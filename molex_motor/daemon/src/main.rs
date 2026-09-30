mod udp_server;
mod video_server;
mod virtual_device;

use std::sync::Arc;
use tokio::sync::RwLock;

fn detect_ultrawide_monitor() -> String {
    if let Ok(output) = std::process::Command::new("hyprctl")
        .arg("monitors")
        .output()
    {
        let text = String::from_utf8_lossy(&output.stdout);
        let mut current_monitor = String::new();

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Monitor ") {
                if let Some(name) = trimmed.split_whitespace().nth(1) {
                    current_monitor = name.to_string();
                }
            }
            // Selecciona automáticamente la pantalla de 34" Ultrawide
            if trimmed.contains("2560x1080") && !current_monitor.is_empty() {
                return current_monitor;
            }
        }

        if !current_monitor.is_empty() {
            return current_monitor;
        }
    }

    "DP-1".to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Iniciando Molex Daemon v2 (Zero-Latency Engine)...");

    let detected_monitor = detect_ultrawide_monitor();
    println!("🖥️  Monitor detectado para streaming: {}", detected_monitor);

    let active_output = Arc::new(RwLock::new(detected_monitor));

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
