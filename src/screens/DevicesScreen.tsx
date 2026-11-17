import React, { useState } from "react";
import {
  Check,
  CheckCircle2,
  Copy,
  Edit3,
  ExternalLink,
  Laptop,
  Loader2,
  Network,
  Play,
  Plus,
  Power,
  RefreshCw,
  Shield,
  Trash2,
  Wifi,
  X,
} from "lucide-react";
import type {
  DeviceProfile,
  TailscalePeer,
  TailscaleStatus,
  WakeProgress,
} from "../types/molex";

interface DevicesScreenProps {
  devices: DeviceProfile[];
  tailscaleStatus: TailscaleStatus | null;
  tailscaleLoading: boolean;
  tailscaleError: string | null;
  wakingDevices: Record<string, WakeProgress>;
  connectingDeviceId: string | null;
  activeDeviceId: string | null;
  onRefreshTailscale: () => void;
  onToggleTailscale: (enable: boolean) => void;
  onSaveDevice: (device: DeviceProfile) => Promise<void>;
  onDeleteDevice: (deviceId: string) => Promise<void>;
  onConnectDevice: (device: DeviceProfile) => Promise<void>;
  onWakeDevice: (device: DeviceProfile) => Promise<void>;
}

const EMPTY_FORM: DeviceProfile = {
  id: "",
  name: "",
  host: "",
  tailscale_ip: null,
  mac_address: null,
  broadcast_ip: "255.255.255.255",
  ssh_port: 22,
  video_port: 9091,
  input_port: 9090,
  username: "daniel",
  password: null,
};

