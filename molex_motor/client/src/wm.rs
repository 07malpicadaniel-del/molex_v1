use crate::ssh_video::MolexVideoClient;
use crate::models::MolexError;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

impl MolexVideoClient {
    pub(crate) async fn fetch_monitors_internal(&self) -> Result<Vec<String>, MolexError> {
        let cmd = "export XDG_RUNTIME_DIR=/run/user/1000; export WAYLAND_DISPLAY=wayland-1; export HYPRLAND_INSTANCE_SIGNATURE=$(ls -t /run/user/1000/hypr | head -n 1); hyprctl monitors | awk '/Monitor/ {print $2}'";
        
        let output = self.run_command(cmd).await?;
        
        if output.starts_with("[No output") {
            return Ok(vec![]);
        }

        let monitors: Vec<String> = output
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !s.contains("ls:") && !s.contains("No such file"))
            .collect();
            
        Ok(monitors)
    }

    pub(crate) async fn capture_frame_internal(&self, monitor_name: Option<String>) -> Result<String, MolexError> {
        // SOLUCIÓN 1: Limpieza agresiva del nombre para borrar retornos de carro (\r) invisibles
        let clean_name = monitor_name.unwrap_or_default().trim().replace(['\r', '\n'], "");
        
        let output_flag = if !clean_name.is_empty() {
            format!("-o '{}' ", clean_name)
        } else {
            "".to_string()
        };

        let cmd = format!(
            "WAYLAND_DISPLAY=wayland-1 XDG_RUNTIME_DIR=/run/user/1000 grim {} -t jpeg -q 30 - | base64 -w 0",
            output_flag
        );
        
        let base64_image = self.run_command(&cmd).await?;

        // SOLUCIÓN 2: Si el string devuelto tiene espacios, NO es Base64. Es un error de la terminal de Linux.
        if base64_image.is_empty() || base64_image.contains(' ') || base64_image.starts_with("[No output") {
            return Err(MolexError::Generic(format!("Fallo en grim: {}", base64_image)));
        }

        let mut hasher = DefaultHasher::new();
        base64_image.hash(&mut hasher);
        let current_hash = hasher.finish();

        let mut last_hash = self.last_frame_hash.lock().await;
        if *last_hash == current_hash {
            return Ok("SAME_FRAME".to_string());
        }
        
        *last_hash = current_hash;
        Ok(base64_image)
    }
}
