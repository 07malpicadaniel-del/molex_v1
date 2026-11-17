use serde_json::Value;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedMonitor {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub refresh_hz: u32,
    pub focused: bool,
    pub compositor: String,
}

#[derive(Debug, Clone)]
pub struct DaemonConfig {
    pub monitor: Option<String>,
    pub video_port: u16,
    pub input_port: u16,
    pub fps: u32,
    pub gop_size: u32,
    pub codec: String,
    pub experimental_vulkan: bool,
    pub low_power: Option<String>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            monitor: None,
            video_port: 9091,
            input_port: 9090,
            fps: 60,
            gop_size: 30,
            codec: "avc".to_string(),
            experimental_vulkan: false,
            low_power: None,
        }
    }
}

impl DaemonConfig {
    pub fn from_env_and_args() -> Self {
        let mut cfg = Self::default();

        if let Ok(v) = std::env::var("MOLEX_MONITOR") {
            let trimmed = v.trim().to_string();
            if !trimmed.is_empty() {
                cfg.monitor = Some(trimmed);
            }
        }
        if let Ok(v) = std::env::var("MOLEX_VIDEO_PORT") {
            if let Ok(port) = v.parse() {
                cfg.video_port = port;
            }
        }
        if let Ok(v) = std::env::var("MOLEX_INPUT_PORT") {
            if let Ok(port) = v.parse() {
                cfg.input_port = port;
            }
        }
        if let Ok(v) = std::env::var("MOLEX_FPS") {
            if let Ok(fps) = v.parse() {
                cfg.fps = fps;
            }
        }
        if let Ok(v) = std::env::var("MOLEX_GOP") {
            if let Ok(gop) = v.parse() {
                cfg.gop_size = gop;
            }
        }
        if let Ok(v) = std::env::var("MOLEX_CODEC") {
            if !v.trim().is_empty() {
                cfg.codec = v.trim().to_string();
            }
        }
        if let Ok(v) = std::env::var("MOLEX_EXPERIMENTAL_VULKAN") {
            cfg.experimental_vulkan = matches!(v.trim(), "1" | "true" | "TRUE" | "yes");
        }
        if let Ok(v) = std::env::var("MOLEX_LOW_POWER") {
            let trimmed = v.trim().to_string();
            if !trimmed.is_empty() {
                cfg.low_power = Some(trimmed);
            }
        }

        let mut args = std::env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--monitor" | "-o" => {
                    if let Some(val) = args.next() {
                        cfg.monitor = Some(val);
                    }
                }
                "--video-port" => {
                    if let Some(val) = args.next().and_then(|s| s.parse().ok()) {
                        cfg.video_port = val;
                    }
                }
                "--input-port" => {
                    if let Some(val) = args.next().and_then(|s| s.parse().ok()) {
                        cfg.input_port = val;
                    }
                }
                "--fps" => {
                    if let Some(val) = args.next().and_then(|s| s.parse().ok()) {
                        cfg.fps = val;
                    }
                }
                "--gop" => {
                    if let Some(val) = args.next().and_then(|s| s.parse().ok()) {
                        cfg.gop_size = val;
                    }
                }
                "--codec" => {
                    if let Some(val) = args.next() {
                        cfg.codec = val;
                    }
                }
                "--experimental-vulkan" => {
                    cfg.experimental_vulkan = true;
                }
                "--no-experimental-vulkan" => {
                    cfg.experimental_vulkan = false;
                }
                "--low-power" => {
                    if let Some(val) = args.next() {
                        cfg.low_power = Some(val);
                    }
                }
                _ => {}
            }
        }

        cfg
    }
}

/// Detecta monitores disponibles en Hyprland, Niri o Wayland genérico (`wlr-randr`).
pub fn detect_monitors() -> Vec<DetectedMonitor> {
    // 1. Hyprland (`hyprctl monitors -j` o texto)
    if let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let parsed = parse_hyprland_monitors_json(&text);
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }
    if let Ok(out) = Command::new("hyprctl").arg("monitors").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let parsed = parse_hyprland_monitors_text(&text);
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }

    // 2. Niri (`niri msg -j outputs` o `niri msg outputs`)
    if let Ok(out) = Command::new("niri").args(["msg", "-j", "outputs"]).output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let parsed = parse_niri_outputs_json(&text);
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }
    if let Ok(out) = Command::new("niri").args(["msg", "outputs"]).output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let parsed = parse_niri_outputs_text(&text);
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }

    // 3. Wayland genérico (`wlr-randr`)
    if let Ok(out) = Command::new("wlr-randr").arg("--json").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let parsed = parse_wlr_randr_json(&text);
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }
    if let Ok(out) = Command::new("wlr-randr").output() {
        if out.status.success() {
            let text = String::from_utf8_lossy(&out.stdout);
            let parsed = parse_wlr_randr_text(&text);
            if !parsed.is_empty() {
                return parsed;
            }
        }
    }

    vec![]
}

