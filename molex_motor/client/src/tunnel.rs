use std::collections::HashMap;
use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;

use crate::models::{DeviceProfile, MolexError, TailscalePeer, TailscaleStatus};
use crate::ssh_video::get_rt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionCandidate {
    pub host: String,
    pub via_tailscale: bool,
}

/// Ordena los hosts candidatos priorizando `tailscale_ip` cuando Tailscale local
/// está `Running` y el peer remoto aparece `Online`; en caso contrario usa `host` (LAN) primero
/// con fallback automático al secundario.
pub fn resolve_connection_candidates(
    profile: &DeviceProfile,
    ts_status: Option<&TailscaleStatus>,
) -> Vec<ConnectionCandidate> {
    let lan_host = profile.host.trim();
    let ts_ip = profile
        .tailscale_ip
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let peer_online_on_tailscale = match (ts_ip, ts_status) {
        (Some(ip), Some(status)) if status.is_running => status
            .peers
            .iter()
            .any(|p| p.online && p.tailscale_ips.iter().any(|peer_ip| peer_ip == ip)),
        _ => false,
    };

    let mut candidates = Vec::new();
    let mut push_unique = |host: &str, via_tailscale: bool| {
        if !host.is_empty() && !candidates.iter().any(|c: &ConnectionCandidate| c.host == host) {
            candidates.push(ConnectionCandidate {
                host: host.to_string(),
                via_tailscale,
            });
        }
    };

    if peer_online_on_tailscale {
        if let Some(ts) = ts_ip {
            push_unique(ts, true);
        }
        push_unique(lan_host, lan_host.starts_with("100."));
    } else {
        push_unique(lan_host, lan_host.starts_with("100."));
        if let Some(ts) = ts_ip {
            push_unique(ts, true);
        }
    }

    candidates
}

#[derive(Debug, Deserialize)]
struct RawTailscaleStatus {
    #[serde(rename = "BackendState", default)]
    backend_state: String,
    #[serde(rename = "AuthURL", default)]
    auth_url: Option<String>,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Option<Vec<String>>,
    #[serde(rename = "Self", default)]
    self_node: Option<RawTailscaleNode>,
    #[serde(rename = "Peer", default)]
    peer: Option<HashMap<String, RawTailscaleNode>>,
}

#[derive(Debug, Deserialize, Default)]
struct RawTailscaleNode {
    #[serde(rename = "HostName", default)]
    hostname: String,
    #[serde(rename = "DNSName", default)]
    dns_name: String,
    #[serde(rename = "OS", default)]
    os: String,
    #[serde(rename = "TailscaleIPs", default)]
    tailscale_ips: Option<Vec<String>>,
    #[serde(rename = "Online", default)]
    online: bool,
    #[serde(rename = "Active", default)]
    active: bool,
    #[serde(rename = "CurAddr", default)]
    cur_addr: String,
    #[serde(rename = "Relay", default)]
    relay: String,
}

/// Parsea la salida de `tailscale status --json` hacia `TailscaleStatus`.
pub fn parse_tailscale_status_json(raw_json: &str) -> Result<TailscaleStatus, MolexError> {
    let raw: RawTailscaleStatus = serde_json::from_str(raw_json)
        .map_err(|e| MolexError::Generic(format!("Error parseando JSON de Tailscale: {}", e)))?;

    let is_running = raw.backend_state == "Running";

    let self_ips = raw
        .self_node
        .as_ref()
        .and_then(|s| s.tailscale_ips.clone())
        .or(raw.tailscale_ips)
        .unwrap_or_default();

    let self_hostname = raw
        .self_node
        .as_ref()
        .map(|s| s.hostname.clone())
        .unwrap_or_default();

    let mut peers: Vec<TailscalePeer> = raw
        .peer
        .unwrap_or_default()
        .into_iter()
        .map(|(id, node)| {
            let ips = node.tailscale_ips.unwrap_or_default();
            let primary_ip = ips
                .iter()
                .find(|ip| ip.contains('.'))
                .or_else(|| ips.first())
                .cloned();
            TailscalePeer {
                id,
                hostname: node.hostname,
                dns_name: node.dns_name.trim_end_matches('.').to_string(),
                os: node.os,
                tailscale_ips: ips,
                primary_ip,
                online: node.online,
                active: node.active,
                cur_addr: node.cur_addr,
                relay: node.relay,
            }
        })
        .collect();

    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
    });

    Ok(TailscaleStatus {
        backend_state: raw.backend_state,
        is_running,
        auth_url: raw.auth_url.filter(|s| !s.trim().is_empty()),
        self_hostname,
        self_ips,
        peers,
    })
}

pub async fn get_tailscale_status_async() -> Result<TailscaleStatus, MolexError> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        Command::new("tailscale").args(["status", "--json"]).output(),
    )
    .await
    .map_err(|_| MolexError::Generic("Timeout consultando estado de Tailscale".into()))?
    .map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            MolexError::Generic("El binario 'tailscale' no está instalado en el sistema".into())
        } else {
            MolexError::Generic(format!("No se pudo ejecutar 'tailscale': {}", e))
        }
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if stdout.starts_with('{') {
        return parse_tailscale_status_json(&stdout);
    }

    if !output.status.success() {
        let msg = if stderr.is_empty() {
            format!("tailscale exited with {}", output.status)
        } else {
            stderr
        };
        return Err(MolexError::Generic(format!("Error Tailscale: {}", msg)));
    }

    Err(MolexError::Generic(
        "Respuesta inesperada de 'tailscale status --json'".into(),
    ))
}

