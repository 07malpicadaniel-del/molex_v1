use tokio::net::UdpSocket;
use std::error::Error;
use evdev::Key;
use crate::virtual_device::MolexVirtualDevice;

pub async fn run_server() -> Result<(), Box<dyn Error>> {
    // 1. Inicializamos nuestra arma secreta (El ratón a nivel de Kernel)
    let mut device = MolexVirtualDevice::new()?;
    println!("✅ Hardware virtual inyectado con éxito.");
    
    // 2. Abrimos el puerto UDP para escuchar al celular
    let socket = UdpSocket::bind("0.0.0.0:9999").await?;
    println!("📡 Servidor UDP activo. Escuchando coordenadas en el puerto 9999...");

    // Buffer ultra pequeño (64 bytes) porque nuestros paquetes son diminutos
    let mut buf = [0; 64]; 

    // 3. Bucle infinito de alta velocidad
    loop {
        let (len, _addr) = socket.recv_from(&mut buf).await?;
        
        // Convertimos los bytes a texto ignorando errores para evitar que el demonio crashee
        if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
            let parts: Vec<&str> = msg.trim().split(':').collect();
            if parts.is_empty() { continue; }

            match parts[0] {
                // Comando de Movimiento (M : X_Porcentaje : Y_Porcentaje)
                "M" if parts.len() == 3 => {
                    if let (Ok(x_pct), Ok(y_pct)) = (parts[1].parse::<f32>(), parts[2].parse::<f32>()) {
                        // Convertimos el porcentaje que manda Android a los píxeles reales de tu monitor
                        let x = (x_pct * 1920.0) as i32;
                        let y = (y_pct * 1080.0) as i32;
                        let _ = device.move_mouse(x, y);
                    }
                },
                // Comando de Clic (C : L o R)
                "C" if parts.len() == 2 => {
                    let btn = if parts[1] == "R" { Key::BTN_RIGHT } else { Key::BTN_LEFT };
                    // Simulamos un toque rápido humano (Presionar y Soltar en milisegundos)
                    let _ = device.click_mouse(btn, true);  // Down
                    let _ = device.click_mouse(btn, false); // Up
                },
                _ => {} // Si llega basura por la red, la ignoramos silenciosamente
            }
        }
    }
}
