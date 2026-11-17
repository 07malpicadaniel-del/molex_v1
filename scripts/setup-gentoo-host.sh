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

CLAMP_MSS="${MOLEX_CLAMP_MSS:-0}"
for arg in "$@"; do
    case "$arg" in
        --clamp-mss) CLAMP_MSS=1 ;;
    esac
done

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
if command -v usermod >/dev/null 2>&1; then
    run_as_root usermod -aG input,video,render "$TARGET_USER" || true
else
    for grp in input video render; do
        getent group "$grp" >/dev/null 2>&1 && run_as_root gpasswd -a "$TARGET_USER" "$grp" || true
    done
fi

# 2b. Soporte VA-API para iGPU Intel en laptops híbridas (libva-intel-media-driver / iHD)
HAS_INTEL_GPU=0
if grep -qi '0x8086' /sys/class/drm/*/device/vendor 2>/dev/null; then
    HAS_INTEL_GPU=1
elif command -v lspci >/dev/null 2>&1 && lspci | grep -qiE 'VGA.*Intel|Display.*Intel'; then
    HAS_INTEL_GPU=1
fi

if [[ "$HAS_INTEL_GPU" -eq 1 ]]; then
    echo "==> GPU Intel detectada: verificando driver VA-API iHD (media-libs/libva-intel-media-driver)..."
    if [[ ! -f /usr/lib64/dri/iHD_drv_video.so && ! -f /usr/lib/dri/iHD_drv_video.so ]]; then
        if command -v emerge >/dev/null 2>&1; then
            echo "==> Instalando media-libs/libva-intel-media-driver..."
            run_as_root emerge --noreplace media-libs/libva-intel-media-driver || {
                echo "⚠️  No se pudo instalar automáticamente; ejecuta manualmente: sudo emerge -av media-libs/libva-intel-media-driver"
            }
        else
            echo "⚠️  Instala manualmente: sudo emerge -av media-libs/libva-intel-media-driver"
        fi
    else
        echo "==> Driver iHD VA-API ya instalado."
    fi
fi

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

# 4b. Regla opcional de TCP MSS clamping (--clamp-mss-to-pmtu) para tailscale0 (MTU 1280)
if [[ "$CLAMP_MSS" -eq 1 ]]; then
    if command -v iptables >/dev/null 2>&1; then
        TMP_MSS="$(mktemp)"
        cat >"$TMP_MSS" <<'EOFMSS'
#!/bin/sh
# /etc/local.d/90-molex-mss.start — Ajusta TCP MSS al PMTU en tailscale0 (MTU 1280)
iptables -t mangle -C POSTROUTING -o tailscale0 -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu 2>/dev/null || \
    iptables -t mangle -A POSTROUTING -o tailscale0 -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu
iptables -t mangle -C FORWARD -o tailscale0 -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu 2>/dev/null || \
    iptables -t mangle -A FORWARD -o tailscale0 -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu
EOFMSS
        run_as_root install -Dm755 "$TMP_MSS" /etc/local.d/90-molex-mss.start
        rm -f "$TMP_MSS"
        run_as_root /etc/local.d/90-molex-mss.start || true
        echo "==> TCP MSS clamping habilitado en /etc/local.d/90-molex-mss.start"
    else
        echo "⚠️  --clamp-mss omitido: 'iptables' no está instalado."
    fi
fi

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
  • Grupos de hardware  : input, video, render
  • Driver Intel VA-API : media-libs/libva-intel-media-driver (LIBVA_DRIVER_NAME=iHD)
  • Operador Tailscale  : $TARGET_USER
  • Niri hook           : spawn-at-startup "molex-host-up"
  • Hyprland hook       : exec-once = molex-host-up
  • Clamping MTU/MSS    : opcional con '--clamp-mss' (o manualmente:
                          sudo iptables -t mangle -A POSTROUTING -o tailscale0 -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu)
  • Nota                : Verifica tener emergidos net-vpn/tailscale, net-misc/openssh y sys-apps/ethtool.
                          Cierra e inicia sesión una vez para aplicar los grupos input, video y render.
EOF