pub fn select_best_monitor(monitors: &[DetectedMonitor]) -> DetectedMonitor {
    if let Some(focused) = monitors.iter().find(|m| m.focused) {
        return focused.clone();
    }
    if let Some(ultrawide) = monitors.iter().find(|m| m.width == 2560 && m.height == 1080) {
        return ultrawide.clone();
    }
    if let Some(highest) = monitors.iter().max_by_key(|m| m.width * m.height) {
        return highest.clone();
    }
    DetectedMonitor {
        name: "DP-1".to_string(),
        width: 1920,
        height: 1080,
        refresh_hz: 60,
        focused: true,
        compositor: "Fallback".to_string(),
    }
}

pub fn parse_hyprland_monitors_json(raw: &str) -> Vec<DetectedMonitor> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(raw.trim()) else {
        return vec![];
    };
    arr.into_iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.trim().to_string();
            if name.is_empty() {
                return None;
            }
            let width = item.get("width").and_then(|v| v.as_u64()).unwrap_or(1920) as u32;
            let height = item.get("height").and_then(|v| v.as_u64()).unwrap_or(1080) as u32;
            let refresh_hz = item
                .get("refreshRate")
                .and_then(|v| v.as_f64())
                .map(|r| r.round() as u32)
                .unwrap_or(60);
            let focused = item.get("focused").and_then(|v| v.as_bool()).unwrap_or(false);
            Some(DetectedMonitor {
                name,
                width,
                height,
                refresh_hz,
                focused,
                compositor: "Hyprland".to_string(),
            })
        })
        .collect()
}

pub fn parse_hyprland_monitors_text(raw: &str) -> Vec<DetectedMonitor> {
    let mut monitors = Vec::new();
    let mut current: Option<DetectedMonitor> = None;

    for line in raw.lines() {
        let l = line.trim();
        if l.starts_with("Monitor ") {
            if let Some(m) = current.take() {
                monitors.push(m);
            }
            if let Some(name) = l.split_whitespace().nth(1) {
                current = Some(DetectedMonitor {
                    name: name.to_string(),
                    width: 1920,
                    height: 1080,
                    refresh_hz: 60,
                    focused: false,
                    compositor: "Hyprland".to_string(),
                });
            }
        } else if let Some(ref mut m) = current {
            if l.contains('@') && l.contains('x') {
                if let Some((res, rest)) = l.split_once('@') {
                    if let Some((w, h)) = res.split_once('x') {
                        m.width = w.trim().parse().unwrap_or(m.width);
                        m.height = h.trim().parse().unwrap_or(m.height);
                    }
                    if let Some(hz) = rest.split_whitespace().next() {
                        m.refresh_hz = hz.parse::<f64>().map(|v| v.round() as u32).unwrap_or(60);
                    }
                }
            } else if l.starts_with("focused:") && l.contains("yes") {
                m.focused = true;
            }
        }
    }

    if let Some(m) = current {
        monitors.push(m);
    }
    monitors
}

pub fn parse_niri_outputs_json(raw: &str) -> Vec<DetectedMonitor> {
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(raw.trim()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for (key, item) in map {
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&key)
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let mut width = 1920u32;
        let mut height = 1080u32;
        let mut refresh_hz = 60u32;
        if let (Some(idx), Some(modes)) = (
            item.get("current_mode").and_then(|v| v.as_u64()).map(|v| v as usize),
            item.get("modes").and_then(|v| v.as_array()),
        ) {
            if let Some(mode) = modes.get(idx) {
                width = mode.get("width").and_then(|v| v.as_u64()).unwrap_or(1920) as u32;
                height = mode.get("height").and_then(|v| v.as_u64()).unwrap_or(1080) as u32;
                let raw_hz = mode.get("refresh_rate").and_then(|v| v.as_u64()).unwrap_or(60000);
                refresh_hz = if raw_hz > 1000 {
                    ((raw_hz as f64) / 1000.0).round() as u32
                } else {
                    raw_hz as u32
                };
            }
        }
        out.push(DetectedMonitor {
            name,
            width,
            height,
            refresh_hz,
            focused: out.is_empty(),
            compositor: "Niri".to_string(),
        });
    }
    out
}

