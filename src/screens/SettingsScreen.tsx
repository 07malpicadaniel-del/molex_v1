import React from "react";
import { Sliders } from "lucide-react";
import type { DesktopSettings } from "../types/molex";

interface SettingsScreenProps {
  settings: DesktopSettings;
  onUpdateSettings: (next: DesktopSettings) => Promise<void>;
}

export const SettingsScreen: React.FC<SettingsScreenProps> = ({
  settings,
  onUpdateSettings,
}) => {
  const update = (patch: Partial<DesktopSettings>) => {
    onUpdateSettings({ ...settings, ...patch });
  };

  return (
    <div className="flex h-full flex-col overflow-y-auto bg-molex-bg p-6">
      <div className="mx-auto w-full max-w-3xl space-y-6">
        <div className="flex items-center gap-3">
          <Sliders className="h-6 w-6 text-molex-accent" />
          <div>
            <h1 className="text-2xl font-bold text-molex-text">Ajustes de Escritorio</h1>
            <p className="text-sm text-molex-muted">
              Preferencias persistentes del decodificador H.264, entrada y Wake-on-LAN.
            </p>
          </div>
        </div>

        {/* Streaming & Decoder */}
        <section className="space-y-5 rounded-xl border border-molex-variant bg-molex-surface p-5">
          <h2 className="text-sm font-bold uppercase tracking-wider text-molex-accent">
            Streaming H.264 y Rendimiento
          </h2>

          <div>
            <div className="flex items-center justify-between text-sm">
              <span className="font-medium text-molex-text">Framerate Objetivo</span>
              <span className="font-mono font-bold text-molex-light">
                {settings.target_fps} FPS
              </span>
            </div>
            <input
              type="range"
              min={30}
              max={144}
              step={15}
              value={settings.target_fps}
              onChange={(e) => update({ target_fps: Number(e.target.value) })}
              className="mt-2 w-full accent-molex-accent"
            />
          </div>

          <div className="border-t border-molex-variant pt-4">
            <div className="flex items-center justify-between text-sm">
              <div>
                <span className="font-medium text-molex-text">
                  Hilos del decodificador FFmpeg (Slice Low-Delay)
                </span>
                <p className="text-xs text-molex-muted">
                  Aumenta los hilos para resoluciones ultrawide 2560×1080 o 4K a 60 FPS.
                </p>
              </div>
              <span className="font-mono font-bold text-molex-light">
                {settings.decoder_threads} threads
              </span>
            </div>
            <input
              type="range"
              min={1}
              max={8}
              step={1}
              value={settings.decoder_threads}
              onChange={(e) => update({ decoder_threads: Number(e.target.value) })}
              className="mt-2 w-full accent-molex-accent"
            />
          </div>
        </section>

        {/* Input & Connection */}
        <section className="space-y-4 rounded-xl border border-molex-variant bg-molex-surface p-5">
          <h2 className="text-sm font-bold uppercase tracking-wider text-molex-accent">
            Conexión, Wake-on-LAN y Teclado
          </h2>

          <label className="flex cursor-pointer items-center justify-between gap-4 py-1">
            <div>
              <span className="block text-sm font-medium text-molex-text">
                Auto-conectar al finalizar Wake-on-LAN
              </span>
              <span className="text-xs text-molex-muted">
                Abre la pantalla remota automáticamente cuando Tailscale y el puerto 9091
                responden.
              </span>
            </div>
            <input
              type="checkbox"
              checked={settings.auto_connect_after_wol}
              onChange={(e) => update({ auto_connect_after_wol: e.target.checked })}
              className="h-4 w-4 accent-molex-accent"
            />
          </label>

          <label className="flex cursor-pointer items-center justify-between gap-4 border-t border-molex-variant pt-4">
            <div>
              <span className="block text-sm font-medium text-molex-text">
                Ocultar cursor local sobre el stream
              </span>
              <span className="text-xs text-molex-muted">
                Evita doble cursor visual al mover el puntero en la sesión remota.
              </span>
            </div>
            <input
              type="checkbox"
              checked={settings.hide_local_cursor_in_stream}
              onChange={(e) => update({ hide_local_cursor_in_stream: e.target.checked })}
              className="h-4 w-4 accent-molex-accent"
            />
          </label>

          <label className="flex cursor-pointer items-center justify-between gap-4 border-t border-molex-variant pt-4">
            <div>
              <span className="block text-sm font-medium text-molex-text">
                Activar Super Lock por defecto
              </span>
              <span className="text-xs text-molex-muted">
                Útil para controlar workspaces de Hyprland/Niri remotos sin que tu compositor local
                intercepte Super.
              </span>
            </div>
            <input
              type="checkbox"
              checked={settings.sticky_super_default}
              onChange={(e) => update({ sticky_super_default: e.target.checked })}
              className="h-4 w-4 accent-molex-accent"
            />
          </label>
        </section>
      </div>
    </div>
  );
};
