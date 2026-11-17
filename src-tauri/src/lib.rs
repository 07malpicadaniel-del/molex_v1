mod storage;
mod video_decoder;

use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use molex_core::{
    get_tailscale_status_async, resolve_connection_candidates, send_magic_packet,
    toggle_tailscale_async, wake_and_wait_async, CommandResult, ConnectionCandidate,
    DesktopMetrics, DeviceProfile, MolexInputClient, MolexVideoClient, MonitorInfo,
    TailscaleStatus, WakeProgress, WakeResult,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;

use crate::storage::{DesktopSettings, PersistedStore};
use crate::video_decoder::{StreamStats, VideoEndpointInfo, VideoPipeline};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSummary {
    pub device: DeviceProfile,
    pub host_used: String,
    pub ssh_connected: bool,
    pub ssh_error: Option<String>,
    pub monitors: Vec<MonitorInfo>,
    pub cwd: String,
    pub video_endpoint: VideoEndpointInfo,
}

pub struct AppState {
    pub store: StdMutex<PersistedStore>,
    pub active_device: StdMutex<Option<DeviceProfile>>,
    pub active_host: StdMutex<Option<String>>,
    pub input_client: StdMutex<Option<Arc<MolexInputClient>>>,
    pub ssh_client: AsyncMutex<Option<Arc<MolexVideoClient>>>,
    pub video_pipeline: Arc<VideoPipeline>,
}

async fn probe_host_reachable(host: &str, ssh_port: u16, video_port: u16) -> bool {
    let ssh_addr = format!("{}:{}", host, ssh_port);
    let vid_addr = format!("{}:{}", host, video_port);
    let (ssh_res, vid_res) = tokio::join!(
        tokio::time::timeout(Duration::from_millis(1400), TcpStream::connect(&ssh_addr)),
        tokio::time::timeout(Duration::from_millis(1400), TcpStream::connect(&vid_addr))
    );
    matches!(ssh_res, Ok(Ok(_))) || matches!(vid_res, Ok(Ok(_)))
}

async fn pick_reachable_candidate(
    candidates: &[ConnectionCandidate],
    ssh_port: u16,
    video_port: u16,
) -> ConnectionCandidate {
    if candidates.len() <= 1 {
        return candidates[0].clone();
    }
    for cand in candidates {
        if probe_host_reachable(&cand.host, ssh_port, video_port).await {
            return cand.clone();
        }
    }
    candidates[0].clone()
}

#[tauri::command]
fn list_devices(state: State<'_, AppState>) -> Result<Vec<DeviceProfile>, String> {
    let guard = state.store.lock().map_err(|e| e.to_string())?;
    Ok(guard.list_devices())
}

#[tauri::command]
fn save_device(
    state: State<'_, AppState>,
    device: DeviceProfile,
) -> Result<Vec<DeviceProfile>, String> {
    let mut guard = state.store.lock().map_err(|e| e.to_string())?;
    guard.upsert_device(device)
}

#[tauri::command]
fn delete_device(
    state: State<'_, AppState>,
    device_id: String,
) -> Result<Vec<DeviceProfile>, String> {
    let mut guard = state.store.lock().map_err(|e| e.to_string())?;
    guard.delete_device(&device_id)
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Result<DesktopSettings, String> {
    let guard = state.store.lock().map_err(|e| e.to_string())?;
    Ok(guard.get_settings())
}

#[tauri::command]
fn save_settings(
    state: State<'_, AppState>,
    settings: DesktopSettings,
) -> Result<DesktopSettings, String> {
    let mut guard = state.store.lock().map_err(|e| e.to_string())?;
    guard.save_settings(settings)
}

#[tauri::command]
async fn get_tailscale_status() -> Result<TailscaleStatus, String> {
    get_tailscale_status_async().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn toggle_tailscale(enable: bool) -> Result<TailscaleStatus, String> {
    toggle_tailscale_async(enable)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn send_wol(mac: String, broadcast_ip: Option<String>) -> Result<(), String> {
    let bcast = broadcast_ip.unwrap_or_else(|| "255.255.255.255".to_string());
    tokio::task::spawn_blocking(move || send_magic_packet(&mac, &bcast))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn wake_and_wait_device(
    app: AppHandle,
    profile: DeviceProfile,
) -> Result<WakeResult, String> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<WakeProgress>();
    let app_clone = app.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app_clone.emit("wol-progress", &progress);
        }
    });

    let res = wake_and_wait_async(profile, Some(tx))
        .await
        .map_err(|e| e.to_string());
    let _ = forwarder.await;
    res
}

#[tauri::command]
async fn connect_device(
    state: State<'_, AppState>,
    profile: DeviceProfile,
) -> Result<ConnectionSummary, String> {
    let clean = profile.sanitized();
    let ts_status = get_tailscale_status_async().await.ok();
    let candidates = resolve_connection_candidates(&clean, ts_status.as_ref());
    if candidates.is_empty() {
        return Err("Debes proporcionar una IP LAN o IP Tailscale".to_string());
    }

    let selected = pick_reachable_candidate(&candidates, clean.ssh_port, clean.video_port).await;
    let mut host = selected.host.clone();

    let decoder_threads = state
        .store
        .lock()
        .map(|s| s.get_settings().decoder_threads)
        .unwrap_or(4);

    // Cerrar sesión previa
    if let Some(prev_ssh) = state.ssh_client.lock().await.take() {
        prev_ssh.disconnect_async().await;
    }
    state.video_pipeline.stop_stream();

    // Conectar SSH (priorizando el host elegido y haciendo fallback automático al secundario si falla)
    let mut connect_profile = clean.clone();
    connect_profile.host = host.clone();
    let mut ssh = MolexVideoClient::from_device(connect_profile);
    let mut ssh_connected = false;
    let mut ssh_error = None;
    let mut monitors = Vec::new();
    let mut cwd = String::new();

    match ssh.connect_async().await {
        Ok(()) => {
            ssh_connected = true;
            cwd = ssh.current_dir_async().await;
            if let Ok(mons) = ssh.get_monitors_detailed_async().await {
                monitors = mons;
            }
        }
        Err(first_err) => {
            if let Some(fallback) = candidates.iter().find(|c| c.host != host) {
                let mut fb_profile = clean.clone();
                fb_profile.host = fallback.host.clone();
                let fb_ssh = MolexVideoClient::from_device(fb_profile);
                if fb_ssh.connect_async().await.is_ok() {
                    host = fallback.host.clone();
                    ssh = fb_ssh;
                    ssh_connected = true;
                    cwd = ssh.current_dir_async().await;
                    if let Ok(mons) = ssh.get_monitors_detailed_async().await {
                        monitors = mons;
                    }
                } else {
                    ssh_error = Some(first_err.to_string());
                }
            } else {
                ssh_error = Some(first_err.to_string());
            }
        }
    }

    // Cliente UDP de input (puerto 9090 o configurado) sobre el host resuelto
    let input = MolexInputClient::new_with_port(host.clone(), clean.input_port)
        .map_err(|e| e.to_string())?;
    if let Ok(mut guard) = state.input_client.lock() {
        *guard = Some(input);
    }
    if let Ok(mut guard) = state.active_device.lock() {
        *guard = Some(clean.clone());
    }
    if let Ok(mut guard) = state.active_host.lock() {
        *guard = Some(host.clone());
    }
    *state.ssh_client.lock().await = Some(ssh);

    // Iniciar pipeline de decodificación H.264 sobre TCP (puerto 9091 o configurado)
    state
        .video_pipeline
        .start_stream(host.clone(), clean.video_port, decoder_threads);

    Ok(ConnectionSummary {
        device: clean,
        host_used: host,
        ssh_connected,
        ssh_error,
        monitors,
        cwd,
        video_endpoint: state.video_pipeline.endpoint_info(),
    })
}

#[tauri::command]
async fn disconnect_device(state: State<'_, AppState>) -> Result<(), String> {
    state.video_pipeline.stop_stream();
    if let Ok(mut guard) = state.input_client.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = state.active_device.lock() {
        *guard = None;
    }
    if let Ok(mut guard) = state.active_host.lock() {
        *guard = None;
    }
    if let Some(ssh) = state.ssh_client.lock().await.take() {
        ssh.disconnect_async().await;
    }
    Ok(())
}

#[tauri::command]
async fn get_monitors(state: State<'_, AppState>) -> Result<Vec<MonitorInfo>, String> {
    let ssh_opt = state.ssh_client.lock().await.clone();
    let Some(ssh) = ssh_opt else {
        return Err("No hay sesión activa".into());
    };
    ssh.get_monitors_detailed_async()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn switch_monitor(state: State<'_, AppState>, monitor_name: String) -> Result<(), String> {
    let input_opt = state.input_client.lock().ok().and_then(|g| g.clone());
    if let Some(input) = input_opt {
        input.send_set_monitor(&monitor_name);
    }

    let device_opt = state.active_device.lock().ok().and_then(|g| g.clone());
    let host_opt = state.active_host.lock().ok().and_then(|g| g.clone());
    let threads = state
        .store
        .lock()
        .map(|s| s.get_settings().decoder_threads)
        .unwrap_or(4);

    if let Some(dev) = device_opt {
        let host = host_opt.unwrap_or_else(|| dev.preferred_host());
        tokio::time::sleep(Duration::from_millis(120)).await;
        state
            .video_pipeline
            .start_stream(host, dev.video_port, threads);
    }
    Ok(())
}

#[tauri::command]
async fn restart_video_stream(state: State<'_, AppState>) -> Result<(), String> {
    let device_opt = state.active_device.lock().ok().and_then(|g| g.clone());
    let host_opt = state.active_host.lock().ok().and_then(|g| g.clone());
    let Some(dev) = device_opt else {
        return Err("No hay dispositivo activo".into());
    };
    let threads = state
        .store
        .lock()
        .map(|s| s.get_settings().decoder_threads)
        .unwrap_or(4);
    let host = host_opt.unwrap_or_else(|| dev.preferred_host());
    state
        .video_pipeline
        .start_stream(host, dev.video_port, threads);
    Ok(())
}

#[tauri::command]
fn get_video_endpoint(state: State<'_, AppState>) -> VideoEndpointInfo {
    state.video_pipeline.endpoint_info()
}

#[tauri::command]
fn get_stream_stats(state: State<'_, AppState>) -> StreamStats {
    state.video_pipeline.get_stats()
}

#[tauri::command]
fn get_latest_video_frame(state: State<'_, AppState>) -> tauri::ipc::Response {
    match state.video_pipeline.get_latest_frame_bytes() {
        Some(bytes) => tauri::ipc::Response::new(bytes),
        None => tauri::ipc::Response::new(Vec::new()),
    }
}

#[tauri::command]
async fn measure_latency(host: String, port: u16) -> Result<u32, String> {
    let target = format!("{}:{}", host.trim(), if port == 0 { 9091 } else { port });
    let start = Instant::now();
    tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(&target))
        .await
        .map_err(|_| "Timeout midiendo latencia".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(start.elapsed().as_millis().max(1) as u32)
}

#[tauri::command]
async fn execute_ssh_command(
    state: State<'_, AppState>,
    command: String,
) -> Result<CommandResult, String> {
    let ssh_opt = state.ssh_client.lock().await.clone();
    let Some(ssh) = ssh_opt else {
        return Err("No hay conexión SSH activa. Conéctate primero desde Dispositivos.".into());
    };
    ssh.execute_command_async(command)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_ssh_cwd(state: State<'_, AppState>) -> Result<String, String> {
    let ssh_opt = state.ssh_client.lock().await.clone();
    let Some(ssh) = ssh_opt else {
        return Ok(String::new());
    };
    Ok(ssh.current_dir_async().await)
}

#[tauri::command]
async fn get_desktop_metrics(state: State<'_, AppState>) -> Result<DesktopMetrics, String> {
    let ssh_opt = state.ssh_client.lock().await.clone();
    let Some(ssh) = ssh_opt else {
        return Err("No hay conexión SSH activa".into());
    };
    ssh.get_desktop_metrics_async()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn input_mouse_move(state: State<'_, AppState>, x_percent: f32, y_percent: f32) {
    if let Ok(guard) = state.input_client.lock() {
        if let Some(ref client) = *guard {
            client.send_move(x_percent, y_percent);
        }
    }
}

#[tauri::command]
fn input_mouse_click(
    state: State<'_, AppState>,
    x_percent: f32,
    y_percent: f32,
    button: String,
) {
    if let Ok(guard) = state.input_client.lock() {
        if let Some(ref client) = *guard {
            client.send_click(x_percent, y_percent, &button);
        }
    }
}

#[tauri::command]
fn input_mouse_button(
    state: State<'_, AppState>,
    x_percent: f32,
    y_percent: f32,
    button: String,
    pressed: bool,
) {
    if let Ok(guard) = state.input_client.lock() {
        if let Some(ref client) = *guard {
            client.send_mouse_button(x_percent, y_percent, &button, pressed);
        }
    }
}

#[tauri::command]
fn input_wheel(state: State<'_, AppState>, dx: i32, dy: i32) {
    if let Ok(guard) = state.input_client.lock() {
        if let Some(ref client) = *guard {
            client.send_wheel(dx, dy);
        }
    }
}

#[tauri::command]
fn input_key(state: State<'_, AppState>, linux_keycode: u16, state_val: i32) {
    if let Ok(guard) = state.input_client.lock() {
        if let Some(ref client) = *guard {
            client.send_key(linux_keycode, state_val);
        }
    }
}

#[tauri::command]
async fn input_key_combo(state: State<'_, AppState>, keycodes: Vec<u16>) -> Result<(), String> {
    let client_opt = state.input_client.lock().ok().and_then(|g| g.clone());
    if let Some(client) = client_opt {
        let _ = tokio::task::spawn_blocking(move || {
            client.send_key_combo(&keycodes);
        })
        .await;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let video_pipeline = VideoPipeline::start_server();
            let store = PersistedStore::load_default();
            app.manage(AppState {
                store: StdMutex::new(store),
                active_device: StdMutex::new(None),
                active_host: StdMutex::new(None),
                input_client: StdMutex::new(None),
                ssh_client: AsyncMutex::new(None),
                video_pipeline,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_devices,
            save_device,
            delete_device,
            get_settings,
            save_settings,
            get_tailscale_status,
            toggle_tailscale,
            send_wol,
            wake_and_wait_device,
            connect_device,
            disconnect_device,
            get_monitors,
            switch_monitor,
            restart_video_stream,
            get_video_endpoint,
            get_stream_stats,
            get_latest_video_frame,
            measure_latency,
            execute_ssh_command,
            get_ssh_cwd,
            get_desktop_metrics,
            input_mouse_move,
            input_mouse_click,
            input_mouse_button,
            input_wheel,
            input_key,
            input_key_combo,
        ])
        .run(tauri::generate_context!())
        .expect("Error ejecutando la aplicación Tauri Molex");
}
