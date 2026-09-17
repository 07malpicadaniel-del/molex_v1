# Molex v1

Molex is an ultra-low latency remote desktop platform specifically designed for controlling Linux Wayland environments from an Android device.

By leveraging a hybrid Rust/Kotlin architecture, Molex achieves high performance, minimal CPU overhead, and responsive user interaction.

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
  - UDP event transmission for low-latency input control via `MolexInputClient`.
- **`daemon` submodule**: Runs as a lightweight Linux daemon on the host machine to inject keyboard and mouse events directly into the Linux kernel via `uinput`.

### 2. Frontend (Android / Kotlin & Jetpack Compose)
- **Framework**: Jetpack Compose (Material3) with `StateFlow` and coroutines (`Dispatchers.IO`).
- **ViewModel (`MolexViewModel`)**: Decouples network I/O and FFI calls from the Compose rendering loop to guarantee 60 FPS UI performance without main thread blocking.
- **Navigation**: Clean single-activity architecture with Compose Navigation and Material3 `NavigationBar`.

---

## ✨ Key Features

- **Low-Latency Video Streaming via SSH**: Uses `grim` for Wayland screen captures encoded as JPEG over Base64, with frame skipping (`SAME_FRAME`) to optimize bandwidth and CPU usage.
- **Real-time System Telemetry**: Fetches OS version, CPU load, and RAM usage metrics asynchronously through the native `MolexVideoClient`.
- **FFI Profile Management**: Dynamic in-memory server profile management (`ServerProfile`), supporting custom SSH host, port, username, and password credentials.
- **Interactive Touch & Mouse Gestures**: Translates Android touch gestures (drag, tap, long press) into normalized relative coordinates (0.0 to 1.0) and transmits mouse movements and click events.
- **Resilient State & Connection Management**: Robust error handling that detects SSH server disconnections, prevents infinite loops, and updates `VideoState` reactively.

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
- **Backend**: Rust, UniFFI, SSH2, Tokio, uinput.
- **Platform**: Android SDK, Linux Wayland.
