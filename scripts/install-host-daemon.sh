#!/usr/bin/env bash
set -euo pipefail

# Compila e instala molex_daemon de forma independiente (sin Node.js, npm ni frontend)
# Compatible con Arch Linux (systemd) y Gentoo Linux (OpenRC).

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET_USER="${SUDO_USER:-${USER:-$(id -un)}}"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"

if [[ -z "${TARGET_HOME:-}" ]]; then
    TARGET_HOME="$HOME"
fi

export PATH="$TARGET_HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
    echo "❌ No se encontró 'cargo'. Instala Rust/Cargo (rustup o paquete del sistema) antes de continuar." >&2
    exit 1
fi

run_as_root() {
    if [[ "$(id -u)" -eq 0 ]]; then
        "$@"
    else
        sudo "$@"
    fi
}

echo "==> Compilando molex_daemon en modo --release (sin dependencias de frontend)..."
if [[ "$(id -u)" -eq 0 && -n "${SUDO_USER:-}" ]]; then
    sudo -u "$SUDO_USER" env PATH="$PATH" \
        cargo build --release --manifest-path "$REPO_ROOT/molex_motor/daemon/Cargo.toml"
else
    cargo build --release --manifest-path "$REPO_ROOT/molex_motor/daemon/Cargo.toml"
fi

BIN_PATH="$REPO_ROOT/molex_motor/target/release/molex_daemon"
if [[ ! -x "$BIN_PATH" ]]; then
    BIN_PATH="$REPO_ROOT/molex_motor/daemon/target/release/molex_daemon"
fi

if [[ ! -x "$BIN_PATH" ]]; then
    echo "❌ No se encontró el binario compilado de molex_daemon." >&2
    exit 1
fi

echo "==> Instalando binario en /usr/local/bin/molex_daemon..."
run_as_root install -Dm755 "$BIN_PATH" /usr/local/bin/molex_daemon

echo "==> Configurando módulo uinput y regla udev (0660, grupo input)..."
run_as_root modprobe uinput || true
run_as_root install -Dm644 "$SCRIPT_DIR/molex-uinput.conf" /etc/modules-load.d/molex-uinput.conf
run_as_root install -Dm644 "$SCRIPT_DIR/99-molex-uinput.rules" /etc/udev/rules.d/99-molex-uinput.rules
if command -v udevadm >/dev/null 2>&1; then
    run_as_root udevadm control --reload-rules
    run_as_root udevadm trigger /dev/uinput || true
fi
if getent group input >/dev/null 2>&1; then
    if command -v usermod >/dev/null 2>&1; then
        run_as_root usermod -aG input "$TARGET_USER" || true
    elif command -v gpasswd >/dev/null 2>&1; then
        run_as_root gpasswd -a "$TARGET_USER" input || true
    fi
fi

echo "==> Generando wrapper /usr/local/bin/molex-host-up..."
TMP_WRAPPER="$(mktemp)"
cat >"$TMP_WRAPPER" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

# Asegurar herramientas de captura (~/.cargo/bin/wl-screenrec) y binarios Molex en PATH
export PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin:${PATH:-}"
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

# Autodescubrimiento de sockets Wayland (Niri / Hyprland) si no están exportados
if [[ -z "${WAYLAND_DISPLAY:-}" ]]; then
    for sock in "$XDG_RUNTIME_DIR"/wayland-[0-9]*; do
        if [[ -S "$sock" ]]; then
            export WAYLAND_DISPLAY
            WAYLAND_DISPLAY="$(basename "$sock")"
            break
        fi
    done
fi

if [[ -z "${NIRI_SOCKET:-}" ]]; then
    for sock in "$XDG_RUNTIME_DIR"/niri.*.sock; do
        if [[ -S "$sock" ]]; then
            export NIRI_SOCKET="$sock"
            break
        fi
    done
fi