pub async fn toggle_tailscale_async(enable: bool) -> Result<TailscaleStatus, MolexError> {
    let args: Vec<&str> = if enable {
        vec!["up", "--timeout=6s"]
    } else {
        vec!["down"]
    };

    let output = tokio::time::timeout(
        Duration::from_secs(10),
        Command::new("tailscale").args(&args).output(),
    )
    .await
    .map_err(|_| MolexError::Generic("Timeout cambiando estado de Tailscale".into()))?
    .map_err(|e| MolexError::Generic(format!("Error ejecutando tailscale {:?}: {}", args, e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };

        if detail.to_lowercase().contains("access denied")
            || detail.to_lowercase().contains("permission")
        {
            return Err(MolexError::Generic(format!(
                "Permisos insuficientes para controlar Tailscale sin root. Ejecuta una vez: `sudo tailscale set --operator=$USER` ({})",
                detail
            )));
        }

        // Si `tailscale up` devuelve URL de login, seguimos consultando status --json para mostrar el AuthURL en UI.
        if !(enable && detail.contains("https://login.tailscale.com")) {
            return Err(MolexError::Generic(format!("Error Tailscale: {}", detail)));
        }
    }

    get_tailscale_status_async().await
}

pub fn get_tailscale_status() -> Result<TailscaleStatus, MolexError> {
    get_rt().block_on(get_tailscale_status_async())
}

pub fn toggle_tailscale(enable: bool) -> Result<TailscaleStatus, MolexError> {
    get_rt().block_on(toggle_tailscale_async(enable))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_needs_login_status() {
        let sample = r#"{
            "BackendState": "NeedsLogin",
            "AuthURL": "https://login.tailscale.com/a/abc123",
            "TailscaleIPs": null,
            "Self": {
                "HostName": "archleika",
                "TailscaleIPs": null,
                "Online": false
            },
            "Peer": null
        }"#;

        let status = parse_tailscale_status_json(sample).unwrap();
        assert_eq!(status.backend_state, "NeedsLogin");
        assert!(!status.is_running);
        assert_eq!(
            status.auth_url.as_deref(),
            Some("https://login.tailscale.com/a/abc123")
        );
        assert_eq!(status.self_hostname, "archleika");
        assert!(status.peers.is_empty());
    }

    #[test]
    fn parses_running_status_with_peers_sorted_online_first() {
        let sample = r#"{
            "BackendState": "Running",
            "Self": {
                "HostName": "gentoo-laptop",
                "TailscaleIPs": ["100.64.0.10", "fd7a::10"],
                "Online": true
            },
            "Peer": {
                "nodekey:1": {
                    "HostName": "offline-nas",
                    "DNSName": "offline-nas.tail123.ts.net.",
                    "OS": "linux",
                    "TailscaleIPs": ["100.64.0.20"],
                    "Online": false,
                    "Active": false,
                    "CurAddr": "",
                    "Relay": "dfw"
                },
                "nodekey:2": {
                    "HostName": "arch-workstation",
                    "DNSName": "arch-workstation.tail123.ts.net.",
                    "OS": "linux",
                    "TailscaleIPs": ["100.64.0.5", "fd7a::5"],
                    "Online": true,
                    "Active": true,
                    "CurAddr": "192.168.1.50:41641",
                    "Relay": ""
                }
            }
        }"#;

        let status = parse_tailscale_status_json(sample).unwrap();
        assert!(status.is_running);
        assert_eq!(status.self_hostname, "gentoo-laptop");
        assert_eq!(status.self_ips, vec!["100.64.0.10", "fd7a::10"]);
        assert_eq!(status.peers.len(), 2);
        assert_eq!(status.peers[0].hostname, "arch-workstation");
        assert!(status.peers[0].online);
        assert_eq!(status.peers[0].primary_ip.as_deref(), Some("100.64.0.5"));
        assert_eq!(status.peers[1].hostname, "offline-nas");
        assert!(!status.peers[1].online);
    }

    #[test]
    fn prioritizes_tailscale_ip_when_running_and_peer_online() {
        let profile = DeviceProfile {
            host: "192.168.100.172".into(),
            tailscale_ip: Some("100.64.0.5".into()),
            ..Default::default()
        };

        let online_status = TailscaleStatus {
            backend_state: "Running".into(),
            is_running: true,
            auth_url: None,
            self_hostname: "archleika".into(),
            self_ips: vec!["100.64.0.1".into()],
            peers: vec![TailscalePeer {
                id: "1".into(),
                hostname: "leika-laptop".into(),
                dns_name: "leika-laptop.ts.net".into(),
                os: "linux".into(),
                tailscale_ips: vec!["100.64.0.5".into()],
                primary_ip: Some("100.64.0.5".into()),
                online: true,
                active: true,
                cur_addr: String::new(),
                relay: String::new(),
            }],
        };

        let cands = resolve_connection_candidates(&profile, Some(&online_status));
        assert_eq!(
            cands,
            vec![
                ConnectionCandidate {
                    host: "100.64.0.5".into(),
                    via_tailscale: true
                },
                ConnectionCandidate {
                    host: "192.168.100.172".into(),
                    via_tailscale: false
                }
            ]
        );

        let mut offline_status = online_status.clone();
        offline_status.peers[0].online = false;
        let fallback_cands = resolve_connection_candidates(&profile, Some(&offline_status));
        assert_eq!(fallback_cands[0].host, "192.168.100.172");
        assert!(!fallback_cands[0].via_tailscale);
        assert_eq!(fallback_cands[1].host, "100.64.0.5");

        let down_cands = resolve_connection_candidates(&profile, None);
        assert_eq!(down_cands[0].host, "192.168.100.172");
    }
}
