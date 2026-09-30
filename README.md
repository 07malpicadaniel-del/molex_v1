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

- **Low-Latency Wayland Video Streaming**: Uses a high-speed TCP socket (port 9091) streaming JPEG frames natively from the daemon, entirely bypassing SSH encryption bottlenecks.
- **Dynamic Monitor Selection**: Queries real Wayland monitor outputs (e.g., `DP-1`, `HDMI-A-1`) via Rust FFI and renders a semi-transparent dropdown selector overlay on the streaming screen.
- **Interactive SSH Terminal**: Terminal emulator with history retention, non-blocking asynchronous execution, and real-time ANSI escape code filtering (`stripAnsiCodes`).
- **Advanced System Telemetry**: Real-time asynchronous polling via UDP (port 9090) of OS info, CPU load, RAM usage, GPU details, and network status rendered in dedicated Material3 cards.
- **Persistent Profile Management**: In-app SQLite database using Room (`DeviceEntity`, `DeviceDao`, `AppDatabase`) for offline profile persistence, auto-reconnection capabilities, and credentials sanitization (`trim()`).
- **Immersive Full-Screen Desktop View**:
  - Automatically locks landscape orientation on `RemoteScreen`.
  - Dynamically hides the bottom `NavigationBar` for full-screen display.
  - Floating overlay drawer menu for quick navigation and a dedicated **Disconnect** action.
- **Zero-Latency Touch & Mouse Injection Architecture**: Mathematical mapping for ultrawide (21:9) monitors discarding letterbox/pillarbox zones. Translates active Android touch gestures (drag, tap, long press) into normalized relative coordinates (`0.0f` to `1.0f`) based on exact visible bounds and transmits mouse movement/click events via high-speed UDP directly to a virtual Linux `uinput` device bound to Hyprland's native output (e.g. DP-4).
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

## 🔮 Phase 4: Optimization & Release (Upcoming)

1. **Local Storage (SQLite via Room)**:
   - Migration of server profile management from in-memory lists to a persistent SQLite database using Room (`DeviceEntity`, `DeviceDao`, `AppDatabase`).
   - Enables offline profile persistence, automatic reconnection to last known servers, and CRUD operations for saved Wayland hosts.

## 🔮 Phase 4.3: H.264 Video Transport & Hardware Decoding (Upcoming)

### The Roadmap to Zero-Latency Streaming

To eradicate the lag inherent in the provisional `grim` (JPEG on CPU) over TCP architecture, the next definitive step is a full end-to-end H.264 video pipeline.

```mermaid
graph TD
    A[Hyprland DP-4 <br> DMA-BUF / ext-image-capture] --> B[wl-screenrec + NVENC <br> RTX 5060 Ti]
    B -->|H.264 NAL Units via stdout| C[TCP Socket 9091 <br> Continuous Memory Stream]
    C -->|Binary TCP Stream| D[Android MediaCodec <br> Tensor G5]
    D -->|Hardware Decode < 10ms| E[AndroidView SurfaceView]
```

1. **Backend (Rust Motor Refactor)**:
   - Complete replacement of `grim` snapshot polling.
   - The Linux Daemon will launch a continuous `wl-screenrec` subprocess using `Command::new("wl-screenrec")`.
   - `wl-screenrec` will utilize **NVENC** hardware encoding, and its `stdout` will be directly piped into the high-speed TCP socket on port `9091` as raw H.264 NAL units.

2. **Frontend (Android Client Refactor)**:
   - Deprecation of `BitmapFactory` and Compose's `Image(bitmap)`.
   - Integration of Android's native `android.media.MediaCodec` configured for `video/avc`.
   - Rendering directly onto a low-level `SurfaceView` embedded in Compose via `AndroidView(factory = { SurfaceView(it) })`.
   - Leveraging Android device silicon (e.g., Tensor G5) for ultra-fast hardware decoding, bringing end-to-end display latency below 15ms.

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

- **Frontend**: Kotlin, Jetpack Compose, Material 3, Coroutines, StateFlow, Navigation Compose, Room (SQLite).
- **Backend**: Rust, UniFFI, SSH2, Tokio, uinput, evdev, Hyprland IPC.
- **Platform**: Android SDK, Linux Wayland.
