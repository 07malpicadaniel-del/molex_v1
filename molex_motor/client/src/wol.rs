use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;

use crate::models::{DeviceProfile, MolexError, WakeProgress, WakeResult, WakeStage};
use crate::ssh_video::{get_rt, MolexVideoClient};
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

/// Comprueba si una IPv4 pertenece al rango CGNAT de Tailscale (`100.64.0.0/10`).
pub fn is_tailscale_cgnat_ip(ip: Ipv4Addr) -> bool {
    (u32::from(ip) & 0xFFC0_0000) == 0x6440_0000
}

/// Estima si el cliente actual está fuera de la LAN local del equipo destino.
pub fn is_outside_local_lan(profile: &DeviceProfile) -> bool {
    let target = profile.preferred_host();
    if let Ok(IpAddr::V4(dest_ip)) = target.parse::<IpAddr>() {
        if is_tailscale_cgnat_ip(dest_ip) {
            return true;
        }
        if let Ok(sock) = UdpSocket::bind("0.0.0.0:0") {
            if sock.connect((dest_ip, 9)).is_ok() {
                if let Ok(local_addr) = sock.local_addr() {
                    if let IpAddr::V4(src_ip) = local_addr.ip() {
                        if is_tailscale_cgnat_ip(src_ip) {
                            return true;
                        }
                        let same_slash24 = src_ip.octets()[..3] == dest_ip.octets()[..3];
                        return !same_slash24;
                    }
                }
            }
        }
    }

    // Si el host primario es una IP Tailscale o no pudimos verificar la misma subred LAN,
    // cuando hay `wol_relay_host` asumimos ruta externa para garantizar entrega L2.
    target.starts_with("100.")
}

/// Parsea una especificación de relay (`100.x.y.z`, `usuario@100.x.y.z`, `usuario@host:2222`).
pub fn parse_relay_spec(spec: &str, default_user: &str, default_port: u16) -> DeviceProfile {
    let trimmed = spec.trim();
    let (user, host_port) = match trimmed.split_once('@') {
        Some((u, hp)) if !u.trim().is_empty() => (u.trim().to_string(), hp.trim()),
        _ => (default_user.trim().to_string(), trimmed),
    };
    let (host, port) = match host_port.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && p.parse::<u16>().is_ok() => {
            (h.trim().to_string(), p.parse::<u16>().unwrap_or(default_port))
        }
        _ => (host_port.to_string(), default_port),
    };

    DeviceProfile {
        id: format!("relay-{}", host),
        name: format!("WoL Relay ({})", host),
        host,
        ssh_port: if port == 0 { 22 } else { port },
        username: if user.is_empty() {
            "root".to_string()
        } else {
            user
        },
        ..Default::default()
    }
}

/// Construye un comando ligero (wakeonlan / python3 / perl) validando estrictamente MAC y broadcast IP.
pub fn build_relay_wol_command(mac: &str, broadcast_ip: &str) -> Result<String, MolexError> {
    let mac_bytes = parse_mac_address(mac)?;
    let mac_colon = format!(
        "{:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
        mac_bytes[0], mac_bytes[1], mac_bytes[2], mac_bytes[3], mac_bytes[4], mac_bytes[5]
    );
    let mac_hex12 = format!(
        "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        mac_bytes[0], mac_bytes[1], mac_bytes[2], mac_bytes[3], mac_bytes[4], mac_bytes[5]
    );

    let bcast_trimmed = broadcast_ip.trim();
    let bcast_ip: Ipv4Addr = if bcast_trimmed.is_empty() {
        Ipv4Addr::BROADCAST
    } else {
        bcast_trimmed.parse::<Ipv4Addr>().map_err(|_| {
            MolexError::Generic(format!(
                "IP de broadcast inválida '{}' para WoL relay",
                broadcast_ip
            ))
        })?
    };
    let bcast_str = bcast_ip.to_string();

    Ok(format!(
        "if command -v wakeonlan >/dev/null 2>&1; then \
           wakeonlan -i {bcast} {mac_colon}; \
         elif command -v python3 >/dev/null 2>&1; then \
           python3 -c 'import socket; m=bytes.fromhex(\"{hex12}\"); s=socket.socket(socket.AF_INET,socket.SOCK_DGRAM); s.setsockopt(socket.SOL_SOCKET,socket.SO_BROADCAST,1); s.sendto(b\"\\xff\"*6+m*16,(\"{bcast}\",9))'; \
         elif command -v perl >/dev/null 2>&1; then \
           perl -e 'use Socket; socket(S,PF_INET,SOCK_DGRAM,getprotobyname(\"udp\")); setsockopt(S,SOL_SOCKET,SO_BROADCAST,1); send(S,\"\\xff\"x6 . pack(\"H12\",\"{hex12}\")x16,0,pack_sockaddr_in(9,inet_aton(\"{bcast}\")));'; \
         else \
           echo \"NO_WOL_TOOL\" >&2; exit 127; \
         fi",
        bcast = bcast_str,
        mac_colon = mac_colon,
        hex12 = mac_hex12
    ))
}

