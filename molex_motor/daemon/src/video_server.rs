use std::io::ErrorKind;
use std::process::Stdio;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::{watch, RwLock};

use crate::monitor::DaemonConfig;

fn is_clean_disconnect(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::UnexpectedEof
    )
}

pub async fn start_video_server(
    addr: &str,
    target_output: Arc<RwLock<String>>,
    monitor_rx: watch::Receiver<u64>,
    config: DaemonConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    println!(
        "📺 Molex H.264 Video Server listo en {} (codec={}, gop={}, max_fps={}, experimental_vulkan={})",
        addr, config.codec, config.gop_size, config.fps, config.experimental_vulkan
    );

    loop {
        let (mut socket, peer) = listener.accept().await?;
        let _ = socket.set_nodelay(true);
        println!("🎥 Cliente conectado al stream H.264: {}", peer);

        let output_ref = target_output.clone();
        let mut rx = monitor_rx.clone();
        let session_cfg = config.clone();

        tokio::spawn(async move {
            let monitor = output_ref.read().await.clone();
            let gop_str = session_cfg.gop_size.to_string();
            let fps_str = session_cfg.fps.to_string();

            let mut args: Vec<String> = Vec::new();
            if session_cfg.experimental_vulkan {
                args.push("--experimental-vulkan".to_string());
            }
            if let Some(ref low_power) = session_cfg.low_power {
                args.push(format!("--low-power={}", low_power));
            }
            args.extend([
                "-o".to_string(),
                monitor.clone(),
                "--codec".to_string(),
                session_cfg.codec.clone(),
                "--ffmpeg-muxer".to_string(),
                "h264".to_string(),
                "--gop-size".to_string(),
                gop_str,
                "--max-fps".to_string(),
                fps_str,
                "-f".to_string(),
                "/dev/stdout".to_string(),
            ]);

            println!("🎬 Lanzando wl-screenrec en monitor '{}': {:?}", monitor, args);

            let mut child = match Command::new("wl-screenrec")
                .args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .kill_on_drop(true)
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    eprintln!("❌ Error al iniciar wl-screenrec: {}", e);
                    return;
                }
            };

            let mut stdout = match child.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    eprintln!("❌ No se pudo capturar stdout de wl-screenrec");
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    return;
                }
            };

            tokio::select! {
                copy_res = tokio::io::copy(&mut stdout, &mut socket) => {
                    match copy_res {
                        Ok(bytes) => {
                            println!("ℹ️ Stream H.264 finalizado limpiamente con {} ({} bytes)", peer, bytes);
                        }
                        Err(e) if is_clean_disconnect(&e) => {
                            println!("ℹ️ Cliente {} desconectado limpiamente ({})", peer, e.kind());
                        }
                        Err(e) => {
                            eprintln!("⚠️ Conexión de video cerrada ({}): {}", peer, e);
                        }
                    }
                }
                Ok(_) = rx.changed() => {
                    println!("🔄 Cambio de monitor solicitado para {}; reiniciando captura", peer);
                }
            }

            let _ = child.kill().await;
            let _ = child.wait().await;
            println!("🔌 Sesión H.264 cerrada con {}", peer);
        });
    }
}
