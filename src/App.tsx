import { useCallback, useEffect, useState } from "react";
import { Sidebar } from "./components/Sidebar";
import { molexApi } from "./lib/tauriApi";
import { DevicesScreen } from "./screens/DevicesScreen";
import { MetricsScreen } from "./screens/MetricsScreen";
import { RemoteScreen } from "./screens/RemoteScreen";
import { SettingsScreen } from "./screens/SettingsScreen";
import { SshScreen } from "./screens/SshScreen";
import type {
  DesktopSettings,
  DeviceProfile,
  MonitorInfo,
  NavTab,
  TailscaleStatus,
  TerminalEntry,
  VideoEndpointInfo,
  WakeProgress,
} from "./types/molex";

const DEFAULT_SETTINGS: DesktopSettings = {
  target_fps: 60,
  auto_connect_after_wol: true,
  hide_local_cursor_in_stream: false,
  sticky_super_default: false,
  decoder_threads: 4,
};

export function App() {
  const [activeTab, setActiveTab] = useState<NavTab>("devices");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const [devices, setDevices] = useState<DeviceProfile[]>([]);
  const [settings, setSettings] = useState<DesktopSettings>(DEFAULT_SETTINGS);

  const [tailscaleStatus, setTailscaleStatus] = useState<TailscaleStatus | null>(null);
  const [tailscaleLoading, setTailscaleLoading] = useState(false);
  const [tailscaleError, setTailscaleError] = useState<string | null>(null);

  const [wakingDevices, setWakingDevices] = useState<Record<string, WakeProgress>>({});
  const [connectingDeviceId, setConnectingDeviceId] = useState<string | null>(null);

  const [activeDevice, setActiveDevice] = useState<DeviceProfile | null>(null);
  const [activeHost, setActiveHost] = useState<string | null>(null);
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [selectedMonitor, setSelectedMonitor] = useState<string | null>(null);
  const [videoEndpoint, setVideoEndpoint] = useState<VideoEndpointInfo | null>(null);

  const [sshCwd, setSshCwd] = useState<string>("");
  const [terminalLogs, setTerminalLogs] = useState<TerminalEntry[]>([]);

  const refreshTailscale = useCallback(async () => {
    setTailscaleLoading(true);
    try {
      const status = await molexApi.getTailscaleStatus();
      setTailscaleStatus(status);
      setTailscaleError(null);
    } catch (err) {
      setTailscaleError(String(err));
    } finally {
      setTailscaleLoading(false);
    }
  }, []);

  useEffect(() => {
    molexApi.listDevices().then(setDevices).catch(console.error);
    molexApi.getSettings().then(setSettings).catch(console.error);
    refreshTailscale();

    let unlisten: (() => void) | null = null;
    molexApi
      .onWakeProgress((progress) => {
        setWakingDevices((prev) => ({ ...prev, [progress.device_id]: progress }));
      })
      .then((fn) => {
        unlisten = fn;
      })
      .catch(console.error);

    return () => {
      unlisten?.();
    };
  }, [refreshTailscale]);

  const handleToggleTailscale = async (enable: boolean) => {
    setTailscaleLoading(true);
    try {
      const status = await molexApi.toggleTailscale(enable);
      setTailscaleStatus(status);
      setTailscaleError(null);
    } catch (err) {
      setTailscaleError(String(err));
    } finally {
      setTailscaleLoading(false);
    }
  };

  const handleSaveDevice = async (device: DeviceProfile) => {
    const updated = await molexApi.saveDevice(device);
    setDevices(updated);
  };

  const handleDeleteDevice = async (deviceId: string) => {
    const updated = await molexApi.deleteDevice(deviceId);
    setDevices(updated);
    if (activeDevice?.id === deviceId) {
      await handleDisconnect();
    }
  };

  const handleConnectDevice = async (profile: DeviceProfile) => {
    setConnectingDeviceId(profile.id);
    try {
      const summary = await molexApi.connectDevice(profile);
      setActiveDevice(summary.device);
      setActiveHost(summary.host_used);
      setMonitors(summary.monitors);
      const focused =
        summary.monitors.find((m) => m.focused)?.name ?? summary.monitors[0]?.name ?? null;
      setSelectedMonitor(focused);
      setVideoEndpoint(summary.video_endpoint);
      setSshCwd(summary.cwd);

      const now = new Date().toLocaleTimeString();
      const bootEntries: TerminalEntry[] = [
        {
          id: `${Date.now()}-conn`,
          kind: summary.ssh_connected ? "info" : "error",
          text: summary.ssh_connected
            ? `Conectado por SSH a ${summary.device.username}@${summary.host_used} (${summary.cwd || "~"})`
            : `SSH no disponible: ${summary.ssh_error || "Error de autenticación/red"}`,
          timestamp: now,
        },
      ];
      setTerminalLogs(bootEntries);
      setActiveTab("remote");
    } finally {
      setConnectingDeviceId(null);
    }
  };

  const handleWakeDevice = async (profile: DeviceProfile) => {
    const result = await molexApi.wakeAndWaitDevice(profile);
    if (settings.auto_connect_after_wol) {
      const resolvedProfile: DeviceProfile = result.via_tailscale
        ? {
            ...profile,
            host: profile.host || result.resolved_host,
            tailscale_ip: profile.tailscale_ip || result.resolved_host,
          }
        : {
            ...profile,
            host: result.resolved_host || profile.host,
          };
      await handleConnectDevice(resolvedProfile);
    }
  };

  const handleDisconnect = async () => {
    await molexApi.disconnectDevice();
    await molexApi.setFullscreen(false);
    setActiveDevice(null);
    setActiveHost(null);
    setMonitors([]);
    setSelectedMonitor(null);
    setVideoEndpoint(null);
    setSshCwd("");
    setActiveTab("devices");
  };

  const handleSelectMonitor = async (monitorName: string) => {
    setSelectedMonitor(monitorName);
    await molexApi.switchMonitor(monitorName);
    const endpoint = await molexApi.getVideoEndpoint();
    setVideoEndpoint(endpoint);
  };

  const handleRefreshMonitors = async () => {
    const list = await molexApi.getMonitors();
    setMonitors(list);
    if (!selectedMonitor && list.length > 0) {
      setSelectedMonitor(list.find((m) => m.focused)?.name ?? list[0].name);
    }
  };

  const handleExecuteSshCommand = async (cmd: string) => {
    if (!activeDevice || !activeHost) return;
    const promptSnapshot = `${activeDevice.username}@${activeHost}:${sshCwd || "~"} $`;
    const ts = new Date().toLocaleTimeString();

    setTerminalLogs((prev) => [
      ...prev,
      {
        id: `${Date.now()}-cmd`,
        kind: "command",
        prompt: promptSnapshot,
        text: cmd,
        timestamp: ts,
      },
    ]);

    try {
      const res = await molexApi.executeSshCommand(cmd);
      setSshCwd(res.cwd);
      setTerminalLogs((prev) => [
        ...prev,
        {
          id: `${Date.now()}-out`,
          kind: res.exit_code !== null && res.exit_code !== 0 ? "error" : "output",
          text: res.output,
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
    } catch (err) {
      setTerminalLogs((prev) => [
        ...prev,
        {
          id: `${Date.now()}-err`,
          kind: "error",
          text: String(err),
          timestamp: new Date().toLocaleTimeString(),
        },
      ]);
    }
  };

  const handleUpdateSettings = async (next: DesktopSettings) => {
    const saved = await molexApi.saveSettings(next);
    setSettings(saved);
  };

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-molex-bg text-molex-text">
      <Sidebar
        activeTab={activeTab}
        onSelectTab={setActiveTab}
        collapsed={sidebarCollapsed}
        onToggleCollapse={() => setSidebarCollapsed((c) => !c)}
        activeDevice={activeDevice}
        activeHost={activeHost}
        tailscaleStatus={tailscaleStatus}
        onDisconnect={handleDisconnect}
      />

      <main className="min-w-0 flex-1 overflow-hidden">
        {activeTab === "devices" && (
          <DevicesScreen
            devices={devices}
            tailscaleStatus={tailscaleStatus}
            tailscaleLoading={tailscaleLoading}
            tailscaleError={tailscaleError}
            wakingDevices={wakingDevices}
            connectingDeviceId={connectingDeviceId}
            activeDeviceId={activeDevice?.id ?? null}
            onRefreshTailscale={refreshTailscale}
            onToggleTailscale={handleToggleTailscale}
            onSaveDevice={handleSaveDevice}
            onDeleteDevice={handleDeleteDevice}
            onConnectDevice={handleConnectDevice}
            onWakeDevice={handleWakeDevice}
          />
        )}

        {activeTab === "remote" && (
          <RemoteScreen
            activeDevice={activeDevice}
            activeHost={activeHost}
            monitors={monitors}
            selectedMonitor={selectedMonitor}
            videoEndpoint={videoEndpoint}
            tailscaleStatus={tailscaleStatus}
            settings={settings}
            onSelectMonitor={handleSelectMonitor}
            onRefreshMonitors={handleRefreshMonitors}
            onDisconnect={handleDisconnect}
            onNavigateDevices={() => setActiveTab("devices")}
            onNavigateSsh={() => setActiveTab("ssh")}
          />
        )}

        {activeTab === "ssh" && (
          <SshScreen
            activeDevice={activeDevice}
            activeHost={activeHost}
            cwd={sshCwd}
            logs={terminalLogs}
            onExecuteCommand={handleExecuteSshCommand}
            onClearLogs={() => setTerminalLogs([])}
            onNavigateDevices={() => setActiveTab("devices")}
          />
        )}

        {activeTab === "metrics" && (
          <MetricsScreen
            activeDevice={activeDevice}
            activeHost={activeHost}
            onNavigateDevices={() => setActiveTab("devices")}
          />
        )}

        {activeTab === "settings" && (
          <SettingsScreen settings={settings} onUpdateSettings={handleUpdateSettings} />
        )}
      </main>
    </div>
  );
}

export default App;
