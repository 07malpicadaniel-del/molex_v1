# Molex v1

Molex is an ultra-low latency remote desktop platform specifically designed for controlling Linux Wayland environments from an Android device.

By leveraging a hybrid Rust/Kotlin architecture, Molex achieves high performance, minimal CPU overhead, and responsive user interaction inspired by game-streaming platforms like Moonlight.

---

## 🏗️ Monorepo Architecture

The repository is structured as a monorepo consisting of two core components:

```
molex_v1/
├── molex_motor/               # Rust Native Backend
│   ├── client/                # SSH & FFI Client bindings (UniFFI)
│   │   ├── src/               # Native Rust logic (SSH video capture & UDP input)
│   │   └── bindings/          # Auto-generated Kotlin FFI bindings (.so)
│   └── daemon/                # Linux Daemon for kernel-level input injection (uinput)
└── app/                       # Android Frontend
    └── src/main/java/...      # Kotlin + Jetpack Compose UI & ViewModel
```

### 1. Backend (Rust Motor)
- **`client` submodule**: Compiles to a dynamic library (`libclient.so`) using `UniFFI` bindings for Android JNI integration. It manages:
  - SSH channel creation and frame retrieval via `MolexVideoClient`.
  - Remote command execution via `executeCommand(cmd)`.
  - UDP event transmission for low-latency input control via `MolexInputClient`.
- **`daemon` submodule**: Runs as a lightweight Linux daemon on the host machine to inject keyboard and mouse events directly into the Linux kernel via `uinput`/`evdev`.

### 2. Frontend (Android / Kotlin & Jetpack Compose)
- **Framework**: Jetpack Compose (Material3) with `StateFlow` and coroutines (`Dispatchers.IO`).
- **ViewModel (`MolexViewModel`)**: Decouples network I/O and FFI calls from the Compose rendering loop to guarantee 60 FPS UI performance without main thread blocking.
- **Navigation**: Clean single-activity architecture with Compose Navigation, Material3 `NavigationBar`, and route-based conditional bottom bar visibility.

---

## ✨ Production Features

- **Low-Latency Video Streaming via SSH**: Uses `grim` for Wayland screen captures encoded as JPEG over Base64, with frame skipping (`SAME_FRAME`) to optimize bandwidth and CPU usage.
- **Interactive SSH Terminal**: Built-in terminal emulator with command history state retention, asynchronous non-blocking command execution, and ANSI escape code filtering (`stripAnsiCodes`).
- **Advanced System Telemetry**: Real-time asynchronous polling of OS info, CPU load, RAM usage, GPU details, and network status rendered in dedicated Material3 cards.
- **FFI Profile Management**: Dynamic in-memory server profile management (`ServerProfile`), supporting custom SSH host, port, username, and password credentials with automatic input sanitization.
- **Immersive Full-Screen Desktop View**:
  - Automatically locks landscape orientation on `RemoteScreen`.
  - Dynamically hides the bottom `NavigationBar` for full-screen display.
  - Subtle floating overlay drawer menu for quick navigation and returning to device settings.
- **Zero-Latency Touch & Mouse Injection Architecture**: Translates Android touch gestures (drag, tap, long press) into normalized relative coordinates (`0.0f` to `1.0f`) based on visual container dimensions (`ContentScale.FillBounds`) and transmits mouse movement/click events directly to Linux `uinput`.

---

## 🚀 Getting Started

### Prerequisites
- **Host (Linux / Wayland)**:
  - Running Wayland compositor (e.g., Sway, Hyprland, GNOME Wayland).
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
- **Backend**: Rust, UniFFI, SSH2, Tokio, uinput, evdev.
- **Platform**: Android SDK, Linux Wayland.
