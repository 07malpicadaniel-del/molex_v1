use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::Arc;

use crate::models::MolexError;

#[cfg_attr(feature = "uniffi-bindings", derive(uniffi::Object))]
pub struct MolexInputClient {
    host: String,
    port: u16,
    target: String,
    resolved_addr: Option<SocketAddr>,
    socket: UdpSocket,
}

impl MolexInputClient {
    pub fn new_with_port(host: String, port: u16) -> Result<Arc<Self>, MolexError> {
        let clean_host = host.trim().to_string();
        let effective_port = if port == 0 { 9090 } else { port };
        let target = format!("{}:{}", clean_host, effective_port);
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| MolexError::Generic(e.to_string()))?;
        let _ = socket.set_nonblocking(true);

        let resolved_addr = target
            .to_socket_addrs()
            .ok()
            .and_then(|mut iter| iter.next());

        Ok(Arc::new(Self {
            host: clean_host,
            port: effective_port,
            target,
            resolved_addr,
            socket,
        }))
    }

    #[inline]
    fn send_raw(&self, payload: &[u8]) {
        if let Some(addr) = self.resolved_addr {
            let _ = self.socket.send_to(payload, addr);
        } else {
            let _ = self.socket.send_to(payload, &self.target);
        }
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn send_move(&self, x_percent: f32, y_percent: f32) {
        let x = x_percent.clamp(0.0, 1.0);
        let y = y_percent.clamp(0.0, 1.0);
        let msg = format!("M:{:.5},{:.5}", x, y);
        self.send_raw(msg.as_bytes());
    }

    pub fn send_click(&self, x_percent: f32, y_percent: f32, button: &str) {
        let x = x_percent.clamp(0.0, 1.0);
        let y = y_percent.clamp(0.0, 1.0);
        let msg = format!("C:{:.5},{:.5},{}", x, y, button);
        self.send_raw(msg.as_bytes());
    }

    pub fn send_mouse_button(&self, x_percent: f32, y_percent: f32, button: &str, pressed: bool) {
        let x = x_percent.clamp(0.0, 1.0);
        let y = y_percent.clamp(0.0, 1.0);
        let state = if pressed { 1 } else { 0 };
        let msg = format!("C:{:.5},{:.5},{},{}", x, y, button, state);
        self.send_raw(msg.as_bytes());
    }

    pub fn send_wheel(&self, dx: i32, dy: i32) {
        let msg = format!("W:{},{}", dx, dy);
        self.send_raw(msg.as_bytes());
    }

    pub fn send_key(&self, linux_keycode: u16, state: i32) {
        let msg = format!("K:{}:{}", linux_keycode, state);
        self.send_raw(msg.as_bytes());
    }

    pub fn send_key_combo(&self, keycodes: &[u16]) {
        for &code in keycodes {
            self.send_key(code, 1);
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        for &code in keycodes.iter().rev() {
            self.send_key(code, 0);
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
    }

    pub fn send_set_monitor(&self, monitor: &str) {
        let clean = monitor.trim();
        if !clean.is_empty() {
            let msg = format!("SET_MONITOR:{}", clean);
            self.send_raw(msg.as_bytes());
        }
    }
}

#[cfg_attr(feature = "uniffi-bindings", uniffi::export)]
impl MolexInputClient {
    #[cfg_attr(feature = "uniffi-bindings", uniffi::constructor)]
    pub fn new(host: String) -> Result<Arc<Self>, MolexError> {
        Self::new_with_port(host, 9090)
    }

    pub fn send_mouse_move(&self, x_percent: f32, y_percent: f32) {
        self.send_move(x_percent, y_percent);
    }

    pub fn send_mouse_click(&self, x_percent: f32, y_percent: f32, is_right_click: bool) {
        let btn = if is_right_click { "RIGHT" } else { "LEFT" };
        self.send_click(x_percent, y_percent, btn);
    }
}
