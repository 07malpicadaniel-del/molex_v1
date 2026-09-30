use std::process::Stdio;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::process::Command;

pub async fn start_video_server(
    addr: &str,
    target_output: Arc<tokio::sync::RwLock<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    println!("📺 Molex H.264 NVENC Video Server listo en {}", addr);

    loop {
        let (mut socket, peer) = listener.accept().await?;
        let _ = socket.set_nodelay(true);
        println!("🎥 Cliente Android conectado: {}", peer);

        let output_ref = target_output.clone();

        tokio::spawn(async move {
            let monitor = output_ref.read().await.clone();

            let mut child = match Command::new("wl-screenrec")
                .args([
                    "--experimental-vulkan",
                    "--low-power=off",
                    "-o",
                    &monitor,
                    "--codec",
                    "avc",
                    "--ffmpeg-muxer",
                    "h264",
                    "--gop-size",
                    "30",
                    "--max-fps",
                    "60",
                    "-f",
                    "-",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit()) // Muestra cualquier log de wl-screenrec en la terminal
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
                    return;
                }
            };

            let copy_result = tokio::io::copy(&mut stdout, &mut socket).await;

            if let Err(e) = copy_result {
                eprintln!("ℹ️ Conexión de video finalizada ({}): {}", peer, e);
            }

            let _ = child.kill().await;
            println!("🔌 Sesión H.264 cerrada con {}", peer);
        });
    }
}
