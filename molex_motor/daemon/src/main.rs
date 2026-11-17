mod monitor;
mod udp_server;
mod video_server;
mod virtual_device;

use std::sync::Arc;
use tokio::sync::{watch, RwLock};

use monitor::{detect_monitors, select_best_monitor, DaemonConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = DaemonConfig::from_env_and_args();
    println!("🚀 Iniciando Molex Host Daemon (PC-to-PC Engine)...");

    let monitors = detect_monitors();
    if monitors.is_empty() {
        println!("⚠️  No se detectaron salidas Wayland activas aún; usando fallback.");
    } else {
        for m in &monitors {
            println!(
                "   • [{}] {} ({}x{}@{}Hz){}",
                m.compositor,
                m.name,
                m.width,
                m.height,
                m.refresh_hz,
                if m.focused { " [focused]" } else { "" }
            );
        }
    }

    let chosen_name = match config.monitor.clone() {
        Some(explicit) => explicit,
        None => select_best_monitor(&monitors).name,
    };

    println!("🖥️  Monitor seleccionado para streaming: {}", chosen_name);

    let active_output = Arc::new(RwLock::new(chosen_name));
    let (monitor_tx, monitor_rx) = watch::channel(0u64);

    let input_addr = format!("0.0.0.0:{}", config.input_port);
    let input_output = active_output.clone();
    let input_task = tokio::spawn(async move {
        loop {
            let failed = match udp_server::start_server(
                &input_addr,
                input_output.clone(),
                monitor_tx.clone(),
            )
            .await
            {
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

    let video_addr = format!("0.0.0.0:{}", config.video_port);
    let video_output = active_output.clone();
    let video_task = tokio::spawn(async move {
        loop {
            let failed = match video_server::start_video_server(
                &video_addr,
                video_output.clone(),
                monitor_rx.clone(),
                config.clone(),
            )
            .await
            {
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
