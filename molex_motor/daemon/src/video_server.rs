use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::net::{TcpListener, TcpStream};
use tokio::process::Command;
use tokio::sync::{watch, RwLock};

use crate::monitor::DaemonConfig;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriSelection {
    pub device_path: String,
    pub vendor: Option<String>,
    pub libva_driver: Option<String>,
}

fn is_clean_disconnect(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        ErrorKind::BrokenPipe
            | ErrorKind::ConnectionReset
            | ErrorKind::ConnectionAborted
            | ErrorKind::UnexpectedEof
    )
}

fn read_dri_vendor(drm_sys_dir: &Path, node_name: &str) -> Option<String> {
    let vendor_path = drm_sys_dir.join(node_name).join("device/vendor");
    std::fs::read_to_string(vendor_path)
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
}

/// Detecta nodos `/dev/dri/renderD*` priorizando `/dev/dri/renderD128` (iGPU Intel/AMD)
/// y selecciona `LIBVA_DRIVER_NAME=iHD` cuando el vendedor es Intel (`0x8086`) y no hay override.
pub fn detect_dri_device_from(
    dri_dir: &Path,
    drm_sys_dir: &Path,
    existing_libva_driver: Option<&str>,
) -> Option<DriSelection> {
    let entries = std::fs::read_dir(dri_dir).ok()?;
    let mut nodes: Vec<(String, PathBuf, Option<String>)> = entries
        .filter_map(|res| res.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("renderD") {
                return None;
            }
            let path = entry.path();
            let vendor = read_dri_vendor(drm_sys_dir, &name);
            Some((name, path, vendor))
        })
        .collect();

    if nodes.is_empty() {
        return None;
    }

    // Prioridad:
    // 1) renderD128 (si existe; en laptops híbridas controla la pantalla Wayland)
    // 2) Cualquier iGPU Intel (0x8086) o AMD (0x1002)
    // 3) Primer renderD* ordenado alfabéticamente
    fn dri_rank(item: &(String, PathBuf, Option<String>)) -> (u8, &str) {
        if item.0 == "renderD128" {
            (0, item.0.as_str())
        } else if matches!(item.2.as_deref(), Some("0x8086") | Some("0x1002")) {
            (1, item.0.as_str())
        } else {
            (2, item.0.as_str())
        }
    }

    nodes.sort_by(|a, b| dri_rank(a).cmp(&dri_rank(b)));

    let (_name, path, vendor) = nodes.into_iter().next()?;
    let env_unset = existing_libva_driver
        .map(|v| v.trim().is_empty())
        .unwrap_or(true);
    let libva_driver = if vendor.as_deref() == Some("0x8086") && env_unset {
        Some("iHD".to_string())
    } else {
        None
    };

    Some(DriSelection {
        device_path: path.to_string_lossy().to_string(),
        vendor,
        libva_driver,
    })
}

pub fn detect_dri_device() -> Option<DriSelection> {
    let existing = std::env::var("LIBVA_DRIVER_NAME").ok();
    detect_dri_device_from(
        Path::new("/dev/dri"),
        Path::new("/sys/class/drm"),
        existing.as_deref(),
    )
}

fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

enum AttemptOutcome {
    StreamedOrClientClosed,
    EarlyZeroBytes,
    MonitorSwitched,
    SpawnFailed(String),
}

