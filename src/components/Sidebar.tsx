import React from "react";
import {
  Activity,
  ChevronLeft,
  ChevronRight,
  Monitor,
  Power,
  Server,
  Settings,
  Shield,
  Terminal,
} from "lucide-react";
import type { DeviceProfile, NavTab, TailscaleStatus } from "../types/molex";

interface SidebarProps {
  activeTab: NavTab;
  onSelectTab: (tab: NavTab) => void;
  collapsed: boolean;
  onToggleCollapse: () => void;
  activeDevice: DeviceProfile | null;
  activeHost: string | null;
  tailscaleStatus: TailscaleStatus | null;
  onDisconnect: () => void;
}

const NAV_ITEMS: { id: NavTab; label: string; icon: React.ComponentType<{ className?: string }> }[] =
  [
    { id: "devices", label: "Dispositivos", icon: Server },
    { id: "remote", label: "Pantalla", icon: Monitor },
    { id: "ssh", label: "Terminal SSH", icon: Terminal },
    { id: "metrics", label: "Métricas", icon: Activity },
    { id: "settings", label: "Ajustes", icon: Settings },
  ];

export const Sidebar: React.FC<SidebarProps> = ({
  activeTab,
  onSelectTab,
  collapsed,
  onToggleCollapse,
  activeDevice,
  activeHost,
  tailscaleStatus,
  onDisconnect,
}) => {
  return (
    <aside
      className={`flex h-full flex-col border-r border-molex-variant bg-molex-surface transition-all duration-200 select-none ${
        collapsed ? "w-[72px]" : "w-64"
      }`}
    >
      {/* Header / Brand */}
      <div className="flex items-center justify-between border-b border-molex-variant px-3.5 py-4">
        <div className="flex items-center gap-3 overflow-hidden">
          <img
            src="/molex-logo.png"
            alt="Molex"
            className="h-9 w-9 shrink-0 rounded-lg border border-molex-variant object-cover"
          />
          {!collapsed && (
            <div className="min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-base font-extrabold tracking-wider text-molex-light">
                  MOLEX
                </span>
                <span className="rounded bg-molex-dark/60 px-1.5 py-0.5 text-[10px] font-semibold uppercase text-molex-light">
                  PC
                </span>
              </div>
              <p className="truncate text-[11px] text-molex-muted">Wayland Low-Latency</p>
            </div>
          )}
        </div>
        <button
          type="button"
          onClick={onToggleCollapse}
          title={collapsed ? "Expandir barra" : "Colapsar barra"}
          className="rounded-md p-1.5 text-molex-muted hover:bg-molex-variant hover:text-molex-text"
        >
          {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
        </button>
      </div>

      {/* Navigation */}
      <nav className="flex-1 space-y-1 p-2.5">
        {NAV_ITEMS.map((item) => {
          const Icon = item.icon;
          const isActive = activeTab === item.id;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelectTab(item.id)}
              title={collapsed ? item.label : undefined}
              className={`flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-sm font-medium transition-colors ${
                isActive
                  ? "bg-molex-accent/20 text-molex-light ring-1 ring-molex-accent/50"
                  : "text-molex-muted hover:bg-molex-variant/70 hover:text-molex-text"
              } ${collapsed ? "justify-center px-0" : ""}`}
            >
              <Icon
                className={`h-5 w-5 shrink-0 ${isActive ? "text-molex-accent" : "text-molex-muted"}`}
              />
              {!collapsed && <span className="truncate">{item.label}</span>}
              {!collapsed && item.id === "remote" && activeDevice && (
                <span className="ml-auto h-2 w-2 rounded-full bg-emerald-400" />
              )}
            </button>
          );
        })}
      </nav>

      {/* Active session + Tailscale footer */}
      <div className="space-y-2.5 border-t border-molex-variant p-3">
        {activeDevice && (
          <div
            className={`rounded-lg border border-emerald-500/30 bg-emerald-950/20 p-2.5 ${
              collapsed ? "flex flex-col items-center gap-2" : ""
            }`}
          >
            {!collapsed ? (
              <>
                <div className="flex items-center justify-between gap-2">
                  <span className="truncate text-xs font-semibold text-emerald-300">
                    {activeDevice.name}
                  </span>
                  <span className="h-2 w-2 shrink-0 rounded-full bg-emerald-400 animate-pulse" />
                </div>
                <p className="mt-0.5 truncate font-mono text-[11px] text-molex-muted">
                  {activeDevice.username}@{activeHost}
                </p>
                <button
                  type="button"
                  onClick={onDisconnect}
                  className="mt-2 flex w-full items-center justify-center gap-1.5 rounded bg-red-500/15 px-2 py-1 text-xs font-medium text-red-300 hover:bg-red-500/25"
                >
                  <Power className="h-3.5 w-3.5" />
                  Desconectar
                </button>
              </>
            ) : (
              <button
                type="button"
                onClick={onDisconnect}
                title={`Desconectar ${activeDevice.name}`}
                className="rounded p-1.5 text-red-300 hover:bg-red-500/20"
              >
                <Power className="h-4 w-4" />
              </button>
            )}
          </div>
        )}

        <div
          className={`flex items-center gap-2.5 rounded-lg bg-molex-bg/70 px-2.5 py-2 ${
            collapsed ? "justify-center px-0" : ""
          }`}
          title={
            tailscaleStatus
              ? `Tailscale: ${tailscaleStatus.backend_state}`
              : "Estado de Tailscale"
          }
        >
          <Shield
            className={`h-4 w-4 shrink-0 ${
              tailscaleStatus?.is_running ? "text-emerald-400" : "text-molex-muted"
            }`}
          />
          {!collapsed && (
            <div className="min-w-0 flex-1">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-molex-text">Tailscale VPN</span>
                <span
                  className={`text-[10px] font-semibold ${
                    tailscaleStatus?.is_running
                      ? "text-emerald-400"
                      : tailscaleStatus?.backend_state === "NeedsLogin"
                        ? "text-amber-400"
                        : "text-molex-muted"
                  }`}
                >
                  {tailscaleStatus?.backend_state ?? "..."}
                </span>
              </div>
              {tailscaleStatus?.self_ips?.[0] && (
                <p className="truncate font-mono text-[10px] text-molex-muted">
                  {tailscaleStatus.self_ips[0]}
                </p>
              )}
            </div>
          )}
        </div>
      </div>
    </aside>
  );
};
