use std::net::UdpSocket;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;

use crate::models::{DeviceProfile, MolexError, WakeProgress, WakeResult, WakeStage};
use crate::ssh_video::get_rt;
use crate::tunnel::get_tailscale_status_async;

/// Parsea una dirección MAC en formato `XX:XX:XX:XX:XX:XX`, `XX-XX-XX-XX-XX-XX` o `XXXXXXXXXXXX`.
pub fn parse_mac_address(mac: &str) -> Result<[u8; 6], MolexError> {
    let cleaned: String = mac
        .trim()
        .chars()
        .filter(|c| *c != ':' && *c != '-' && *c != '.')
        .collect();

    if cleaned.len() != 12 {
        return Err(MolexError::Generic(format!(
            "Dirección MAC inválida '{}': se esperan 6 octetos hexadecimales (XX:XX:XX:XX:XX:XX)",
            mac
        )));
    }

    let mut bytes = [0u8; 6];
    for i in 0..6 {
        let slice = &cleaned[i * 2..i * 2 + 2];
        bytes[i] = u8::from_str_radix(slice, 16).map_err(|_| {
            MolexError::Generic(format!("Octeto hexadecimal inválido '{}' en MAC '{}'", slice, mac))
        })?;
    }

    Ok(bytes)
}

/// Construye la trama mágica estándar Wake-on-LAN de 102 bytes (`6x 0xFF` + `16x MAC`).
pub fn build_magic_packet(mac: [u8; 6]) -> [u8; 102] {
    let mut packet = [0u8; 102];
    packet[..6].fill(0xFF);
    for i in 0..16 {
        let start = 6 + i * 6;
        packet[start..start + 6].copy_from_slice(&mac);
    }
    packet
}

/// Envía el Magic Packet Wake-on-LAN por UDP al puerto `9` usando broadcast.
pub fn send_magic_packet(mac: &str, broadcast_ip: &str) -> Result<(), MolexError> {
    let mac_bytes = parse_mac_address(mac)?;
    let packet = build_magic_packet(mac_bytes);

    let target_ip = {
        let t = broadcast_ip.trim();
        if t.is_empty() {
            "255.255.255.255"
        } else {
            t
        }
    };
    let target = format!("{}:9", target_ip);

    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| MolexError::Generic(format!("Error creando socket UDP WoL: {}", e)))?;
    socket
        .set_broadcast(true)
        .map_err(|e| MolexError::Generic(format!("Error activando SO_BROADCAST: {}", e)))?;

    socket
        .send_to(&packet, &target)
        .map_err(|e| MolexError::Generic(format!("Error enviando Magic Packet a {}: {}", target, e)))?;

    Ok(())
}

async fn probe_tcp_port(host: &str, port: u16, timeout_ms: u64) -> bool {
    let addr = format!("{}:{}", host, port);
    matches!(
        tokio::time::timeout(Duration::from_millis(timeout_ms), TcpStream::connect(&addr)).await,
        Ok(Ok(_))
    )
}

fn emit_progress(tx: &Option<UnboundedSender<WakeProgress>>, progress: WakeProgress) {
    if let Some(sender) = tx {
        let _ = sender.send(progress);
    }
}

