#!/usr/bin/env bash
set -euo pipefail

# Setup de host Molex para Gentoo Linux (OpenRC + Niri/Hyprland + NVIDIA RTX + WoL)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GENTOO_DIR="$SCRIPT_DIR/gentoo"
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

echo "==> Configurando host Molex en Gentoo Linux (OpenRC) para usuario: $TARGET_USER"

# 1. Asegurar binario /usr/local/bin/molex_daemon y wrapper /usr/local/bin/molex-host-up
if [[ ! -x /usr/local/bin/molex_daemon || ! -x /usr/local/bin/molex-host-up ]]; then
    "$SCRIPT_DIR/install-host-daemon.sh"
fi

# 2. Módulo uinput persistente en Gentoo y regla udev (0660, grupo input)
run_as_root modprobe uinput || true
run_as_root install -Dm644 "$SCRIPT_DIR/molex-uinput.conf" /etc/modules-load.d/molex-uinput.conf
if [[ -f /etc/conf.d/modules ]] && ! grep -q '\buinput\b' /etc/conf.d/modules; then
    echo 'modules="$modules uinput"' | run_as_root tee -a /etc/conf.d/modules >/dev/null
fi
run_as_root install -Dm644 "$SCRIPT_DIR/99-molex-uinput.rules" /etc/udev/rules.d/99-molex-uinput.rules
run_as_root udevadm control --reload-rules
run_as_root udevadm trigger /dev/uinput || true
run_as_root gpasswd -a "$TARGET_USER" input

# 3. Habilitar tailscaled y sshd en el runlevel default de OpenRC (antes del login)
run_as_root rc-update add tailscaled default
run_as_root rc-update add sshd default
run_as_root rc-service tailscaled start || true
run_as_root rc-service sshd start || true

# Permite controlar tailscale up/down desde la UI Tauri sin root
run_as_root tailscale set --operator="$TARGET_USER" || true

# 4. Wake-on-LAN persistente vía /etc/local.d/wol.start (+x) en OpenRC
run_as_root install -Dm755 "$GENTOO_DIR/wol.start" /etc/local.d/wol.start
run_as_root rc-update add local default
run_as_root /etc/local.d/wol.start || true

# 5. Generar hooks de autostart para Niri y Hyprland usando molex-host-up
run_as_target_user mkdir -p "$TARGET_HOME/.config/niri" "$TARGET_HOME/.config/hypr"

NIRI_CFG="$TARGET_HOME/.config/niri/config.kdl"
NIRI_HOOK="$TARGET_HOME/.config/niri/molex-autostart.kdl"
cat <<'EOF' | run_as_target_user tee "$NIRI_HOOK" >/dev/null
// Molex PC-to-PC Host Daemon
spawn-at-startup "molex-host-up"
EOF

if [[ -f "$NIRI_CFG" ]]; then
    if ! grep -q 'molex-host-up' "$NIRI_CFG"; then
        cat <<'EOF' | run_as_target_user tee -a "$NIRI_CFG" >/dev/null

// Molex PC-to-PC Host Daemon
spawn-at-startup "molex-host-up"
EOF
        echo "==> Hook añadido a $NIRI_CFG"
    fi
else
    echo "==> Hook generado en $NIRI_HOOK (añade 'spawn-at-startup \"molex-host-up\"' en tu config.kdl)"
fi

HYPR_CFG="$TARGET_HOME/.config/hypr/hyprland.conf"
HYPR_HOOK="$TARGET_HOME/.config/hypr/molex-autostart.conf"
cat <<'EOF' | run_as_target_user tee "$HYPR_HOOK" >/dev/null
# Molex PC-to-PC Host Daemon
exec-once = molex-host-up
EOF

if [[ -f "$HYPR_CFG" ]]; then
    if ! grep -q 'molex-host-up' "$HYPR_CFG"; then
        cat <<'EOF' | run_as_target_user tee -a "$HYPR_CFG" >/dev/null

# Molex PC-to-PC Host Daemon
exec-once = molex-host-up
EOF
        echo "==> Hook añadido a $HYPR_CFG"
    fi
else
    echo "==> Hook generado en $HYPR_HOOK (añade 'exec-once = molex-host-up' en tu hyprland.conf)"
fi

cat <<EOF

✅ Configuración Gentoo Linux (OpenRC) completada para '$TARGET_USER'.
  • OpenRC default      : tailscaled, sshd, local (/etc/local.d/wol.start)
  • Operador Tailscale  : $TARGET_USER
  • Niri hook           : spawn-at-startup "molex-host-up"
  • Hyprland hook       : exec-once = molex-host-up
  • Nota                : Verifica tener emergidos net-vpn/tailscale, net-misc/openssh y sys-apps/ethtool.
EOF
