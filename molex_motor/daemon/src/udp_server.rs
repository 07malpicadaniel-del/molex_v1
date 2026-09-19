use tokio::net::UdpSocket;
use crate::virtual_device::MolexMouse;

pub async fn start_server(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind(addr).await?;
    println!("📡 Molex UDP Server escuchando en {}", addr);

    // Creamos el canvas del mouse con 10,000 puntos de precisión para evitar saltos
    let mut mouse = MolexMouse::new(10000, 10000)?; 

    let mut buf = [0; 1024];

    loop {
        let (len, _) = socket.recv_from(&mut buf).await?;
        let msg = String::from_utf8_lossy(&buf[..len]);
        
        let parts: Vec<&str> = msg.split(':').collect();
        if parts.is_empty() { continue; }

        match parts[0] {
            "MOVE" if parts.len() == 3 => {
                if let (Ok(x), Ok(y)) = (parts[1].parse::<f32>(), parts[2].parse::<f32>()) {
                    let _ = mouse.move_mouse(x, y);
                }
            }
            "CLICK" if parts.len() == 4 => {
                if let (Ok(x), Ok(y)) = (parts[1].parse::<f32>(), parts[2].parse::<f32>()) {
                    let _ = mouse.move_mouse(x, y); // Mover el mouse a la posición antes del click
                    let is_right = parts[3] == "RIGHT";
                    let _ = mouse.click(is_right);
                }
            }
            _ => {}
        }
    }
}
