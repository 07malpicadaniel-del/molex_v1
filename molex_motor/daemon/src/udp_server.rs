use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{watch, RwLock};

use crate::virtual_device::{self, MolexMouse, MouseButton};

#[derive(Debug, PartialEq)]
pub enum InputCommand {
    Move { x: f32, y: f32 },
    Click { x: Option<f32>, y: Option<f32>, button: MouseButton, state: Option<bool> },
    Wheel { dx: i32, dy: i32 },
    Key { key_code: u16, state: i32 },
    SetMonitor(String),
}

pub fn parse_input_datagram(raw: &str) -> Option<InputCommand> {
    let msg = raw.trim();
    if msg.is_empty() {
        return None;
    }

    if let Some(payload) = msg.strip_prefix("K:") {
        let mut parts = payload.split(':');
        let code = parts.next()?.trim().parse::<u16>().ok()?;
        let state = parts.next()?.trim().parse::<i32>().ok()?;
        return Some(InputCommand::Key {
            key_code: code,
            state,
        });
    }

    if let Some(payload) = msg.strip_prefix("M:").or_else(|| msg.strip_prefix("MOVE:")) {
        let coords: Vec<&str> = payload.split(|c| c == ',' || c == ':').collect();
        if coords.len() >= 2 {
            let x = coords[0].trim().parse::<f32>().ok()?;
            let y = coords[1].trim().parse::<f32>().ok()?;
            return Some(InputCommand::Move { x, y });
        }
        return None;
    }

    if let Some(payload) = msg.strip_prefix("C:").or_else(|| msg.strip_prefix("CLICK:")) {
        let parts: Vec<&str> = payload.split(|c| c == ',' || c == ':').collect();
        if parts.len() >= 3 {
            let x = parts[0].trim().parse::<f32>().ok();
            let y = parts[1].trim().parse::<f32>().ok();
            let button = MouseButton::parse(parts[2]);
            let state = parts.get(3).and_then(|s| match s.trim() {
                "1" | "down" | "true" => Some(true),
                "0" | "up" | "false" => Some(false),
                _ => None,
            });
            return Some(InputCommand::Click {
                x,
                y,
                button,
                state,
            });
        } else if parts.len() == 1 {
            let button = MouseButton::parse(parts[0]);
            return Some(InputCommand::Click {
                x: None,
                y: None,
                button,
                state: None,
            });
        }
        return None;
    }

    if let Some(payload) = msg.strip_prefix("W:").or_else(|| msg.strip_prefix("SCROLL:")) {
        let coords: Vec<&str> = payload.split(|c| c == ',' || c == ':').collect();
        if coords.len() >= 2 {
            let dx = coords[0].trim().parse::<i32>().unwrap_or(0);
            let dy = coords[1].trim().parse::<i32>().unwrap_or(0);
            return Some(InputCommand::Wheel { dx, dy });
        }
        return None;
    }

    if let Some(monitor) = msg.strip_prefix("SET_MONITOR:") {
        let clean = monitor.trim();
        if !clean.is_empty() {
            return Some(InputCommand::SetMonitor(clean.to_string()));
        }
    }

    None
}

fn dispatch_command(
    dev: &mut MolexMouse,
    cmd: InputCommand,
    active_output: &Arc<RwLock<String>>,
    monitor_tx: &watch::Sender<u64>,
) {
    match cmd {
        InputCommand::Move { x, y } => {
            let _ = dev.move_mouse(x, y);
        }
        InputCommand::Click {
            x,
            y,
            button,
            state,
        } => {
            if let (Some(px), Some(py)) = (x, y) {
                let _ = dev.move_mouse(px, py);
            }
            match state {
                Some(pressed) => {
                    let _ = dev.mouse_button(button, pressed);
                }
                None => {
                    let _ = dev.click(button);
                }
            }
        }
        InputCommand::Wheel { dx, dy } => {
            let _ = dev.scroll(dx, dy);
        }
        InputCommand::Key { key_code, state } => {
            let _ = dev.send_key(key_code, state);
        }
        InputCommand::SetMonitor(monitor) => {
            let output_ref = active_output.clone();
            let tx = monitor_tx.clone();
            tokio::spawn(async move {
                let mut guard = output_ref.write().await;
                if *guard != monitor {
                    println!("🖥️  Cambiando monitor activo a: {}", monitor);
                    *guard = monitor;
                    let next = tx.borrow().wrapping_add(1);
                    let _ = tx.send(next);
                }
            });
        }
    }
}

pub async fn start_server(
    addr: &str,
    active_output: Arc<RwLock<String>>,
    monitor_tx: watch::Sender<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind(addr).await?;
    println!("📡 Molex UDP Input Server escuchando en {}", addr);

    let mut device = virtual_device::create_input_device()?;
    let mut buf = [0u8; 1024];

    // Procesamiento FIFO estricto para preservar el orden exacto de eventos de ratón y teclado.
    loop {
        let (len, _peer) = socket.recv_from(&mut buf).await?;
        let msg = String::from_utf8_lossy(&buf[..len]);
        if let Some(cmd) = parse_input_datagram(&msg) {
            dispatch_command(&mut device, cmd, &active_output, &monitor_tx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_input_datagram_formats() {
        assert_eq!(
            parse_input_datagram("M:0.50000,0.25000"),
            Some(InputCommand::Move { x: 0.5, y: 0.25 })
        );
        assert_eq!(
            parse_input_datagram("C:0.4,0.6,RIGHT"),
            Some(InputCommand::Click {
                x: Some(0.4),
                y: Some(0.6),
                button: MouseButton::Right,
                state: None,
            })
        );
        assert_eq!(
            parse_input_datagram("C:0.4,0.6,MIDDLE,1"),
            Some(InputCommand::Click {
                x: Some(0.4),
                y: Some(0.6),
                button: MouseButton::Middle,
                state: Some(true),
            })
        );
        assert_eq!(
            parse_input_datagram("K:125:1"),
            Some(InputCommand::Key {
                key_code: 125,
                state: 1
            })
        );
        assert_eq!(
            parse_input_datagram("W:0,-1"),
            Some(InputCommand::Wheel { dx: 0, dy: -1 })
        );
        assert_eq!(
            parse_input_datagram("SET_MONITOR:HDMI-A-1"),
            Some(InputCommand::SetMonitor("HDMI-A-1".to_string()))
        );
    }
}
