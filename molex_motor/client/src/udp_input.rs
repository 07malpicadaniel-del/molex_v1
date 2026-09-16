use std::sync::Arc;
use std::net::UdpSocket;

#[derive(uniffi::Object)]
pub struct MolexInputClient {
    target_addr: String,
    socket: UdpSocket,
}

#[uniffi::export]
impl MolexInputClient {
    #[uniffi::constructor]
    pub fn new(host: String) -> Arc<Self> {
        // Abrimos un único socket UDP efímero para no asfixiar a Android
        let socket = UdpSocket::bind("0.0.0.0:0").expect("No se pudo asignar puerto UDP local");
        
        // Magia negra: Lo hacemos no bloqueante. 
        // Si el paquete se pierde en el aire, no nos importa, el siguiente llegará en milisegundos.
        socket.set_nonblocking(true).expect("Fallo al hacer el socket no bloqueante");

        Arc::new(Self {
            target_addr: format!("{}:9999", host), // 9999 es el puerto que usará nuestro demonio en Gentoo
            socket,
        })
    }

    pub fn send_mouse_move(&self, x_percent: f32, y_percent: f32) {
        // Formateamos a 4 decimales para que el paquete de red sea súper ligero (ej. "M:0.5123:0.1234")
        let msg = format!("M:{:.4}:{:.4}", x_percent, y_percent);
        
        // Fire-and-forget nativo
        let _ = self.socket.send_to(msg.as_bytes(), &self.target_addr);
    }

    pub fn send_mouse_click(&self, _x_percent: f32, _y_percent: f32, is_right_click: bool) {
        let btn = if is_right_click { "R" } else { "L" };
        let msg = format!("C:{}", btn);
        let _ = self.socket.send_to(msg.as_bytes(), &self.target_addr);
    }
}