async fn run_encoder_process(
    program: &str,
    args: &[String],
    libva_driver: Option<&str>,
    socket: &mut TcpStream,
    rx: &mut watch::Receiver<u64>,
    peer: std::net::SocketAddr,
) -> AttemptOutcome {
    let started = Instant::now();
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);

    if let Some(driver) = libva_driver {
        cmd.env("LIBVA_DRIVER_NAME", driver);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => return AttemptOutcome::SpawnFailed(e.to_string()),
    };

    let mut stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return AttemptOutcome::SpawnFailed("No se pudo capturar stdout".into());
        }
    };

    let outcome = tokio::select! {
        copy_res = tokio::io::copy(&mut stdout, socket) => {
            let elapsed = started.elapsed();
            match copy_res {
                Ok(0) => {
                    println!(
                        "⚠️  {} terminó sin emitir datos (0 bytes en {} ms)",
                        program,
                        elapsed.as_millis()
                    );
                    if elapsed < Duration::from_millis(1500) {
                        AttemptOutcome::EarlyZeroBytes
                    } else {
                        AttemptOutcome::StreamedOrClientClosed
                    }
                }
                Ok(bytes) => {
                    println!("ℹ️ Stream H.264 finalizado limpiamente con {} ({} bytes)", peer, bytes);
                    AttemptOutcome::StreamedOrClientClosed
                }
                Err(e) if is_clean_disconnect(&e) => {
                    println!("ℹ️ Cliente {} desconectado limpiamente ({})", peer, e.kind());
                    AttemptOutcome::StreamedOrClientClosed
                }
                Err(e) => {
                    eprintln!("⚠️ Conexión de video cerrada ({}): {}", peer, e);
                    AttemptOutcome::StreamedOrClientClosed
                }
            }
        }
        Ok(_) = rx.changed() => {
            println!("🔄 Cambio de monitor solicitado para {}; reiniciando captura", peer);
            AttemptOutcome::MonitorSwitched
        }
    };

    let _ = child.kill().await;
    let _ = child.wait().await;
    outcome
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

    if let Some(dri) = detect_dri_device() {
        println!(
            "🎮 DRI detectado: {} (vendor={:?}, LIBVA_DRIVER_NAME={:?})",
            dri.device_path, dri.vendor, dri.libva_driver
        );
    }

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
            let dri = detect_dri_device();
            let libva_driver = dri.as_ref().and_then(|d| d.libva_driver.as_deref());

            // 1. Intento principal: codificación por hardware (VA-API / DRI detectado)
            let mut hw_args: Vec<String> = Vec::new();
            if session_cfg.experimental_vulkan {
                hw_args.push("--experimental-vulkan".to_string());
            }
            if let Some(ref low_power) = session_cfg.low_power {
                hw_args.push(format!("--low-power={}", low_power));
            }
            if let Some(ref dri_sel) = dri {
                // Pasamos --dri-device explícitamente para renderD128 / iGPU Intel o AMD
                if dri_sel.device_path == "/dev/dri/renderD128"
                    || matches!(dri_sel.vendor.as_deref(), Some("0x8086") | Some("0x1002"))
                {
                    hw_args.push("--dri-device".to_string());
                    hw_args.push(dri_sel.device_path.clone());
                }
            }
            hw_args.extend([
                "-o".to_string(),
                monitor.clone(),
                "--codec".to_string(),
                session_cfg.codec.clone(),
                "--ffmpeg-muxer".to_string(),
                "h264".to_string(),
                "--gop-size".to_string(),
                gop_str.clone(),
                "--max-fps".to_string(),
                fps_str.clone(),
                "-f".to_string(),
                "/dev/stdout".to_string(),
            ]);

            println!(
                "🎬 Lanzando wl-screenrec (HW) en monitor '{}': {:?} (LIBVA_DRIVER_NAME={:?})",
                monitor, hw_args, libva_driver
            );

            let first = run_encoder_process(
                "wl-screenrec",
                &hw_args,
                libva_driver,
                &mut socket,
                &mut rx,
                peer,
            )
            .await;

            let should_fallback = match first {
                AttemptOutcome::EarlyZeroBytes => true,
                AttemptOutcome::SpawnFailed(ref e) => {
                    eprintln!("❌ Error al iniciar wl-screenrec (HW): {}", e);
                    true
                }
                AttemptOutcome::StreamedOrClientClosed | AttemptOutcome::MonitorSwitched => false,
            };

            if should_fallback {
                // 2. Fallback automático a software (wl-screenrec + libx264 sin aceleración VA-API)
                let sw_args = vec![
                    "--no-hw".to_string(),
                    "-o".to_string(),
                    monitor.clone(),
                    "--codec".to_string(),
                    "avc".to_string(),
                    "--ffmpeg-encoder".to_string(),
                    "libx264".to_string(),
                    "--ffmpeg-encoder-options".to_string(),
                    "preset=ultrafast,tune=zerolatency".to_string(),
                    "--encode-pixfmt".to_string(),
                    "yuv420p".to_string(),
                    "--ffmpeg-muxer".to_string(),
                    "h264".to_string(),
                    "--gop-size".to_string(),
                    gop_str.clone(),
                    "--max-fps".to_string(),
                    fps_str.clone(),
                    "-f".to_string(),
                    "/dev/stdout".to_string(),
                ];

                println!(
                    "🛟 Reintentando con fallback software (wl-screenrec libx264) en '{}': {:?}",
                    monitor, sw_args
                );

                let second = run_encoder_process(
                    "wl-screenrec",
                    &sw_args,
                    libva_driver,
                    &mut socket,
                    &mut rx,
                    peer,
                )
                .await;

                let try_wf_recorder = matches!(
                    second,
                    AttemptOutcome::EarlyZeroBytes | AttemptOutcome::SpawnFailed(_)
                );

                if try_wf_recorder {
                    if let Some(wf_bin) = find_in_path("wf-recorder") {
                        let wf_args = vec![
                            "-o".to_string(),
                            monitor.clone(),
                            "-c".to_string(),
                            "libx264".to_string(),
                            "-x".to_string(),
                            "yuv420p".to_string(),
                            "-m".to_string(),
                            "h264".to_string(),
                            "-p".to_string(),
                            "preset=ultrafast".to_string(),
                            "-p".to_string(),
                            "tune=zerolatency".to_string(),
                            "-f".to_string(),
                            "/dev/stdout".to_string(),
                        ];
                        println!(
                            "🛟 Reintentando con fallback wf-recorder en '{}': {:?}",
                            monitor, wf_args
                        );
                        let _ = run_encoder_process(
                            &wf_bin.to_string_lossy(),
                            &wf_args,
                            None,
                            &mut socket,
                            &mut rx,
                            peer,
                        )
                        .await;
                    }
                }
            }

            println!("🔌 Sesión H.264 cerrada con {}", peer);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_intel_render_d128_and_sets_ihd() {
        let base = std::env::temp_dir().join(format!("molex_dri_test_{}", std::process::id()));
        let dri = base.join("dri");
        let drm = base.join("drm");
        std::fs::create_dir_all(drm.join("renderD128/device")).unwrap();
        std::fs::create_dir_all(drm.join("renderD129/device")).unwrap();
        std::fs::create_dir_all(&dri).unwrap();
        std::fs::write(dri.join("renderD128"), b"").unwrap();
        std::fs::write(dri.join("renderD129"), b"").unwrap();
        std::fs::write(drm.join("renderD128/device/vendor"), b"0x8086\n").unwrap();
        std::fs::write(drm.join("renderD129/device/vendor"), b"0x10de\n").unwrap();

        let sel = detect_dri_device_from(&dri, &drm, None).unwrap();
        assert!(sel.device_path.ends_with("renderD128"));
        assert_eq!(sel.vendor.as_deref(), Some("0x8086"));
        assert_eq!(sel.libva_driver.as_deref(), Some("iHD"));

        // Si el usuario ya definió LIBVA_DRIVER_NAME, no se sobreescribe
        let sel_override = detect_dri_device_from(&dri, &drm, Some("i965")).unwrap();
        assert_eq!(sel_override.libva_driver, None);

        let _ = std::fs::remove_dir_all(base);
    }
}
