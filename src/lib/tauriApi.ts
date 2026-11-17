import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  CommandResult,
  ConnectionSummary,
  DesktopMetrics,
  DesktopSettings,
  DeviceProfile,
  MonitorInfo,
  StreamStats,
  TailscaleStatus,
  VideoEndpointInfo,
  WakeProgress,
  WakeResult,
} from "../types/molex";

export const molexApi = {
  listDevices: () => invoke<DeviceProfile[]>("list_devices"),
  saveDevice: (device: DeviceProfile) => invoke<DeviceProfile[]>("save_device", { device }),
  deleteDevice: (deviceId: string) => invoke<DeviceProfile[]>("delete_device", { deviceId }),

  getSettings: () => invoke<DesktopSettings>("get_settings"),
  saveSettings: (settings: DesktopSettings) =>
    invoke<DesktopSettings>("save_settings", { settings }),

  getTailscaleStatus: () => invoke<TailscaleStatus>("get_tailscale_status"),
  toggleTailscale: (enable: boolean) => invoke<TailscaleStatus>("toggle_tailscale", { enable }),

  sendWol: (mac: string, broadcastIp?: string | null) =>
    invoke<void>("send_wol", { mac, broadcastIp: broadcastIp ?? null }),
  wakeAndWaitDevice: (profile: DeviceProfile) =>
    invoke<WakeResult>("wake_and_wait_device", { profile }),
  onWakeProgress: (handler: (progress: WakeProgress) => void): Promise<UnlistenFn> =>
    listen<WakeProgress>("wol-progress", (e) => handler(e.payload)),

  connectDevice: (profile: DeviceProfile) =>
    invoke<ConnectionSummary>("connect_device", { profile }),
  disconnectDevice: () => invoke<void>("disconnect_device"),

  getMonitors: () => invoke<MonitorInfo[]>("get_monitors"),
  switchMonitor: (monitorName: string) => invoke<void>("switch_monitor", { monitorName }),
  restartVideoStream: () => invoke<void>("restart_video_stream"),
  getVideoEndpoint: () => invoke<VideoEndpointInfo>("get_video_endpoint"),
  getStreamStats: () => invoke<StreamStats>("get_stream_stats"),
  measureLatency: (host: string, port: number) =>
    invoke<number>("measure_latency", { host, port }),

  executeSshCommand: (command: string) =>
    invoke<CommandResult>("execute_ssh_command", { command }),
  getSshCwd: () => invoke<string>("get_ssh_cwd"),
  getDesktopMetrics: () => invoke<DesktopMetrics>("get_desktop_metrics"),

  inputMouseMove: (xPercent: number, yPercent: number) =>
    invoke<void>("input_mouse_move", { xPercent, yPercent }).catch(() => {}),
  inputMouseClick: (xPercent: number, yPercent: number, button: "LEFT" | "RIGHT" | "MIDDLE") =>
    invoke<void>("input_mouse_click", { xPercent, yPercent, button }).catch(() => {}),
  inputMouseButton: (
    xPercent: number,
    yPercent: number,
    button: "LEFT" | "RIGHT" | "MIDDLE",
    pressed: boolean
  ) => invoke<void>("input_mouse_button", { xPercent, yPercent, button, pressed }).catch(() => {}),
  inputWheel: (dx: number, dy: number) =>
    invoke<void>("input_wheel", { dx, dy }).catch(() => {}),
  inputKey: (linuxKeycode: number, stateVal: 0 | 1 | 2) =>
    invoke<void>("input_key", { linuxKeycode, stateVal }).catch(() => {}),
  inputKeyCombo: (keycodes: number[]) =>
    invoke<void>("input_key_combo", { keycodes }).catch(() => {}),

  setFullscreen: async (fullscreen: boolean) => {
    try {
      await getCurrentWindow().setFullscreen(fullscreen);
    } catch {
      // Ignorado si se ejecuta en navegador fuera de Tauri
    }
  },
};
