# Molex v1

Molex is an ultra-low latency remote desktop platform specifically designed for controlling Linux Wayland environments (Sway, Hyprland, GNOME) from an Android device.

By leveraging a hybrid Rust/Kotlin architecture, Molex achieves high performance, minimal CPU overhead, and responsive user interaction inspired by game-streaming platforms like Moonlight.

---

## 🏗️ Monorepo Architecture

The repository is structured as a monorepo consisting of two core components:

```
molex_v1/
├── molex_motor/               # Rust Native Backend
│   ├── client/                # SSH & FFI Client bindings (UniFFI)
│   │   ├── src/               # Modularized Rust code
│   │   │   ├── lib.rs         # UniFFI interface exports & library entry point
│   │   │   ├── models.rs      # Data structures, exceptions, & FFI types
│   │   │   ├── wm.rs          # Wayland / Hyprland compositor logic & capture
│   │   │   └── ssh_video.rs   # SSH session management & FFI client implementation
│   │   └── bindings/          # Auto-generated Kotlin FFI bindings (.so)
│   └── daemon/                # Linux Daemon for kernel-level input injection (uinput)
└── app/                       # Android Frontend (Kotlin & Jetpack Compose)
    └── src/main/java/...      # UI Screens, Navigation & MolexViewModel
```

### 1. Backend (Rust Motor)
- **`client` submodule**: Compiles to a dynamic library (`libclient.so`) using `UniFFI` bindings for Android JNI integration:
  - **`models.rs`**: Shared data types (`ServerProfile`, `SystemMetrics`) and error types.
  - **`wm.rs`**: Hyprland IPC / Wayland monitor discovery and frame capture execution.
  - **`ssh_video.rs`**: SSH2 session handling, remote `executeCommand()`, video streaming via `MolexVideoClient`, and system telemetry polling.
  - **`MolexInputClient`**: Low-latency UDP socket transmission for input events.
- **`daemon` submodule**: Runs as a lightweight Linux daemon on the host machine to inject keyboard and mouse events directly into the Linux kernel via `uinput`/`evdev`.

### 2. Frontend (Android / Kotlin & Jetpack Compose)
- **Framework**: Jetpack Compose (Material 3) with `StateFlow` and Coroutines (`Dispatchers.IO`).
- **`MolexViewModel`**: Asynchronously manages FFI connections, video frames, telemetry polling, SSH terminal commands, and monitor selection state without blocking the Main thread.
- **Navigation & Enforced Routing**: Single-activity architecture enforcing initial entry via `DevicesScreen` / `SessionsScreen` to prevent null client interactions.

---

## ✨ Production Features

- **Low-Latency Wayland Video Streaming**: Uses `grim` for screen captures encoded as JPEG over Base64, with frame skipping (`SAME_FRAME`) to optimize bandwidth and CPU usage.
- **Dynamic Monitor Selection**: Queries real Wayland monitor outputs (e.g., `DP-1`, `HDMI-A-1`) via Rust FFI and renders a semi-transparent dropdown selector overlay on the streaming screen.
- **Interactive SSH Terminal**: Terminal emulator with history retention, non-blocking asynchronous execution, and real-time ANSI escape code filtering (`stripAnsiCodes`).
- **Advanced System Telemetry**: Real-time asynchronous polling of OS info, CPU load, RAM usage, GPU details, and network status rendered in dedicated Material3 cards.
- **FFI Profile Management**: Dynamic in-memory server profile management (`ServerProfile`), supporting custom SSH host, port, username, and password credentials with automatic input sanitization.
- **Immersive Full-Screen Desktop View**:
  - Automatically locks landscape orientation on `RemoteScreen`.
  - Dynamically hides the bottom `NavigationBar` for full-screen display.
  - Floating overlay drawer menu for quick navigation and a dedicated **Disconnect** action.
- **Full-Screen Error Overlay**: Catches connection timeouts or FFI exceptions and renders a full-screen dark red alert UI (`Color(0xFF2B0000)`) with error messages and a "Back to Devices" action.

---

## 🐛 Resolved Bugs & Debugging Log

1. **ANR Prevention (Application Not Responding)**:
   - *Problem*: `MolexVideoClient` and `getMonitors()` initialization were previously executed on the Main thread inside `connectToServer()`, causing UI freezes.
   - *Solution*: Refactored `connectToServer()` to launch all UniFFI instantiations and network I/O strictly inside `viewModelScope.launch(Dispatchers.IO)`.

2. **Silent Base64 Decoding Failures**:
   - *Problem*: When Rust returned a plain text bash error instead of a valid Base64 JPEG frame, `BitmapFactory.decodeByteArray()` returned `null` silently, leaving the UI frozen on the last valid frame.
   - *Solution*: Added explicit `null` checks on decoded bitmaps in `startVideoLoop()`. If decoding fails, the error message is logged and `VideoState.Error` is emitted immediately to trigger the red error UI.

3. **Hyprland SSH Environment Blindness**:
   - *Problem*: `hyprctl` commands executed over SSH failed to detect active monitors because SSH non-interactive shells lacked Wayland environment variables.
   - *Solution*: Implemented dynamic socket discovery in Rust (`wm.rs`), scanning `/run/user/1000/hypr` to discover and inject `HYPRLAND_INSTANCE_SIGNATURE` automatically into the SSH environment.

---

## 📌 Pending Tasks & Next Session

- [ ] **Monitor Pivoting Bug**: While Wayland monitor outputs (e.g., `DP-1`, `HDMI-A-1`) are discovered and selectable in the UI, switching monitors updates the state but `grim` continues capturing the primary output. Need to verify the `-o <output>` flag handling in Rust (`wm.rs`).
- [ ] **UDP Input Engine Integration**: Complete the UDP input receiver server in Rust and connect Compose touch events (`xPercent`, `yPercent`) to transmit movement and click events directly to Linux `uinput`/`evdev`.

---

## 🚀 Getting Started

### Prerequisites
- **Host (Linux / Wayland)**:
  - Running Wayland compositor (e.g., Hyprland, Sway, GNOME Wayland).
  - `grim` installed for screen capture (`sudo apt install grim` / `pacman -S grim`).
  - SSH server (`sshd`) enabled and accessible.
- **Android Device**:
  - Android 8.0 (API level 26) or higher.

### Building the Project
1. **Compile Rust FFI Bindings**:
   ```bash
   cd molex_motor/client
   cargo build --target aarch64-linux-android --release
   ```
2. **Build Android Application**:
   Open the root project in Android Studio and run:
   ```bash
   ./gradlew assembleDebug
   ```

---

## 🛠️ Tech Stack

- **Frontend**: Kotlin, Jetpack Compose, Material 3, Coroutines, StateFlow, Navigation Compose.
- **Backend**: Rust, UniFFI, SSH2, Tokio, uinput, evdev, Hyprland IPC.
- **Platform**: Android SDK, Linux Wayland.