pub fn parse_niri_outputs_text(raw: &str) -> Vec<DetectedMonitor> {
    let mut monitors = Vec::new();
    let mut current: Option<DetectedMonitor> = None;

    for line in raw.lines() {
        let l = line.trim();
        if l.starts_with("Output ") {
            if let Some(m) = current.take() {
                monitors.push(m);
            }
            let name = if let Some(idx) = l.find('(') {
                l[..idx]
                    .trim()
                    .strip_prefix("Output ")
                    .unwrap_or("")
                    .trim_matches('"')
                    .trim()
                    .to_string()
            } else {
                l.strip_prefix("Output ")
                    .unwrap_or("")
                    .trim_matches('"')
                    .trim()
                    .to_string()
            };
            if !name.is_empty() {
                current = Some(DetectedMonitor {
                    name,
                    width: 1920,
                    height: 1080,
                    refresh_hz: 60,
                    focused: monitors.is_empty(),
                    compositor: "Niri".to_string(),
                });
            }
        } else if let Some(ref mut m) = current {
            if l.starts_with("Current mode:") {
                let val = l.trim_start_matches("Current mode:").trim();
                if let Some((res, rest)) = val.split_once('@') {
                    if let Some((w, h)) = res.split_once('x') {
                        m.width = w.trim().parse().unwrap_or(m.width);
                        m.height = h.trim().parse().unwrap_or(m.height);
                    }
                    if let Some(hz) = rest.split_whitespace().next() {
                        m.refresh_hz = hz.parse::<f64>().map(|v| v.round() as u32).unwrap_or(60);
                    }
                }
            }
        }
    }

    if let Some(m) = current {
        monitors.push(m);
    }
    monitors
}

pub fn parse_wlr_randr_json(raw: &str) -> Vec<DetectedMonitor> {
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(raw.trim()) else {
        return vec![];
    };
    let mut out = Vec::new();
    for item in arr {
        if !item.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true) {
            continue;
        }
        let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let mut width = 1920u32;
        let mut height = 1080u32;
        let mut refresh_hz = 60u32;
        if let Some(modes) = item.get("modes").and_then(|v| v.as_array()) {
            if let Some(cur) = modes
                .iter()
                .find(|m| m.get("current").and_then(|v| v.as_bool()).unwrap_or(false))
                .or_else(|| modes.first())
            {
                width = cur.get("width").and_then(|v| v.as_u64()).unwrap_or(1920) as u32;
                height = cur.get("height").and_then(|v| v.as_u64()).unwrap_or(1080) as u32;
                refresh_hz = cur
                    .get("refresh")
                    .and_then(|v| v.as_f64())
                    .map(|r| r.round() as u32)
                    .unwrap_or(60);
            }
        }
        out.push(DetectedMonitor {
            name: name.to_string(),
            width,
            height,
            refresh_hz,
            focused: out.is_empty(),
            compositor: "wlr-randr".to_string(),
        });
    }
    out
}

pub fn parse_wlr_randr_text(raw: &str) -> Vec<DetectedMonitor> {
    let mut monitors = Vec::new();
    for line in raw.lines() {
        if !line.starts_with(' ') && !line.starts_with('\t') && !line.trim().is_empty() {
            if let Some(name) = line.split_whitespace().next() {
                monitors.push(DetectedMonitor {
                    name: name.trim_matches('"').to_string(),
                    width: 1920,
                    height: 1080,
                    refresh_hz: 60,
                    focused: monitors.is_empty(),
                    compositor: "wlr-randr".to_string(),
                });
            }
        }
    }
    monitors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hyprland_and_niri_text() {
        let hypr = "Monitor DP-4 (ID 0):\n\t2560x1080@143.99800 at 0x0\n\tfocused: yes\n";
        let h = parse_hyprland_monitors_text(hypr);
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].name, "DP-4");
        assert_eq!(h[0].width, 2560);
        assert!(h[0].focused);

        let niri = "Output \"eDP-1\" (BOE 0x095F)\n  Current mode: 1920x1080 @ 144.000 Hz\n";
        let n = parse_niri_outputs_text(niri);
        assert_eq!(n.len(), 1);
        assert_eq!(n[0].name, "eDP-1");
        assert_eq!(n[0].width, 1920);
        assert_eq!(n[0].refresh_hz, 144);
    }
}