/// Envía el Magic Packet y realiza sondeo cada 2s sobre Tailscale y los puertos `video_port` (9091) / `ssh_port` (22).
pub async fn wake_and_wait_async(
    profile: DeviceProfile,
    progress_tx: Option<UnboundedSender<WakeProgress>>,
) -> Result<WakeResult, MolexError> {
    let clean = profile.sanitized();
    let mac = clean.mac_address.as_deref().ok_or_else(|| {
        MolexError::Generic("El dispositivo no tiene dirección MAC configurada para Wake-on-LAN".into())
    })?;

    let broadcast_ip = clean.effective_broadcast_ip().to_string();
    send_magic_packet(mac, &broadcast_ip)?;

    let started = Instant::now();
    let max_attempts: u32 = 30; // 60 segundos (cada 2s)
    let candidates = clean.candidate_hosts();
    if candidates.is_empty() {
        return Err(MolexError::Generic(
            "El dispositivo no tiene host ni tailscale_ip configurado".into(),
        ));
    }

    emit_progress(
        &progress_tx,
        WakeProgress {
            device_id: clean.id.clone(),
            stage: WakeStage::PacketSent,
            attempt: 0,
            max_attempts,
            tailscale_online: false,
            ssh_ready: false,
            video_ready: false,
            resolved_host: None,
            message: format!("Magic Packet enviado a {} ({})...", mac, broadcast_ip),
        },
    );

    let mut ssh_only_grace_polls: u32 = 0;

    for attempt in 1..=max_attempts {
        // Re-envía una vez en el intento 3 por si se perdió el primer paquete UDP broadcast en Wi-Fi.
        if attempt == 3 {
            let _ = send_magic_packet(mac, &broadcast_ip);
        }

        let mut tailscale_online = false;
        let mut ts_resolved_ip: Option<String> = None;

        if let Ok(ts_status) = get_tailscale_status_async().await {
            if ts_status.is_running {
                for peer in &ts_status.peers {
                    let matches_ip = clean
                        .tailscale_ip
                        .as_deref()
                        .map(|ip| peer.tailscale_ips.iter().any(|p| p == ip))
                        .unwrap_or(false);
                    let matches_host = peer.tailscale_ips.iter().any(|p| p == &clean.host)
                        || (!clean.name.is_empty()
                            && peer.hostname.eq_ignore_ascii_case(&clean.name));

                    if (matches_ip || matches_host) && peer.online {
                        tailscale_online = true;
                        ts_resolved_ip = peer.primary_ip.clone();
                        break;
                    }
                }
            }
        }

        // Lista dinámica de hosts a sondear (priorizando Tailscale si el peer está online)
        let mut probe_hosts = Vec::new();
        if tailscale_online {
            if let Some(ref ts_ip) = ts_resolved_ip {
                probe_hosts.push(ts_ip.clone());
            }
            if let Some(ref cfg_ts) = clean.tailscale_ip {
                if !probe_hosts.iter().any(|h| h == cfg_ts) {
                    probe_hosts.push(cfg_ts.clone());
                }
            }
        }
        for c in &candidates {
            if !probe_hosts.iter().any(|h| h == c) {
                probe_hosts.push(c.clone());
            }
        }

        let mut ready_video_host: Option<String> = None;
        let mut ready_ssh_host: Option<String> = None;

        for host in &probe_hosts {
            let (vid_ok, ssh_ok) = tokio::join!(
                probe_tcp_port(host, clean.video_port, 1200),
                probe_tcp_port(host, clean.ssh_port, 1200)
            );
            if vid_ok && ready_video_host.is_none() {
                ready_video_host = Some(host.clone());
            }
            if ssh_ok && ready_ssh_host.is_none() {
                ready_ssh_host = Some(host.clone());
            }
        }

        if let Some(resolved) = ready_video_host {
            let via_tailscale = resolved.starts_with("100.") || tailscale_online;
            let ssh_ready = ready_ssh_host.is_some();
            emit_progress(
                &progress_tx,
                WakeProgress {
                    device_id: clean.id.clone(),
                    stage: WakeStage::Ready,
                    attempt,
                    max_attempts,
                    tailscale_online,
                    ssh_ready,
                    video_ready: true,
                    resolved_host: Some(resolved.clone()),
                    message: format!("Equipo listo en {} (puerto {})", resolved, clean.video_port),
                },
            );
            return Ok(WakeResult {
                device_id: clean.id,
                resolved_host: resolved,
                via_tailscale,
                ssh_ready,
                video_ready: true,
                elapsed_secs: started.elapsed().as_secs(),
            });
        }

        if let Some(ref ssh_host) = ready_ssh_host {
            ssh_only_grace_polls += 1;
            // SSH ya respondió; damos hasta 4 ciclos (8s) para que la sesión Wayland arranque molex_daemon en 9091.
            if ssh_only_grace_polls >= 4 || attempt == max_attempts {
                let via_tailscale = ssh_host.starts_with("100.") || tailscale_online;
                emit_progress(
                    &progress_tx,
                    WakeProgress {
                        device_id: clean.id.clone(),
                        stage: WakeStage::Ready,
                        attempt,
                        max_attempts,
                        tailscale_online,
                        ssh_ready: true,
                        video_ready: false,
                        resolved_host: Some(ssh_host.clone()),
                        message: format!(
                            "SSH activo en {} (puerto {}), daemon de video aún iniciando",
                            ssh_host, clean.ssh_port
                        ),
                    },
                );
                return Ok(WakeResult {
                    device_id: clean.id,
                    resolved_host: ssh_host.clone(),
                    via_tailscale,
                    ssh_ready: true,
                    video_ready: false,
                    elapsed_secs: started.elapsed().as_secs(),
                });
            }

            emit_progress(
                &progress_tx,
                WakeProgress {
                    device_id: clean.id.clone(),
                    stage: WakeStage::VpnOnlineWaitingDaemon,
                    attempt,
                    max_attempts,
                    tailscale_online,
                    ssh_ready: true,
                    video_ready: false,
                    resolved_host: Some(ssh_host.clone()),
                    message: format!(
                        "SSH activo en {}, esperando daemon de video ({})...",
                        ssh_host, clean.video_port
                    ),
                },
            );
        } else if tailscale_online {
            emit_progress(
                &progress_tx,
                WakeProgress {
                    device_id: clean.id.clone(),
                    stage: WakeStage::VpnOnlineWaitingDaemon,
                    attempt,
                    max_attempts,
                    tailscale_online: true,
                    ssh_ready: false,
                    video_ready: false,
                    resolved_host: ts_resolved_ip.clone(),
                    message: "Nodo activo en Tailscale, esperando servicios SSH/Video...".into(),
                },
            );
        } else {
            emit_progress(
                &progress_tx,
                WakeProgress {
                    device_id: clean.id.clone(),
                    stage: WakeStage::WaitingForNetwork,
                    attempt,
                    max_attempts,
                    tailscale_online: false,
                    ssh_ready: false,
                    video_ready: false,
                    resolved_host: None,
                    message: format!(
                        "Despertando equipo y esperando VPN... ({}/{})",
                        attempt, max_attempts
                    ),
                },
            );
        }

        if attempt < max_attempts {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    emit_progress(
        &progress_tx,
        WakeProgress {
            device_id: clean.id.clone(),
            stage: WakeStage::Timeout,
            attempt: max_attempts,
            max_attempts,
            tailscale_online: false,
            ssh_ready: false,
            video_ready: false,
            resolved_host: None,
            message: "Tiempo de espera agotado tras 60s sin respuesta del equipo".into(),
        },
    );

    Err(MolexError::Generic(format!(
        "El equipo '{}' no respondió tras {} segundos después de enviar Wake-on-LAN",
        clean.name,
        max_attempts * 2
    )))
}

pub fn wake_and_wait(profile: DeviceProfile) -> Result<WakeResult, MolexError> {
    get_rt().block_on(wake_and_wait_async(profile, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_valid_102_byte_magic_packet() {
        let mac = parse_mac_address("aa:bb:cc:dd:ee:ff").unwrap();
        assert_eq!(mac, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);

        let packet = build_magic_packet(mac);
        assert_eq!(packet.len(), 102);
        assert_eq!(&packet[..6], &[0xFF; 6]);
        for i in 0..16 {
            let offset = 6 + i * 6;
            assert_eq!(&packet[offset..offset + 6], &mac);
        }
    }

    #[test]
    fn parses_dash_and_raw_mac_formats() {
        assert_eq!(
            parse_mac_address("00-11-22-33-44-55").unwrap(),
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]
        );
        assert_eq!(
            parse_mac_address("001122334455").unwrap(),
            [0x00, 0x11, 0x22, 0x33, 0x44, 0x55]
        );
        assert!(parse_mac_address("invalid-mac").is_err());
        assert!(parse_mac_address("ZZ:11:22:33:44:55").is_err());
    }
}