export const DevicesScreen: React.FC<DevicesScreenProps> = ({
  devices,
  tailscaleStatus,
  tailscaleLoading,
  tailscaleError,
  wakingDevices,
  connectingDeviceId,
  activeDeviceId,
  onRefreshTailscale,
  onToggleTailscale,
  onSaveDevice,
  onDeleteDevice,
  onConnectDevice,
  onWakeDevice,
}) => {
  const [modalOpen, setModalOpen] = useState(false);
  const [form, setForm] = useState<DeviceProfile>(EMPTY_FORM);
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<DeviceProfile | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [copiedPermissionCmd, setCopiedPermissionCmd] = useState(false);

  const TAILSCALE_OPERATOR_CMD = "sudo tailscale set --operator=$USER";

  const copyTailscalePermissionCommand = async () => {
    try {
      await navigator.clipboard.writeText(TAILSCALE_OPERATOR_CMD);
    } catch {
      const textarea = document.createElement("textarea");
      textarea.value = TAILSCALE_OPERATOR_CMD;
      textarea.style.position = "fixed";
      textarea.style.opacity = "0";
      document.body.appendChild(textarea);
      textarea.focus();
      textarea.select();
      document.execCommand("copy");
      document.body.removeChild(textarea);
    }
    setCopiedPermissionCmd(true);
    window.setTimeout(() => setCopiedPermissionCmd(false), 2000);
  };

  const openNewModal = () => {
    setForm({ ...EMPTY_FORM });
    setModalOpen(true);
  };

  const openEditModal = (device: DeviceProfile) => {
    setForm({ ...device });
    setModalOpen(true);
  };

  const openFromTailscalePeer = (peer: TailscalePeer) => {
    const ip = peer.primary_ip ?? "";
    setForm({
      ...EMPTY_FORM,
      name: peer.hostname || peer.dns_name || ip,
      host: ip,
      tailscale_ip: ip || null,
    });
    setModalOpen(true);
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setActionError(null);
    try {
      await onSaveDevice(form);
      setModalOpen(false);
    } catch (err) {
      setActionError(String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleWake = async (device: DeviceProfile) => {
    setActionError(null);
    try {
      await onWakeDevice(device);
    } catch (err) {
      setActionError(String(err));
    }
  };

  const handleConnect = async (device: DeviceProfile) => {
    setActionError(null);
    try {
      await onConnectDevice(device);
    } catch (err) {
      setActionError(String(err));
    }
  };

  return (
    <div className="flex h-full flex-col overflow-y-auto bg-molex-bg p-6">
      <div className="mx-auto w-full max-w-6xl space-y-6">
        {/* Top bar */}
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-molex-text">
              Dispositivos y Red Mesh
            </h1>
            <p className="mt-1 text-sm text-molex-muted">
              Gestiona tus equipos Wayland, despiértalos por Wake-on-LAN y conéctate vía LAN o
              Tailscale.
            </p>
          </div>
          <button
            type="button"
            onClick={openNewModal}
            className="inline-flex items-center gap-2 rounded-lg bg-molex-accent px-4 py-2.5 text-sm font-semibold text-molex-bg shadow transition hover:bg-molex-light"
          >
            <Plus className="h-4 w-4" />
            Agregar Equipo
          </button>
        </div>

        {actionError && (
          <div className="flex items-center justify-between rounded-lg border border-red-500/40 bg-red-950/30 px-4 py-3 text-sm text-red-200">
            <span>{actionError}</span>
            <button
              type="button"
              onClick={() => setActionError(null)}
              className="rounded p-1 hover:bg-red-500/20"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        )}

        {/* Tailscale VPN Mesh Card */}
        <section className="rounded-xl border border-molex-variant bg-molex-surface p-5">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <div className="flex items-center gap-3.5">
              <div
                className={`flex h-11 w-11 items-center justify-center rounded-xl ${
                  tailscaleStatus?.is_running
                    ? "bg-emerald-500/15 text-emerald-400"
                    : "bg-molex-variant text-molex-muted"
                }`}
              >
                <Shield className="h-6 w-6" />
              </div>
              <div>
                <div className="flex items-center gap-2.5">
                  <h2 className="text-base font-semibold text-molex-text">VPN Mesh Tailscale</h2>
                  <span
                    className={`rounded-full px-2.5 py-0.5 text-xs font-semibold ${
                      tailscaleStatus?.is_running
                        ? "bg-emerald-500/15 text-emerald-300"
                        : tailscaleStatus?.backend_state === "NeedsLogin"
                          ? "bg-amber-500/15 text-amber-300"
                          : "bg-molex-variant text-molex-muted"
                    }`}
                  >
                    {tailscaleStatus?.backend_state ?? "Consultando..."}
                  </span>
                </div>
                <p className="mt-0.5 text-xs text-molex-muted">
                  {tailscaleStatus?.self_hostname
                    ? `Nodo local: ${tailscaleStatus.self_hostname}${
                        tailscaleStatus.self_ips.length > 0
                          ? ` (${tailscaleStatus.self_ips.join(", ")})`
                          : ""
                      }`
                    : "Detectando nodo local de Tailscale"}
                </p>
              </div>
            </div>

            <div className="flex items-center gap-2.5">
              <button
                type="button"
                onClick={onRefreshTailscale}
                disabled={tailscaleLoading}
                className="inline-flex items-center gap-1.5 rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 text-xs font-medium text-molex-text hover:bg-molex-variant disabled:opacity-50"
              >
                <RefreshCw className={`h-3.5 w-3.5 ${tailscaleLoading ? "animate-spin" : ""}`} />
                Actualizar
              </button>
              <button
                type="button"
                disabled={tailscaleLoading}
                onClick={() => onToggleTailscale(!(tailscaleStatus?.is_running ?? false))}
                className={`inline-flex items-center gap-1.5 rounded-lg px-3.5 py-2 text-xs font-semibold transition disabled:opacity-50 ${
                  tailscaleStatus?.is_running
                    ? "bg-red-500/15 text-red-300 hover:bg-red-500/25"
                    : "bg-emerald-500/20 text-emerald-300 hover:bg-emerald-500/30"
                }`}
              >
                <Power className="h-3.5 w-3.5" />
                {tailscaleStatus?.is_running ? "Desactivar VPN" : "Activar VPN"}
              </button>
            </div>
          </div>

          {tailscaleError && (
            <div className="mt-3 flex flex-wrap items-center justify-between gap-3 rounded-lg border border-red-500/30 bg-red-950/30 px-3.5 py-2.5 text-xs text-red-200">
              <div className="min-w-0 flex-1 space-y-1">
                <p>{tailscaleError}</p>
                <p className="font-mono text-[11px] text-red-300/90">
                  {TAILSCALE_OPERATOR_CMD}
                </p>
              </div>
              <button
                type="button"
                onClick={copyTailscalePermissionCommand}
                className="inline-flex shrink-0 items-center gap-1.5 rounded-md border border-red-400/30 bg-red-900/40 px-3 py-1.5 text-xs font-semibold text-red-100 transition hover:bg-red-800/60"
              >
                {copiedPermissionCmd ? (
                  <>
                    <Check className="h-3.5 w-3.5 text-emerald-300" />
                    ¡Copiado!
                  </>
                ) : (
                  <>
                    <Copy className="h-3.5 w-3.5" />
                    Copiar comando de permisos
                  </>
                )}
              </button>
            </div>
          )}

          {tailscaleStatus?.auth_url && !tailscaleStatus.is_running && (
            <div className="mt-3 flex flex-wrap items-center justify-between gap-2 rounded-lg border border-amber-500/30 bg-amber-950/20 px-3.5 py-2.5 text-xs text-amber-200">
              <span>
                Este nodo requiere autenticación en Tailscale antes de ver tus peers remotos:
              </span>
              <a
                href={tailscaleStatus.auth_url}
                target="_blank"
                rel="noreferrer"
                className="inline-flex items-center gap-1 font-mono font-semibold text-amber-300 underline"
              >
                {tailscaleStatus.auth_url}
                <ExternalLink className="h-3.5 w-3.5" />
              </a>
            </div>
          )}

          {/* Peers list */}
          {tailscaleStatus && tailscaleStatus.peers.length > 0 && (
            <div className="mt-4 border-t border-molex-variant pt-4">
              <h3 className="mb-2.5 text-xs font-semibold uppercase tracking-wider text-molex-muted">
                Nodos detectados en Tailnet ({tailscaleStatus.peers.length})
              </h3>
              <div className="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
                {tailscaleStatus.peers.map((peer) => (
                  <div
                    key={peer.id}
                    className="flex items-center justify-between rounded-lg border border-molex-variant bg-molex-bg/70 px-3.5 py-2.5"
                  >
                    <div className="min-w-0 pr-2">
                      <div className="flex items-center gap-2">
                        <span
                          className={`h-2 w-2 shrink-0 rounded-full ${
                            peer.online ? "bg-emerald-400" : "bg-zinc-500"
                          }`}
                        />
                        <span className="truncate text-sm font-medium text-molex-text">
                          {peer.hostname}
                        </span>
                        <span className="rounded bg-molex-variant px-1.5 py-0.5 text-[10px] text-molex-muted">
                          {peer.os || "linux"}
                        </span>
                      </div>
                      <p className="mt-0.5 truncate font-mono text-xs text-molex-muted">
                        {peer.primary_ip ?? "Sin IP"}
                        {peer.cur_addr ? " · Directo" : peer.relay ? ` · Relay ${peer.relay}` : ""}
                      </p>
                    </div>
                    <button
                      type="button"
                      onClick={() => openFromTailscalePeer(peer)}
                      className="shrink-0 rounded-md bg-molex-variant px-2.5 py-1.5 text-xs font-medium text-molex-light hover:bg-molex-accent hover:text-molex-bg"
                    >
                      + Guardar
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}
        </section>

        {/* Saved Devices Section */}
        <section className="space-y-3">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-semibold uppercase tracking-wider text-molex-muted">
              Equipos Guardados ({devices.length})
            </h2>
          </div>

          {devices.length === 0 ? (
            <div className="flex flex-col items-center justify-center rounded-xl border border-dashed border-molex-variant bg-molex-surface/50 px-6 py-14 text-center">
              <Laptop className="h-10 w-10 text-molex-accent" />
              <h3 className="mt-3 text-base font-semibold text-molex-text">
                No hay equipos guardados todavía
              </h3>
              <p className="mt-1 max-w-md text-sm text-molex-muted">
                Agrega tu workstation Arch Linux o laptop Gentoo con su IP LAN / Tailscale y MAC
                para despertarla con Wake-on-LAN y controlarla a 60 FPS.
              </p>
              <button
                type="button"
                onClick={openNewModal}
                className="mt-5 inline-flex items-center gap-2 rounded-lg bg-molex-accent px-4 py-2 text-sm font-semibold text-molex-bg hover:bg-molex-light"
              >
                <Plus className="h-4 w-4" />
                Agregar primer equipo
              </button>
            </div>
          ) : (
            <div className="grid gap-4 md:grid-cols-2">
              {devices.map((device) => {
                const wakeState = wakingDevices[device.id];
                const isWaking = Boolean(wakeState && wakeState.stage !== "ready" && wakeState.stage !== "timeout");
                const isConnecting = connectingDeviceId === device.id;
                const isConnected = activeDeviceId === device.id;

                return (
                  <div
                    key={device.id}
                    className={`flex flex-col justify-between rounded-xl border p-5 transition ${
                      isConnected
                        ? "border-emerald-500/50 bg-molex-surface shadow-lg shadow-emerald-950/20"
                        : "border-molex-variant bg-molex-surface hover:border-molex-accent/50"
                    }`}
                  >
                    <div>
                      <div className="flex items-start justify-between gap-3">
                        <div className="flex items-center gap-3 min-w-0">
                          <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-molex-variant text-molex-light">
                            <Laptop className="h-5 w-5 text-molex-accent" />
                          </div>
                          <div className="min-w-0">
                            <div className="flex items-center gap-2">
                              <h3 className="truncate text-base font-bold text-molex-text">
                                {device.name}
                              </h3>
                              {isConnected && (
                                <span className="inline-flex items-center gap-1 rounded-full bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold text-emerald-300">
                                  <CheckCircle2 className="h-3 w-3" /> Activo
                                </span>
                              )}
                            </div>
                            <p className="truncate font-mono text-xs text-molex-muted">
                              {device.username}@{device.host}
                            </p>
                          </div>
                        </div>

                        <div className="flex items-center gap-1">
                          <button
                            type="button"
                            onClick={() => openEditModal(device)}
                            title="Editar dispositivo"
                            className="rounded-lg p-1.5 text-molex-muted hover:bg-molex-variant hover:text-molex-text"
                          >
                            <Edit3 className="h-4 w-4" />
                          </button>
                          <button
                            type="button"
                            onClick={() => setDeleteTarget(device)}
                            title="Eliminar dispositivo"
                            className="rounded-lg p-1.5 text-molex-muted hover:bg-red-500/20 hover:text-red-300"
                          >
                            <Trash2 className="h-4 w-4" />
                          </button>
                        </div>
                      </div>

                      {/* Metadata tags */}
                      <div className="mt-4 flex flex-wrap gap-2 text-xs">
                        {device.tailscale_ip && (
                          <span className="inline-flex items-center gap-1 rounded-md bg-molex-bg px-2.5 py-1 font-mono text-molex-light">
                            <Shield className="h-3 w-3 text-molex-accent" />
                            TS: {device.tailscale_ip}
                          </span>
                        )}
                        {device.mac_address && (
                          <span className="inline-flex items-center gap-1 rounded-md bg-molex-bg px-2.5 py-1 font-mono text-molex-muted">
                            <Wifi className="h-3 w-3 text-molex-accent" />
                            MAC: {device.mac_address}
                          </span>
                        )}
                        <span className="inline-flex items-center gap-1 rounded-md bg-molex-bg px-2.5 py-1 font-mono text-molex-muted">
                          <Network className="h-3 w-3 text-molex-accent" />
                          SSH:{device.ssh_port} · Vid:{device.video_port} · In:{device.input_port}
                        </span>
                      </div>

                      {/* Wake-on-LAN live progress banner */}
                      {wakeState && (
                        <div className="mt-3 rounded-lg border border-molex-accent/40 bg-molex-bg/80 p-3">
                          <div className="flex items-center gap-2 text-xs font-medium text-molex-light">
                            {isWaking ? (
                              <Loader2 className="h-3.5 w-3.5 animate-spin text-molex-accent" />
                            ) : (
                              <CheckCircle2 className="h-3.5 w-3.5 text-emerald-400" />
                            )}
                            <span>{wakeState.message}</span>
                          </div>
                          <div className="mt-2 flex flex-wrap gap-2 text-[10px] font-mono">
                            <span
                              className={`rounded px-1.5 py-0.5 ${
                                wakeState.tailscale_online
                                  ? "bg-emerald-500/20 text-emerald-300"
                                  : "bg-molex-variant text-molex-muted"
                              }`}
                            >
                              VPN: {wakeState.tailscale_online ? "ONLINE" : "ESPERANDO"}
                            </span>
                            <span
                              className={`rounded px-1.5 py-0.5 ${
                                wakeState.ssh_ready
                                  ? "bg-emerald-500/20 text-emerald-300"
                                  : "bg-molex-variant text-molex-muted"
                              }`}
                            >
                              SSH: {wakeState.ssh_ready ? "LISTO" : "..."}
                            </span>
                            <span
                              className={`rounded px-1.5 py-0.5 ${
                                wakeState.video_ready
                                  ? "bg-emerald-500/20 text-emerald-300"
                                  : "bg-molex-variant text-molex-muted"
                              }`}
                            >
                              VIDEO: {wakeState.video_ready ? "LISTO" : "..."}
                            </span>
                          </div>
                        </div>
                      )}
                    </div>

                    {/* Card actions */}
                    <div className="mt-5 flex items-center gap-2.5 border-t border-molex-variant pt-4">
                      <button
                        type="button"
                        disabled={!device.mac_address || isWaking}
                        onClick={() => handleWake(device)}
                        title={
                          device.mac_address
                            ? "Enviar Magic Packet Wake-on-LAN y esperar VPN"
                            : "Configura una MAC Address para usar Wake-on-LAN"
                        }
                        className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 text-xs font-semibold text-molex-light transition hover:border-molex-accent hover:bg-molex-variant disabled:cursor-not-allowed disabled:opacity-40"
                      >
                        {isWaking ? (
                          <>
                            <Loader2 className="h-3.5 w-3.5 animate-spin text-molex-accent" />
                            Despertando...
                          </>
                        ) : (
                          <>
                            <Power className="h-3.5 w-3.5 text-molex-accent" />
                            Encender (WoL)
                          </>
                        )}
                      </button>

                      <button
                        type="button"
                        disabled={isConnecting}
                        onClick={() => handleConnect(device)}
                        className="inline-flex flex-1 items-center justify-center gap-1.5 rounded-lg bg-molex-accent px-3 py-2 text-xs font-semibold text-molex-bg transition hover:bg-molex-light disabled:opacity-50"
                      >
                        {isConnecting ? (
                          <>
                            <Loader2 className="h-3.5 w-3.5 animate-spin" />
                            Conectando...
                          </>
                        ) : (
                          <>
                            <Play className="h-3.5 w-3.5" />
                            Conectar
                          </>
                        )}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </section>
      </div>

      {/* Add / Edit Device Modal */}
      {modalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4 backdrop-blur-sm">
          <div className="w-full max-w-xl rounded-2xl border border-molex-variant bg-molex-surface p-6 shadow-2xl">
            <div className="flex items-center justify-between border-b border-molex-variant pb-3">
              <h2 className="text-lg font-bold text-molex-text">
                {form.id ? "Editar Dispositivo" : "Nuevo Equipo Remoto Wayland"}
              </h2>
              <button
                type="button"
                onClick={() => setModalOpen(false)}
                className="rounded-lg p-1 text-molex-muted hover:bg-molex-variant hover:text-molex-text"
              >
                <X className="h-5 w-5" />
              </button>
            </div>

            <form onSubmit={handleSave} className="mt-4 space-y-4">
              <div className="grid gap-4 sm:grid-cols-2">
                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">Nombre del equipo</span>
                  <input
                    type="text"
                    required
                    placeholder="Arch Workstation (RTX 5060 Ti)"
                    value={form.name}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 text-sm text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>

                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">
                    Host primario (IP LAN o Tailscale)
                  </span>
                  <input
                    type="text"
                    required
                    placeholder="192.168.1.100 o 100.x.y.z"
                    value={form.host}
                    onChange={(e) => setForm({ ...form, host: e.target.value })}
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 font-mono text-sm text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>

                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">
                    IP Tailscale (opcional)
                  </span>
                  <input
                    type="text"
                    placeholder="100.64.x.y"
                    value={form.tailscale_ip ?? ""}
                    onChange={(e) =>
                      setForm({ ...form, tailscale_ip: e.target.value || null })
                    }
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 font-mono text-sm text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>

                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">
                    MAC Address (Wake-on-LAN)
                  </span>
                  <input
                    type="text"
                    placeholder="AA:BB:CC:DD:EE:FF"
                    value={form.mac_address ?? ""}
                    onChange={(e) =>
                      setForm({ ...form, mac_address: e.target.value || null })
                    }
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 font-mono text-sm text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>

                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">Usuario SSH</span>
                  <input
                    type="text"
                    required
                    placeholder="daniel"
                    value={form.username}
                    onChange={(e) => setForm({ ...form, username: e.target.value })}
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 font-mono text-sm text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>

                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">
                    Contraseña SSH (opcional si usas llave)
                  </span>
                  <input
                    type="password"
                    placeholder="••••••••"
                    value={form.password ?? ""}
                    onChange={(e) => setForm({ ...form, password: e.target.value || null })}
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-3 py-2 text-sm text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>
              </div>

              <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">Broadcast WoL</span>
                  <input
                    type="text"
                    value={form.broadcast_ip ?? "255.255.255.255"}
                    onChange={(e) =>
                      setForm({ ...form, broadcast_ip: e.target.value || "255.255.255.255" })
                    }
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-2.5 py-2 font-mono text-xs text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>
                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">Puerto SSH</span>
                  <input
                    type="number"
                    min={1}
                    max={65535}
                    value={form.ssh_port}
                    onChange={(e) =>
                      setForm({ ...form, ssh_port: Number(e.target.value) || 22 })
                    }
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-2.5 py-2 font-mono text-xs text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>
                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">Puerto Video</span>
                  <input
                    type="number"
                    min={1}
                    max={65535}
                    value={form.video_port}
                    onChange={(e) =>
                      setForm({ ...form, video_port: Number(e.target.value) || 9091 })
                    }
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-2.5 py-2 font-mono text-xs text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>
                <label className="block space-y-1">
                  <span className="text-xs font-medium text-molex-muted">Puerto Input</span>
                  <input
                    type="number"
                    min={1}
                    max={65535}
                    value={form.input_port}
                    onChange={(e) =>
                      setForm({ ...form, input_port: Number(e.target.value) || 9090 })
                    }
                    className="w-full rounded-lg border border-molex-variant bg-molex-bg px-2.5 py-2 font-mono text-xs text-molex-text focus:border-molex-accent focus:outline-none"
                  />
                </label>
              </div>

              <div className="flex justify-end gap-2.5 border-t border-molex-variant pt-4">
                <button
                  type="button"
                  onClick={() => setModalOpen(false)}
                  className="rounded-lg px-4 py-2 text-sm font-medium text-molex-muted hover:bg-molex-variant hover:text-molex-text"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={saving}
                  className="rounded-lg bg-molex-accent px-5 py-2 text-sm font-semibold text-molex-bg hover:bg-molex-light disabled:opacity-50"
                >
                  {saving ? "Guardando..." : "Guardar Dispositivo"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Delete confirmation */}
      {deleteTarget && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 p-4">
          <div className="w-full max-w-md rounded-xl border border-molex-variant bg-molex-surface p-5 shadow-2xl">
            <h3 className="text-base font-bold text-molex-text">Eliminar Dispositivo</h3>
            <p className="mt-2 text-sm text-molex-muted">
              ¿Deseas eliminar{" "}
              <span className="font-semibold text-molex-text">{deleteTarget.name}</span> (
              {deleteTarget.username}@{deleteTarget.host})?
            </p>
            <div className="mt-5 flex justify-end gap-2.5">
              <button
                type="button"
                onClick={() => setDeleteTarget(null)}
                className="rounded-lg px-3.5 py-2 text-sm text-molex-muted hover:bg-molex-variant"
              >
                Cancelar
              </button>
              <button
                type="button"
                onClick={async () => {
                  const id = deleteTarget.id;
                  setDeleteTarget(null);
                  await onDeleteDevice(id);
                }}
                className="rounded-lg bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-500"
              >
                Eliminar
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