if [[ -z "${HYPRLAND_INSTANCE_SIGNATURE:-}" && -d "$XDG_RUNTIME_DIR/hypr" ]]; then
    latest_hypr="$(ls -1t "$XDG_RUNTIME_DIR/hypr" 2>/dev/null | head -n 1 || true)"
    if [[ -n "$latest_hypr" ]]; then
        export HYPRLAND_INSTANCE_SIGNATURE="$latest_hypr"
    fi
fi

try_root_noninteractive() {
    if [[ "$(id -u)" -eq 0 ]]; then
        "$@"
    elif command -v sudo >/dev/null 2>&1; then
        sudo -n "$@" 2>/dev/null || true
    fi
}

# 1. Verificar permisos activos de /dev/uinput (0660, grupo input)
if [[ ! -e /dev/uinput ]]; then
    try_root_noninteractive modprobe uinput
    if command -v udevadm >/dev/null 2>&1; then
        try_root_noninteractive udevadm trigger /dev/uinput
    fi
fi

if [[ -e /dev/uinput ]]; then
    UINPUT_META="$(stat -c '%a:%G' /dev/uinput 2>/dev/null || true)"
    if [[ "$UINPUT_META" != "660:input" ]]; then
        try_root_noninteractive chgrp input /dev/uinput
        try_root_noninteractive chmod 0660 /dev/uinput
        UINPUT_META="$(stat -c '%a:%G' /dev/uinput 2>/dev/null || true)"
    fi
    if [[ ! -r /dev/uinput || ! -w /dev/uinput ]]; then
        echo "❌ /dev/uinput ($UINPUT_META) no tiene acceso lectura/escritura para $(id -un)." >&2
        echo "   Verifica que perteneces al grupo 'input' y que /etc/udev/rules.d/99-molex-uinput.rules está cargado." >&2
        exit 1
    fi
else
    echo "❌ /dev/uinput no existe. Carga el módulo del kernel con: sudo modprobe uinput" >&2
    exit 1
fi

# 2. Asegurar que tailscaled y sshd están corriendo (Arch systemd o Gentoo OpenRC)
ensure_service_running() {
    local svc="$1"
    if command -v systemctl >/dev/null 2>&1 && [[ -d /run/systemd/system ]]; then
        if ! systemctl is-active --quiet "$svc"; then
            try_root_noninteractive systemctl start "$svc"
        fi
    elif command -v rc-service >/dev/null 2>&1; then
        if ! rc-service "$svc" status >/dev/null 2>&1; then
            try_root_noninteractive rc-service "$svc" start
        fi
    fi
}

ensure_service_running tailscaled
ensure_service_running sshd

if ! command -v wl-screenrec >/dev/null 2>&1; then
    echo "⚠️  Advertencia: no se encontró 'wl-screenrec' en PATH ($PATH)." >&2
fi

# Evitar instancias duplicadas si el compositor recarga su configuración
if pgrep -u "$(id -u)" -x molex_daemon >/dev/null 2>&1; then
    echo "ℹ️  molex_daemon ya se encuentra en ejecución."
    exit 0
fi

# 3. Lanzar molex_daemon en 0.0.0.0:9090 (UDP input) y 0.0.0.0:9091 (TCP video H.264)
exec /usr/local/bin/molex_daemon --input-port 9090 --video-port 9091 "$@"
EOF

run_as_root install -Dm755 "$TMP_WRAPPER" /usr/local/bin/molex-host-up
rm -f "$TMP_WRAPPER"

cat <<'EOF'

✅ Instalación independiente completada:
  • Binario daemon : /usr/local/bin/molex_daemon
  • Wrapper inicio : /usr/local/bin/molex-host-up
  • Regla udev     : /etc/udev/rules.d/99-molex-uinput.rules (0660, grupo input)

Siguiente paso:
  • En Arch Linux  : ./scripts/setup-arch-host.sh
  • En Gentoo Linux: ./scripts/setup-gentoo-host.sh
EOF
