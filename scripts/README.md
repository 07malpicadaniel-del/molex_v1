# Infraestructura de Arranque, Wake-on-LAN y Tailscale para Molex PC-to-PC

Esta carpeta contiene el instalador independiente del daemon anfitrión y los scripts de servicio para dejar listas tanto la **Workstation `archleika` (Arch Linux + Niri/Hyprland + RTX 5060 Ti)** como la **Laptop `leika-laptop` (Gentoo Linux + OpenRC + Niri/Hyprland + RTX 3050)** inmediatamente después de despertar por **Wake-on-LAN (WoL)**.

---

## 1. Instalación del Binario Anfitrión (Sin Node.js ni Frontend)

En cualquier máquina anfitriona (solo requiere Rust/Cargo):

```bash
chmod +x scripts/install-host-daemon.sh
./scripts/install-host-daemon.sh
```

Este script:
- Compila `molex_motor/daemon` en modo `--release` e instala `/usr/local/bin/molex_daemon`.
- Crea `/usr/local/bin/molex-host-up`, que:
  1. Verifica y asegura `/dev/uinput` (`0660`, grupo `input`).
  2. Comprueba que `tailscaled` y `sshd` estén activos (`systemd` o `OpenRC`).
  3. Añade `~/.cargo/bin` al `PATH` (para localizar `wl-screenrec`) y lanza `molex_daemon` en `0.0.0.0:9090` (UDP) y `0.0.0.0:9091` (TCP H.264).

---

## 2. Arch Linux (`archleika`: systemd + Niri 26.04 / Hyprland)

Con `ErP = Disabled` en BIOS:

```bash
chmod +x scripts/setup-arch-host.sh
./scripts/setup-arch-host.sh
```

Configura:
- Instala `ethtool` si falta y habilita `wol-ethtool@enp8s0.service` (`ethtool -s %i wol g`).
- Habilita `tailscaled.service` y `sshd.service` en el arranque y ejecuta `tailscale set --operator=$USER`.
- Instala `~/.config/systemd/user/molex-daemon.service` con `PATH=%h/.cargo/bin:/usr/local/bin:/usr/bin` y soporte para `NIRI_SOCKET` y `HYPRLAND_INSTANCE_SIGNATURE`.
- Añade hooks de autostart en `~/.config/niri/config.kdl` y `~/.config/hypr/hyprland.conf`.

---

## 3. Gentoo Linux (`leika-laptop`: OpenRC + Niri / Hyprland)

```bash
chmod +x scripts/setup-gentoo-host.sh
./scripts/setup-gentoo-host.sh
# Opcional: habilitar clamping TCP MSS (--clamp-mss-to-pmtu) para tailscale0 (MTU 1280)
./scripts/setup-gentoo-host.sh --clamp-mss
```

Configura:
- Añade al usuario a los grupos `input`, `video` y `render` para acceso a `/dev/uinput` y `/dev/dri/renderD*`.
- Si detecta iGPU Intel (`0x8086`), instala `media-libs/libva-intel-media-driver` (driver VA-API `iHD`).
- Añade `tailscaled`, `sshd` y `local` al runlevel `default` de OpenRC (`rc-update add ... default`) y aplica `tailscale set --operator=$USER`.
- Instala `/etc/local.d/wol.start` (`0755`) para habilitar `ethtool -s <interfaz_ethernet> wol g` en cada arranque.
- Con `--clamp-mss`, instala `/etc/local.d/90-molex-mss.start` para aplicar `iptables -t mangle -A POSTROUTING -o tailscale0 -p tcp --tcp-flags SYN,RST SYN -j TCPMSS --clamp-mss-to-pmtu` (evita bloqueos de paquetes grandes cuando Tailscale MTU 1280 convive con Wi-Fi/Docker).
- Genera los hooks de autostart para **Niri** (`spawn-at-startup "molex-host-up"`) y **Hyprland** (`exec-once = molex-host-up`).

---

## 4. Red y Firewall

Puertos utilizados por Molex en LAN y/o interfaz `tailscale0`:
- `22/tcp`: SSH (terminal persistente, métricas del sistema y descubrimiento de monitores)
- `9090/udp`: Input virtual `uinput` (ratón, rueda y teclado completo)
- `9091/tcp`: Stream continuo H.264 por hardware (`wl-screenrec`)
- `9/udp`: Wake-on-LAN Magic Packet

---

## 5. Despliegue Rápido en una Laptop Nueva (Cliente y Host)

### Como Cliente (sin compilar Node.js ni Rust)
1. **Instalar Tailscale y dependencias runtime de WebView/FFmpeg:**
   - **Arch Linux:** `sudo pacman -S tailscale webkit2gtk-4.1 ffmpeg && sudo systemctl enable --now tailscaled && sudo tailscale up --operator=$USER`
   - **Gentoo Linux:** `sudo emerge -av net-vpn/tailscale net-libs/webkit-gtk media-video/ffmpeg && sudo rc-update add tailscaled default && sudo rc-service tailscaled start && sudo tailscale up --operator=$USER`
   - **Debian / Ubuntu:** `curl -fsSL https://tailscale.com/install.sh | sh && sudo apt install -y libwebkit2gtk-4.1-0 ffmpeg && sudo tailscale up --operator=$USER`
2. **Copiar y ejecutar** el binario `molex-desktop` (o `.AppImage`): `chmod +x ./molex-desktop && ./molex-desktop`.
3. En **Dispositivos**, pulsa **+ Guardar** sobre el nodo de tu workstation en **Nodos detectados en Tailnet**, ingresa usuario SSH y pulsa **Conectar** (o **Detectar MAC por SSH** / **Encender PC**; si estás fuera de la LAN, configura **Relay WoL por SSH**).

### Como Host (para controlar la laptop remotamente)
- **Arch Linux:** `./scripts/install-host-daemon.sh && ./scripts/setup-arch-host.sh`
- **Gentoo Linux:** `./scripts/install-host-daemon.sh && ./scripts/setup-gentoo-host.sh`
- **Debian / Ubuntu:** instala `openssh-server`, `ethtool`, `wl-screenrec` (en `~/.cargo/bin`) y ejecuta `./scripts/install-host-daemon.sh`, habilita `ssh` y `tailscaled` (`sudo systemctl enable --now ssh tailscaled && sudo tailscale set --operator=$USER`) y lanza `molex-host-up` en tu compositor Wayland.
