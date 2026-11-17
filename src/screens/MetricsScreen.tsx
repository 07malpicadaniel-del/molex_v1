import React, { useEffect, useState } from "react";
import { Activity, Cpu, HardDrive, Monitor, Network, RefreshCw, Thermometer } from "lucide-react";
import { molexApi } from "../lib/tauriApi";
import type { DesktopMetrics, DeviceProfile } from "../types/molex";

interface MetricsScreenProps {
  activeDevice: DeviceProfile | null;
  activeHost: string | null;
  onNavigateDevices: () => void;
}

export const MetricsScreen: React.FC<MetricsScreenProps> = ({
  activeDevice,
  activeHost,
  onNavigateDevices,
}) => {
  const [metrics, setMetrics] = useState<DesktopMetrics | null>(null);
  const [cpuHistory, setCpuHistory] = useState<number[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const fetchMetrics = async () => {
    if (!activeDevice) return;
    setLoading(true);
    try {
      const m = await molexApi.getDesktopMetrics();
      setMetrics(m);
      setCpuHistory((prev) => [...prev.slice(-29), m.cpu_percent]);
      setError(null);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!activeDevice) {
      setMetrics(null);
      setCpuHistory([]);
      return;
    }
    fetchMetrics();
    const id = window.setInterval(fetchMetrics, 1500);
    return () => clearInterval(id);
  }, [activeDevice?.id]);

  if (!activeDevice || !activeHost) {
    return (
      <div className="flex h-full flex-col items-center justify-center bg-molex-bg p-8 text-center">
        <Activity className="h-12 w-12 text-molex-accent" />
        <h2 className="mt-4 text-xl font-bold text-molex-text">Métricas del Sistema</h2>
        <p className="mt-1 max-w-md text-sm text-molex-muted">
          Conéctate a un equipo remoto para monitorear CPU, RAM, GPU NVIDIA y red en tiempo real.
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

  const ramPct =
    metrics && metrics.ram_total_mb > 0
      ? Math.min(100, Math.round((metrics.ram_used_mb / metrics.ram_total_mb) * 100))
      : 0;

  const sparklinePoints =
    cpuHistory.length > 1
      ? cpuHistory
          .map((val, idx) => {
            const x = (idx / (cpuHistory.length - 1)) * 220;
            const y = 44 - (Math.min(100, Math.max(0, val)) / 100) * 38;
            return `${x.toFixed(1)},${y.toFixed(1)}`;
          })
          .join(" ")
      : "0,44 220,44";

  return (
    <div className="flex h-full flex-col overflow-y-auto bg-molex-bg p-6">
      <div className="mx-auto w-full max-w-5xl space-y-6">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold text-molex-text">Monitor de Recursos</h1>
            <p className="mt-1 font-mono text-xs text-molex-muted">
              Telemetría SSH en vivo · {activeDevice.username}@{activeHost}
            </p>
          </div>
          <button
            type="button"
            onClick={fetchMetrics}
            disabled={loading}
            className="inline-flex items-center gap-1.5 rounded-lg border border-molex-variant bg-molex-surface px-3.5 py-2 text-xs font-medium text-molex-text hover:bg-molex-variant"
          >
            <RefreshCw className={`h-3.5 w-3.5 ${loading ? "animate-spin" : ""}`} />
            Refrescar
          </button>
        </div>

        {error && (
          <div className="rounded-xl border border-red-500/40 bg-red-950/30 p-4 text-sm text-red-200">
            {error}
          </div>
        )}

        {!metrics && !error ? (
          <div className="flex items-center justify-center rounded-xl border border-molex-variant bg-molex-surface py-16 text-sm text-molex-muted">
            Recolectando telemetría del sistema remoto...
          </div>
        ) : (
          metrics && (
            <>
              {/* System Header Card */}
              <div className="grid gap-4 sm:grid-cols-4">
                <div className="rounded-xl border border-molex-variant bg-molex-surface p-4">
                  <span className="text-[11px] font-semibold uppercase text-molex-muted">
                    Hostname
                  </span>
                  <p className="mt-1 truncate text-base font-bold text-molex-text">
                    {metrics.hostname}
                  </p>
                </div>
                <div className="rounded-xl border border-molex-variant bg-molex-surface p-4">
                  <span className="text-[11px] font-semibold uppercase text-molex-muted">
                    Sistema Operativo
                  </span>
                  <p className="mt-1 truncate font-mono text-sm font-semibold text-molex-light">
                    {metrics.os_info}
                  </p>
                </div>
                <div className="rounded-xl border border-molex-variant bg-molex-surface p-4">
                  <span className="text-[11px] font-semibold uppercase text-molex-muted">
                    Compositor
                  </span>
                  <p className="mt-1 flex items-center gap-1.5 text-sm font-semibold text-emerald-300">
                    <Monitor className="h-4 w-4" />
                    {metrics.compositor}
                  </p>
                </div>
                <div className="rounded-xl border border-molex-variant bg-molex-surface p-4">
                  <span className="text-[11px] font-semibold uppercase text-molex-muted">
                    Uptime
                  </span>
                  <p className="mt-1 truncate text-sm font-semibold text-molex-text">
                    {metrics.uptime}
                  </p>
                </div>
              </div>

              {/* CPU & RAM */}
              <div className="grid gap-4 md:grid-cols-2">
                <div className="rounded-xl border border-molex-variant bg-molex-surface p-5">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2.5">
                      <Cpu className="h-5 w-5 text-molex-accent" />
                      <h3 className="text-sm font-bold text-molex-text">Carga de CPU</h3>
                    </div>
                    <span className="font-mono text-lg font-bold text-molex-light">
                      {metrics.cpu_percent.toFixed(1)}%
                    </span>
                  </div>
                  <p className="mt-1 truncate text-xs text-molex-muted">{metrics.cpu_model}</p>

                  <div className="mt-3 h-2.5 w-full overflow-hidden rounded-full bg-molex-bg">
                    <div
                      className="h-full rounded-full bg-molex-accent transition-all duration-300"
                      style={{ width: `${Math.min(100, metrics.cpu_percent)}%` }}
                    />
                  </div>

                  <div className="mt-4 rounded-lg bg-molex-bg/80 p-2">
                    <svg viewBox="0 0 220 48" className="h-12 w-full">
                      <polyline
                        fill="none"
                        stroke="#8D6E63"
                        strokeWidth="2"
                        points={sparklinePoints}
                      />
                    </svg>
                  </div>
                </div>

                <div className="rounded-xl border border-molex-variant bg-molex-surface p-5">
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2.5">
                      <HardDrive className="h-5 w-5 text-molex-accent" />
                      <h3 className="text-sm font-bold text-molex-text">Memoria RAM</h3>
                    </div>
                    <span className="font-mono text-lg font-bold text-molex-light">{ramPct}%</span>
                  </div>
                  <p className="mt-1 font-mono text-xs text-molex-muted">{metrics.ram_usage}</p>

                  <div className="mt-4 h-3 w-full overflow-hidden rounded-full bg-molex-bg">
                    <div
                      className="h-full rounded-full bg-emerald-500/80 transition-all duration-300"
                      style={{ width: `${ramPct}%` }}
                    />
                  </div>

                  <div className="mt-5 grid grid-cols-2 gap-3 text-xs">
                    <div className="rounded-lg bg-molex-bg p-3">
                      <span className="text-molex-muted">En uso</span>
                      <p className="mt-0.5 font-mono font-bold text-molex-text">
                        {metrics.ram_used_mb} MB
                      </p>
                    </div>
                    <div className="rounded-lg bg-molex-bg p-3">
                      <span className="text-molex-muted">Total</span>
                      <p className="mt-0.5 font-mono font-bold text-molex-text">
                        {metrics.ram_total_mb} MB
                      </p>
                    </div>
                  </div>
                </div>
              </div>

              {/* GPU & Network */}
              <div className="grid gap-4 md:grid-cols-2">
                <div className="rounded-xl border border-molex-variant bg-molex-surface p-5">
                  <div className="flex items-center justify-between">
                    <h3 className="text-sm font-bold text-molex-text">Acelerador Gráfico (GPU)</h3>
                    {metrics.gpu_temp_c !== null && (
                      <span className="inline-flex items-center gap-1 rounded-full bg-amber-500/15 px-2.5 py-0.5 font-mono text-xs font-semibold text-amber-300">
                        <Thermometer className="h-3.5 w-3.5" />
                        {metrics.gpu_temp_c.toFixed(0)}°C
                      </span>
                    )}
                  </div>
                  <p className="mt-2 font-mono text-sm font-semibold text-molex-light">
                    {metrics.gpu_info}
                  </p>

                  <div className="mt-4 grid grid-cols-2 gap-3 text-xs">
                    <div className="rounded-lg bg-molex-bg p-3">
                      <span className="text-molex-muted">Utilización NVENC/3D</span>
                      <p className="mt-0.5 font-mono font-bold text-molex-text">
                        {metrics.gpu_util_percent !== null
                          ? `${metrics.gpu_util_percent.toFixed(0)}%`
                          : "N/D"}
                      </p>
                    </div>
                    <div className="rounded-lg bg-molex-bg p-3">
                      <span className="text-molex-muted">VRAM</span>
                      <p className="mt-0.5 font-mono font-bold text-molex-text">
                        {metrics.gpu_vram_usage ?? "N/D"}
                      </p>
                    </div>
                  </div>
                </div>

                <div className="rounded-xl border border-molex-variant bg-molex-surface p-5">
                  <div className="flex items-center gap-2.5">
                    <Network className="h-5 w-5 text-molex-accent" />
                    <h3 className="text-sm font-bold text-molex-text">Interfaces de Red y VPN</h3>
                  </div>
                  <div className="mt-3 space-y-2">
                    {metrics.network_status.split("·").map((iface, idx) => (
                      <div
                        key={idx}
                        className="rounded-lg bg-molex-bg px-3 py-2 font-mono text-xs text-molex-light"
                      >
                        {iface.trim()}
                      </div>
                    ))}
                  </div>
                </div>
              </div>
            </>
          )
        )}
      </div>
    </div>
  );
};
