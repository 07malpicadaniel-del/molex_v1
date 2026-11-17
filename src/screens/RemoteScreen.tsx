import React, { useCallback, useEffect, useRef, useState } from "react";
import {
  AlertTriangle,
  ChevronDown,
  ChevronUp,
  Keyboard,
  Maximize2,
  Minimize2,
  Monitor,
  Power,
  RefreshCw,
  Shield,
  Terminal,
  Wifi,
} from "lucide-react";
import { YuvCanvas } from "../components/YuvCanvas";
import {
  LINUX_KEY_LEFTMETA,
  WM_COMBO_PRESETS,
  mapBrowserKeyCode,
} from "../lib/evdevKeymap";
import { molexApi } from "../lib/tauriApi";
import type {
  DesktopSettings,
  DeviceProfile,
  MonitorInfo,
  StreamStats,
  TailscaleStatus,
  VideoEndpointInfo,
} from "../types/molex";

interface RemoteScreenProps {
  activeDevice: DeviceProfile | null;
  activeHost: string | null;
  monitors: MonitorInfo[];
  selectedMonitor: string | null;
  videoEndpoint: VideoEndpointInfo | null;
  tailscaleStatus: TailscaleStatus | null;
  settings: DesktopSettings;
  onSelectMonitor: (name: string) => Promise<void>;
  onRefreshMonitors: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onNavigateDevices: () => void;
  onNavigateSsh: () => void;
}

