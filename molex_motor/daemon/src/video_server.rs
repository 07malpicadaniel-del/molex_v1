use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;
use tokio::process::Command;
use tokio::sync::watch;

pub async fn start_video_server(
    addr: &str,
    target_output: Arc<tokio::sync::RwLock<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(addr).await?;
    println!("📺 Molex Video TCP Server transmitiendo en {}", addr);

    // Canal con capacidad para almacenar únicamente el ÚLTIMO frame capturado
    let (tx, rx) = watch::channel::<Option<Vec<u8>>>(None);

    // Tarea 1: Bucle de captura continua en memoria
    let output_for_capture = target_output.clone();
    tokio::spawn(async move {
        let frame_interval = Duration::from_millis(16); // Límite ~60 FPS

        loop {
            let start = Instant::now();
            let monitor = output_for_capture.read().await.clone();

            // grim con calidad optimizada y escala 0.75 para reducir ancho de banda
            let grim_output = Command::new("grim")
                .args(["-t", "jpeg", "-q", "55", "-s", "0.75", "-o", &monitor, "-"])
                .output()
                .await;

            if let Ok(out) = grim_output {
                if out.status.success() && !out.stdout.is_empty() {
                    // Sobrescribe el buffer; los frames no leídos se descartan en el acto
                    let _ = tx.send(Some(out.stdout));
                }
            }

            let elapsed = start.elapsed();
            if elapsed < frame_interval {
                tokio::time::sleep(frame_interval - elapsed).await;
            }
        }
    });

    // Tarea 2: Servidor que solo despacha el frame más fresco a la red
    loop {
        let (mut socket, peer) = listener.accept().await?;
        let _ = socket.set_nodelay(true);
        println!("🎥 Cliente conectado: {}", peer);

        let mut client_rx = rx.clone();

        tokio::spawn(async move {
            while client_rx.changed().await.is_ok() {
                let frame_bytes = match client_rx.borrow().as_ref() {
                    Some(bytes) => bytes.clone(),
                    None => continue,
                };

                let frame_len = frame_bytes.len() as u32;

                // Si la red se atora, no se acumula cola; se salta al siguiente
                if socket.write_all(&frame_len.to_be_bytes()).await.is_err() {
                    break;
                }
                if socket.write_all(&frame_bytes).await.is_err() {
                    break;
                }
                if socket.flush().await.is_err() {
                    break;
                }
            }
            println!("🔌 Cliente desconectado: {}", peer);
        });
    }
}
