#!/usr/bin/env bash
set -euo pipefail

# Lanzador supervisor de molex_daemon para sesiones Wayland en Gentoo (Niri / Hyprland)
DAEMON_BIN="${HOME}/.local/bin/molex_daemon"
LOG_DIR="${XDG_STATE_HOME:-$HOME/.local/state}/molex"
mkdir -p "$LOG_DIR"

if [[ ! -x "$DAEMON_BIN" ]]; then
    echo "No se encontró el binario ejecutable: $DAEMON_BIN" >&2
    exit 1
fi

# Evitar instancias duplicadas si el compositor recarga su configuración
if pgrep -u "$USER" -x molex_daemon >/dev/null 2>&1; then
    exit 0
fi

# Esperar brevemente a que el socket Wayland y compositor queden listos
for _ in $(seq 1 15); do
    if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
        break
    fi
    sleep 0.5
done

# Supervisor simple para mantener molex_daemon activo en la sesión Wayland
while true; do
    "$DAEMON_BIN" >>"$LOG_DIR/daemon.log" 2>&1 || true
    sleep 2
done