export const RemoteScreen: React.FC<RemoteScreenProps> = ({
  activeDevice,
  activeHost,
  monitors,
  selectedMonitor,
  videoEndpoint,
  tailscaleStatus,
  settings,
  onSelectMonitor,
  onRefreshMonitors,
  onDisconnect,
  onNavigateDevices,
  onNavigateSsh,
}) => {
  const viewportRef = useRef<HTMLDivElement | null>(null);
  const pressedKeysRef = useRef<Set<number>>(new Set());

  const [hudVisible, setHudVisible] = useState(true);
  const [monitorMenuOpen, setMonitorMenuOpen] = useState(false);
  const [combosMenuOpen, setCombosMenuOpen] = useState(false);
  const [stickySuper, setStickySuper] = useState(settings.sticky_super_default);
  const [isFullscreen, setIsFullscreen] = useState(false);

  const [stats, setStats] = useState<StreamStats | null>(videoEndpoint?.stats ?? null);
  const [canvasMetrics, setCanvasMetrics] = useState<{
    width: number;
    height: number;
    fps: number;
  } | null>(null);
  const [rttMs, setRttMs] = useState<number | null>(null);

  useEffect(() => {
    setStickySuper(settings.sticky_super_default);
  }, [settings.sticky_super_default]);

  const releaseAllKeys = useCallback(() => {
    if (pressedKeysRef.current.size === 0) return;
    pressedKeysRef.current.forEach((code) => {
      molexApi.inputKey(code, 0);
    });
    pressedKeysRef.current.clear();
  }, []);

  useEffect(() => {
    return () => {
      releaseAllKeys();
    };
  }, [releaseAllKeys]);

  // Poll stream stats y latencia RTT cada segundo mientras la vista remota está activa
  useEffect(() => {
    if (!activeDevice || !activeHost) return;
    let cancelled = false;

    const poll = async () => {
      try {
        const [st, ping] = await Promise.all([
          molexApi.getStreamStats(),
          molexApi.measureLatency(activeHost, activeDevice.video_port).catch(() => null),
        ]);
        if (!cancelled) {
          setStats(st);
          if (ping !== null) setRttMs(ping);
        }
      } catch {
        // Ignorar errores transitorios
      }
    };

    poll();
    const id = window.setInterval(poll, 1000);
    return () => {
      cancelled = true;
      clearInterval(id);
    };
  }, [activeDevice, activeHost]);

  const handleFrameMetrics = useCallback(
    (m: { width: number; height: number; fps: number }) => {
      setCanvasMetrics(m);
    },
    []
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const linuxCode = mapBrowserKeyCode(e.code);
    if (linuxCode === null) return;

    if (e.repeat) {
      molexApi.inputKey(linuxCode, 2);
      return;
    }

    if (stickySuper && linuxCode !== LINUX_KEY_LEFTMETA) {
      molexApi.inputKey(LINUX_KEY_LEFTMETA, 1);
      molexApi.inputKey(linuxCode, 1);
      pressedKeysRef.current.add(LINUX_KEY_LEFTMETA);
      pressedKeysRef.current.add(linuxCode);
      return;
    }

    pressedKeysRef.current.add(linuxCode);
    molexApi.inputKey(linuxCode, 1);
  };

  const handleKeyUp = (e: React.KeyboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    const linuxCode = mapBrowserKeyCode(e.code);
    if (linuxCode === null) return;

    pressedKeysRef.current.delete(linuxCode);
    molexApi.inputKey(linuxCode, 0);

    if (stickySuper && linuxCode !== LINUX_KEY_LEFTMETA) {
      pressedKeysRef.current.delete(LINUX_KEY_LEFTMETA);
      molexApi.inputKey(LINUX_KEY_LEFTMETA, 0);
    }
  };

  const sendCombo = async (keycodes: number[]) => {
    setCombosMenuOpen(false);
    await molexApi.inputKeyCombo(keycodes);
    viewportRef.current?.focus();
  };

  const toggleFullscreen = async () => {
    const next = !isFullscreen;
    setIsFullscreen(next);
    await molexApi.setFullscreen(next);
  };

  if (!activeDevice || !activeHost) {
    return (
      <div className="flex h-full flex-col items-center justify-center bg-molex-bg p-8 text-center">
        <Monitor className="h-12 w-12 text-molex-accent" />
        <h2 className="mt-4 text-xl font-bold text-molex-text">Sin sesión remota activa</h2>
        <p className="mt-1 max-w-md text-sm text-molex-muted">
          Selecciona un equipo guardado o un nodo de Tailscale en la pestaña Dispositivos para
          iniciar la transmisión H.264 a 60 FPS.
        </p>
        <button
          type="button"
          onClick={onNavigateDevices}
          className="mt-6 rounded-lg bg-molex-accent px-5 py-2.5 text-sm font-semibold text-molex-bg hover:bg-molex-light"
        >
          Ir a Dispositivos
        </button>
      </div>
    );
  }

  const activeMonitorObj = monitors.find((m) => m.name === selectedMonitor);
  const displayFps = canvasMetrics?.fps ?? stats?.fps ?? 0;
  const displayW = canvasMetrics?.width || stats?.width || activeMonitorObj?.width || 0;
  const displayH = canvasMetrics?.height || stats?.height || activeMonitorObj?.height || 0;
  const isViaTailscale = activeHost.startsWith("100.");

  return (
    <div
      ref={viewportRef}
      tabIndex={0}
      onKeyDown={handleKeyDown}
      onKeyUp={handleKeyUp}
      onBlur={releaseAllKeys}
      className="relative flex h-full w-full flex-col bg-black focus:outline-none"
    >
      {/* Video Viewport */}
      <YuvCanvas
        wsUrl={videoEndpoint?.ws_url ?? null}
        hideLocalCursor={settings.hide_local_cursor_in_stream}
        onMouseMoveNorm={(x, y) => molexApi.inputMouseMove(x, y)}
        onMouseButtonNorm={(x, y, btn, pressed) =>
          molexApi.inputMouseButton(x, y, btn, pressed)
        }
        onWheelInput={(dx, dy) => molexApi.inputWheel(dx, dy)}
        onFrameMetrics={handleFrameMetrics}
        onFocusCanvas={() => viewportRef.current?.focus()}
      />

      {/* Connecting / Error Non-Blocking Banner */}
      {stats && !stats.connected && (
        <div className="pointer-events-auto absolute inset-0 flex items-center justify-center bg-black/75 backdrop-blur-sm">
          <div className="max-w-md rounded-2xl border border-molex-variant bg-molex-surface p-6 text-center shadow-2xl">
            <AlertTriangle className="mx-auto h-10 w-10 text-amber-400" />
            <h3 className="mt-3 text-base font-bold text-molex-text">
              Esperando stream H.264 en {activeHost}:{activeDevice.video_port}
            </h3>
            <p className="mt-1.5 text-xs text-molex-muted">
              {stats.error ||
                "Conectando al socket TCP de wl-screenrec en molex_daemon..."}
            </p>
            <div className="mt-5 flex flex-wrap justify-center gap-2.5">
              <button
                type="button"
                onClick={() => molexApi.restartVideoStream()}
                className="inline-flex items-center gap-1.5 rounded-lg bg-molex-accent px-4 py-2 text-xs font-semibold text-molex-bg hover:bg-molex-light"
              >
                <RefreshCw className="h-3.5 w-3.5" />
                Reintentar Video
              </button>
              <button
                type="button"
                onClick={onNavigateSsh}
                className="inline-flex items-center gap-1.5 rounded-lg border border-molex-variant bg-molex-bg px-4 py-2 text-xs font-medium text-molex-text hover:bg-molex-variant"
              >
                <Terminal className="h-3.5 w-3.5" />
                Abrir Terminal SSH
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Floating Top HUD Bar */}
      <div className="pointer-events-none absolute inset-x-0 top-3 z-30 flex flex-col items-center px-4">
        {hudVisible ? (
          <div className="pointer-events-auto flex flex-wrap items-center gap-2 rounded-2xl border border-white/10 bg-molex-surface/90 px-3 py-2 shadow-2xl backdrop-blur-md">
            {/* Monitor Selector */}
            <div className="relative">
              <button
                type="button"
                onClick={() => {
                  setMonitorMenuOpen(!monitorMenuOpen);
                  setCombosMenuOpen(false);
                }}
                className="inline-flex items-center gap-1.5 rounded-lg bg-molex-bg/90 px-2.5 py-1.5 text-xs font-medium text-molex-text hover:bg-molex-variant"
              >
                <Monitor className="h-3.5 w-3.5 text-molex-accent" />
                <span>{selectedMonitor || "Monitor auto"}</span>
                <ChevronDown className="h-3.5 w-3.5 text-molex-muted" />
              </button>

              {monitorMenuOpen && (
                <div className="absolute left-0 top-full mt-2 w-60 rounded-xl border border-molex-variant bg-molex-surface p-1.5 shadow-2xl">
                  <div className="flex items-center justify-between px-2.5 py-1 text-[10px] font-semibold uppercase tracking-wider text-molex-muted">
                    <span>Monitores Wayland</span>
                    <button
                      type="button"
                      onClick={() => onRefreshMonitors()}
                      className="rounded p-1 hover:bg-molex-variant hover:text-molex-text"
                    >
                      <RefreshCw className="h-3 w-3" />
                    </button>
                  </div>
                  {monitors.length === 0 ? (
                    <div className="px-2.5 py-2 text-xs text-molex-muted">
                      Sin monitores detectados por SSH
                    </div>
                  ) : (
                    monitors.map((m) => (
                      <button
                        key={m.name}
                        type="button"
                        onClick={async () => {
                          setMonitorMenuOpen(false);
                          await onSelectMonitor(m.name);
                          viewportRef.current?.focus();
                        }}
                        className={`flex w-full items-center justify-between rounded-lg px-2.5 py-2 text-left text-xs ${
                          m.name === selectedMonitor
                            ? "bg-molex-accent/20 font-semibold text-molex-light"
                            : "text-molex-text hover:bg-molex-variant"
                        }`}
                      >
                        <span>
                          {m.name}{" "}
                          <span className="text-[10px] text-molex-muted">({m.compositor})</span>
                        </span>
                        <span className="font-mono text-[10px] text-molex-muted">
                          {m.width}×{m.height}@{m.refresh_hz}
                        </span>
                      </button>
                    ))
                  )}
                </div>
              )}
            </div>

            <div className="h-5 w-px bg-molex-variant" />

            {/* Quick WM Shortcuts */}
            <button
              type="button"
              onClick={() => sendCombo(WM_COMBO_PRESETS[0].keycodes)}
              title="Enviar Super + Enter al compositor remoto"
              className="rounded-lg bg-molex-variant/80 px-2.5 py-1.5 font-mono text-[11px] font-semibold text-molex-light hover:bg-molex-accent hover:text-molex-bg"
            >
              Super+Enter
            </button>
            <button
              type="button"
              onClick={() => sendCombo(WM_COMBO_PRESETS[1].keycodes)}
              title="Enviar Ctrl + Alt + T"
              className="rounded-lg bg-molex-variant/80 px-2.5 py-1.5 font-mono text-[11px] font-semibold text-molex-light hover:bg-molex-accent hover:text-molex-bg"
            >
              Ctrl+Alt+T
            </button>

            {/* More combos dropdown */}
            <div className="relative">
              <button
                type="button"
                onClick={() => {
                  setCombosMenuOpen(!combosMenuOpen);
                  setMonitorMenuOpen(false);
                }}
                className="inline-flex items-center gap-1 rounded-lg bg-molex-bg/90 px-2.5 py-1.5 text-xs font-medium text-molex-text hover:bg-molex-variant"
              >
                <Keyboard className="h-3.5 w-3.5 text-molex-accent" />
                <span>Combos</span>
                <ChevronDown className="h-3.5 w-3.5 text-molex-muted" />
              </button>
              {combosMenuOpen && (
                <div className="absolute left-0 top-full mt-2 w-60 rounded-xl border border-molex-variant bg-molex-surface p-1.5 shadow-2xl">
                  {WM_COMBO_PRESETS.map((combo) => (
                    <button
                      key={combo.id}
                      type="button"
                      onClick={() => sendCombo(combo.keycodes)}
                      className="flex w-full items-center justify-between rounded-lg px-2.5 py-2 text-left text-xs text-molex-text hover:bg-molex-variant"
                    >
                      <span className="font-mono font-semibold text-molex-light">
                        {combo.label}
                      </span>
                      <span className="truncate pl-2 text-[10px] text-molex-muted">
                        {combo.description}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Sticky Super Toggle */}
            <button
              type="button"
              onClick={() => {
                setStickySuper(!stickySuper);
                viewportRef.current?.focus();
              }}
              title="Sticky Super: antepone tecla Super remota a cada tecla presionada"
              className={`rounded-lg px-2.5 py-1.5 text-xs font-semibold transition ${
                stickySuper
                  ? "bg-amber-500/25 text-amber-200 ring-1 ring-amber-400/50"
                  : "bg-molex-bg/90 text-molex-muted hover:bg-molex-variant hover:text-molex-text"
              }`}
            >
              Super Lock {stickySuper ? "ON" : "OFF"}
            </button>

            <div className="h-5 w-px bg-molex-variant" />

            {/* VPN & Latency Badges */}
            <div className="inline-flex items-center gap-1.5 rounded-lg bg-molex-bg/90 px-2.5 py-1.5 text-[11px]">
              {isViaTailscale || tailscaleStatus?.is_running ? (
                <Shield className="h-3.5 w-3.5 text-emerald-400" />
              ) : (
                <Wifi className="h-3.5 w-3.5 text-molex-accent" />
              )}
              <span className="font-medium text-molex-text">
                {isViaTailscale
                  ? "Tailscale"
                  : tailscaleStatus?.is_running
                    ? "LAN · VPN Activa"
                    : "LAN Directo"}
              </span>
            </div>

            <div className="inline-flex items-center gap-2 rounded-lg bg-molex-bg/90 px-2.5 py-1.5 font-mono text-[11px] text-molex-light">
              <span className="text-emerald-400">{displayFps.toFixed(0)} FPS</span>
              {displayW > 0 && (
                <span className="text-molex-muted">
                  {displayW}×{displayH}
                </span>
              )}
              {rttMs !== null && <span className="text-amber-300">{rttMs} ms</span>}
              {stats && stats.decode_ms > 0 && (
                <span className="text-molex-muted">dec {stats.decode_ms.toFixed(1)}ms</span>
              )}
            </div>

            {/* Fullscreen & Disconnect & Collapse HUD */}
            <button
              type="button"
              onClick={toggleFullscreen}
              title={isFullscreen ? "Salir de pantalla completa" : "Pantalla completa"}
              className="rounded-lg bg-molex-bg/90 p-1.5 text-molex-text hover:bg-molex-variant"
            >
              {isFullscreen ? (
                <Minimize2 className="h-4 w-4" />
              ) : (
                <Maximize2 className="h-4 w-4" />
              )}
            </button>

            <button
              type="button"
              onClick={onDisconnect}
              title="Desconectar sesión"
              className="inline-flex items-center gap-1 rounded-lg bg-red-500/20 px-2.5 py-1.5 text-xs font-semibold text-red-300 hover:bg-red-500/30"
            >
              <Power className="h-3.5 w-3.5" />
              <span>Salir</span>
            </button>

            <button
              type="button"
              onClick={() => setHudVisible(false)}
              title="Ocultar barra flotante"
              className="rounded-lg p-1.5 text-molex-muted hover:bg-molex-variant hover:text-molex-text"
            >
              <ChevronUp className="h-4 w-4" />
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={() => setHudVisible(true)}
            className="pointer-events-auto inline-flex items-center gap-1.5 rounded-full border border-white/10 bg-molex-surface/80 px-3 py-1 text-xs text-molex-light backdrop-blur hover:bg-molex-variant"
          >
            <ChevronDown className="h-3.5 w-3.5 text-molex-accent" />
            <span>{activeDevice.name} · {displayFps.toFixed(0)} FPS</span>
          </button>
        )}
      </div>
    </div>
  );
};
