#!/usr/bin/env bash
set -euo pipefail

# Setup de host Molex para Arch Linux (systemd + Niri/Hyprland + NVIDIA RTX + WoL)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ARCH_DIR="$SCRIPT_DIR/arch"
TARGET_USER="${SUDO_USER:-${USER:-$(id -un)}}"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"

if [[ -z "${TARGET_HOME:-}" ]]; then
    TARGET_HOME="$HOME"
fi

run_as_root() {
    if [[ "$(id -u)" -eq 0 ]]; then
        "$@"
    else
        sudo "$@"
    fi
}

run_as_target_user() {
    if [[ "$(id -u)" -eq 0 && -n "${SUDO_USER:-}" ]]; then
        sudo -u "$TARGET_USER" env HOME="$TARGET_HOME" "$@"
    else
        "$@"
    fi
}

echo "==> Configurando host Molex en Arch Linux para usuario: $TARGET_USER"

# 1. Asegurar binario /usr/local/bin/molex_daemon y wrapper /usr/local/bin/molex-host-up
if [[ ! -x /usr/local/bin/molex_daemon || ! -x /usr/local/bin/molex-host-up ]]; then
    "$SCRIPT_DIR/install-host-daemon.sh"
fi

# 2. Instalar ethtool si no está presente
if ! command -v ethtool >/dev/null 2>&1; then
    echo "==> Instalando ethtool..."
    run_as_root pacman -S --needed --noconfirm ethtool
fi

# 3. Módulo uinput y regla udev para input virtual (0660, grupo input)
run_as_root modprobe uinput || true
run_as_root install -Dm644 "$SCRIPT_DIR/molex-uinput.conf" /etc/modules-load.d/molex-uinput.conf
run_as_root install -Dm644 "$SCRIPT_DIR/99-molex-uinput.rules" /etc/udev/rules.d/99-molex-uinput.rules
run_as_root udevadm control --reload-rules
run_as_root udevadm trigger /dev/uinput || true
run_as_root usermod -aG input,video,render "$TARGET_USER"

# 4. Habilitar tailscaled y sshd en el arranque (antes del login)
run_as_root systemctl enable --now tailscaled.service
run_as_root systemctl enable --now sshd.service

# Permite controlar tailscale up/down desde la UI Tauri sin root
run_as_root tailscale set --operator="$TARGET_USER" || true

# 5. Wake-on-LAN persistente (prioriza enp8s0 de archleika o detecta interfaz cableada)
WOL_IFACE=""
if [[ -e /sys/class/net/enp8s0 ]]; then
    WOL_IFACE="enp8s0"
else
    for iface_path in /sys/class/net/*; do
        iface="$(basename "$iface_path")"
        case "$iface" in
            lo|tailscale*|docker*|virbr*|veth*|br*) continue ;;
        esac
        if [[ ! -d "$iface_path/wireless" ]]; then
            WOL_IFACE="$iface"
            break
        fi
    done
fi

if [[ -n "$WOL_IFACE" ]]; then
    echo "==> Habilitando Wake-on-LAN (g) en interfaz: $WOL_IFACE"
    run_as_root ethtool -s "$WOL_IFACE" wol g || true
    run_as_root install -Dm644 "$ARCH_DIR/wol-ethtool@.service" /etc/systemd/system/wol-ethtool@.service
    run_as_root systemctl daemon-reload
    run_as_root systemctl enable --now "wol-ethtool@${WOL_IFACE}.service"
else
    echo "⚠️  No se detectó interfaz Ethernet cableada para WoL. Habilita manualmente wol-ethtool@enp8s0.service."
fi

# 6. Instalar servicio systemd de usuario (con PATH para ~/.cargo/bin y soporte Niri/Hyprland)
run_as_target_user mkdir -p "$TARGET_HOME/.config/systemd/user"
run_as_target_user install -Dm644 "$ARCH_DIR/molex-daemon.service" \
    "$TARGET_HOME/.config/systemd/user/molex-daemon.service"

TARGET_UID="$(id -u "$TARGET_USER")"
USER_RUNTIME_DIR="/run/user/$TARGET_UID"
if [[ -d "$USER_RUNTIME_DIR" ]]; then
    run_as_target_user env XDG_RUNTIME_DIR="$USER_RUNTIME_DIR" \
        systemctl --user daemon-reload || true
    run_as_target_user env XDG_RUNTIME_DIR="$USER_RUNTIME_DIR" \
        systemctl --user enable molex-daemon.service || true
fi

# 7. Hooks idempotentes de autostart en Niri y Hyprland
NIRI_CFG="$TARGET_HOME/.config/niri/config.kdl"
if [[ -f "$NIRI_CFG" ]] && ! grep -qE 'molex-host-up|molex-daemon\.service' "$NIRI_CFG"; then
    cat <<'EOF' | run_as_target_user tee -a "$NIRI_CFG" >/dev/null

// Molex PC-to-PC Host Daemon
spawn-at-startup "systemctl" "--user" "import-environment" "WAYLAND_DISPLAY" "XDG_RUNTIME_DIR" "NIRI_SOCKET"
spawn-at-startup "molex-host-up"
EOF
    echo "==> Hook añadido a $NIRI_CFG"
fi

HYPR_CFG="$TARGET_HOME/.config/hypr/hyprland.conf"
if [[ -f "$HYPR_CFG" ]] && ! grep -qE 'molex-host-up|molex-daemon\.service' "$HYPR_CFG"; then
    cat <<'EOF' | run_as_target_user tee -a "$HYPR_CFG" >/dev/null

# Molex PC-to-PC Host Daemon
exec-once = systemctl --user import-environment WAYLAND_DISPLAY XDG_RUNTIME_DIR HYPRLAND_INSTANCE_SIGNATURE
exec-once = molex-host-up
EOF
    echo "==> Hook añadido a $HYPR_CFG"
fi

cat <<EOF

✅ Configuración Arch Linux completada para '$TARGET_USER'.
  • WoL habilitado en   : ${WOL_IFACE:-ninguna detectada}
  • Servicios activos   : tailscaled.service, sshd.service, molex-daemon.service
  • Operador Tailscale  : $TARGET_USER
  • Puertos requeridos  : 22/tcp (SSH), 9090/udp (uinput), 9091/tcp (Video H.264)
  • Nota                : Cierra e inicia sesión una vez si acabas de entrar al grupo 'input'.
EOF