/// Envía el Magic Packet ejecutando el comando por SSH en un nodo relay dentro de la LAN destino.
pub async fn send_magic_packet_via_relay_async(
    profile: &DeviceProfile,
    relay_spec: &str,
    mac: &str,
    broadcast_ip: &str,
) -> Result<(), MolexError> {
    let relay_cmd = build_relay_wol_command(mac, broadcast_ip)?;
    let mut relay_profile = parse_relay_spec(relay_spec, &profile.username, profile.ssh_port);
    relay_profile.password = profile.password.clone();

    let client = MolexVideoClient::from_device(relay_profile);
    client.connect_async().await.map_err(|e| {
        MolexError::Generic(format!("Error conectando al relay WoL '{}': {}", relay_spec, e))
    })?;

    let exec_res = client.execute_command_async(relay_cmd).await;
    client.disconnect_async().await;

    let out = exec_res?;
    if out.exit_code != Some(0) {
        return Err(MolexError::Generic(format!(
            "El nodo relay '{}' no pudo enviar el Magic Packet: {}",
            relay_spec, out.output
        )));
    }
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
    let relay_host = clean.wol_relay_host.clone();
    let use_relay_immediately = relay_host.is_some() && is_outside_local_lan(&clean);

    let sent_msg = if let Some(ref relay) = relay_host {
        if use_relay_immediately {
            send_magic_packet_via_relay_async(&clean, relay, mac, &broadcast_ip).await?;
            format!("Enviando Magic Packet WoL vía relay {}...", relay)
        } else {
            send_magic_packet(mac, &broadcast_ip)?;
            "Enviando Magic Packet WoL...".to_string()
        }
    } else {
        send_magic_packet(mac, &broadcast_ip)?;
        "Enviando Magic Packet WoL...".to_string()
    };

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
            message: sent_msg,
        },
    );

    let mut ssh_only_grace_polls: u32 = 0;

    for attempt in 1..=max_attempts {
        // En intento 2/3 reintentamos broadcast local o fallback a relay SSH si está configurado.
        if attempt == 2 {
            if let Some(ref relay) = relay_host {
                if !use_relay_immediately {
                    let _ = send_magic_packet_via_relay_async(&clean, relay, mac, &broadcast_ip).await;
                }
            }
        } else if attempt == 3 {
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
                    message: "Host en línea — conectando automáticamente...".to_string(),
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
                        message: "Host en línea — conectando automáticamente...".to_string(),
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
                        "Esperando arranque del sistema y red Tailscale... ({}/{})",
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

    #[test]
    fn parses_relay_spec_and_builds_safe_wol_command() {
        let p1 = parse_relay_spec("pi@100.64.0.99:2222", "daniel", 22);
        assert_eq!(p1.username, "pi");
        assert_eq!(p1.host, "100.64.0.99");
        assert_eq!(p1.ssh_port, 2222);

        let p2 = parse_relay_spec("100.64.0.88", "daniel", 22);
        assert_eq!(p2.username, "daniel");
        assert_eq!(p2.host, "100.64.0.88");
        assert_eq!(p2.ssh_port, 22);

        let cmd = build_relay_wol_command("AA:BB:CC:DD:EE:FF", "192.168.100.255").unwrap();
        assert!(cmd.contains("wakeonlan -i 192.168.100.255 aa:bb:cc:dd:ee:ff"));
        assert!(cmd.contains("aabbccddeeff"));
        assert!(build_relay_wol_command("AA:BB:CC:DD:EE:FF", "192.168.1.1; rm -rf /").is_err());
        assert!(is_tailscale_cgnat_ip("100.77.181.76".parse().unwrap()));
        assert!(!is_tailscale_cgnat_ip("192.168.100.172".parse().unwrap()));
    }
}
