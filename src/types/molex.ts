export interface DeviceProfile {
  id: string;
  name: string;
  host: string;
  tailscale_ip: string | null;
  mac_address: string | null;
  broadcast_ip: string | null;
  ssh_port: number;
  video_port: number;
  input_port: number;
  username: string;
  password: string | null;
}

export interface MonitorInfo {
  name: string;
  width: number;
  height: number;
  refresh_hz: number;
  focused: boolean;
  compositor: string;
}

export interface TailscalePeer {
  id: string;
  hostname: string;
  dns_name: string;
  os: string;
  tailscale_ips: string[];
  primary_ip: string | null;
  online: boolean;
  active: boolean;
  cur_addr: string;
  relay: string;
}

export interface TailscaleStatus {
  backend_state: string;
  is_running: boolean;
  auth_url: string | null;
  self_hostname: string;
  self_ips: string[];
  peers: TailscalePeer[];
}

export type WakeStage =
  | "packet_sent"
  | "waiting_for_network"
  | "vpn_online_waiting_daemon"
  | "ready"
  | "timeout";

export interface WakeProgress {
  device_id: string;
  stage: WakeStage;
  attempt: number;
  max_attempts: number;
  tailscale_online: boolean;
  ssh_ready: boolean;
  video_ready: boolean;
  resolved_host: string | null;
  message: string;
}

export interface WakeResult {
  device_id: string;
  resolved_host: string;
  via_tailscale: boolean;
  ssh_ready: boolean;
  video_ready: boolean;
  elapsed_secs: number;
}

export interface StreamStats {
  active: boolean;
  connected: boolean;
  host: string;
  port: number;
  width: number;
  height: number;
  fps: number;
  decode_ms: number;
  kbps: number;
  frame_seq: number;
  error: string | null;
}

export interface VideoEndpointInfo {
  ws_url: string;
  token: string;
  stats: StreamStats;
}

export interface ConnectionSummary {
  device: DeviceProfile;
  host_used: string;
  ssh_connected: boolean;
  ssh_error: string | null;
  monitors: MonitorInfo[];
  cwd: string;
  video_endpoint: VideoEndpointInfo;
}

export interface CommandResult {
  output: string;
  cwd: string;
  exit_code: number | null;
}

export interface DesktopMetrics {
  os_info: string;
  hostname: string;
  uptime: string;
  cpu_model: string;
  cpu_percent: number;
  ram_used_mb: number;
  ram_total_mb: number;
  ram_usage: string;
  gpu_info: string;
  gpu_temp_c: number | null;
  gpu_util_percent: number | null;
  gpu_vram_usage: string | null;
  network_status: string;
  compositor: string;
}

export interface DesktopSettings {
  target_fps: number;
  auto_connect_after_wol: boolean;
  hide_local_cursor_in_stream: boolean;
  sticky_super_default: boolean;
  decoder_threads: number;
}

export type NavTab = "devices" | "remote" | "ssh" | "metrics" | "settings";

export interface TerminalEntry {
  id: string;
  kind: "command" | "output" | "error" | "info";
  prompt?: string;
  text: string;
  timestamp: string;
}
