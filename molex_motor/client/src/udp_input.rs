use std::sync::Arc;
use std::net::UdpSocket;
use crate::models::MolexError;

#[derive(uniffi::Object)]
pub struct MolexInputClient {
    host: String,
    socket: UdpSocket,
}

#[uniffi::export]
impl MolexInputClient {
    #[uniffi::constructor]
    pub fn new(host: String) -> Result<Arc<Self>, MolexError> {
        // Abrimos un socket UDP en un puerto efímero del celular
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| MolexError::Generic(e.to_string()))?;
        Ok(Arc::new(Self { host, socket }))
    }

    pub fn send_mouse_move(&self, x_percent: f32, y_percent: f32) {
        // Formato ultra ligero: "MOVE:x:y"
        let msg = format!("MOVE:{}:{}", x_percent, y_percent);
        // Enviaremos esto al puerto 9090 donde vivirá nuestro Daemon en Linux
        let target = format!("{}:9090", self.host); 
        let _ = self.socket.send_to(msg.as_bytes(), &target);
    }

    pub fn send_mouse_click(&self, x_percent: f32, y_percent: f32, is_right_click: bool) {
        let btn = if is_right_click { "RIGHT" } else { "LEFT" };
        let msg = format!("CLICK:{}:{}:{}", x_percent, y_percent, btn);
        let target = format!("{}:9090", self.host);
        let _ = self.socket.send_to(msg.as_bytes(), &target);
    }
}
