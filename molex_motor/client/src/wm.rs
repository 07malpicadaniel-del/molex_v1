use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use serde_json::Value;

use crate::models::{MolexError, MonitorInfo};
use crate::ssh_video::MolexVideoClient;

const WAYLAND_ENV_PREAMBLE: &str = concat!(
    "export XDG_RUNTIME_DIR=\"${XDG_RUNTIME_DIR:-/run/user/$(id -u)}\"; ",
    "if [ -z \"$WAYLAND_DISPLAY\" ]; then ",
    "W_SOCK=$(ls -t \"$XDG_RUNTIME_DIR\"/wayland-[0-9]* 2>/dev/null | grep -v '\\.lock$' | head -n 1); ",
    "[ -n \"$W_SOCK\" ] && export WAYLAND_DISPLAY=$(basename \"$W_SOCK\"); ",
    "fi; ",
    "if [ -z \"$HYPRLAND_INSTANCE_SIGNATURE\" ] && [ -d \"$XDG_RUNTIME_DIR/hypr\" ]; then ",
    "export HYPRLAND_INSTANCE_SIGNATURE=$(ls -t \"$XDG_RUNTIME_DIR/hypr\" 2>/dev/null | head -n 1); ",
    "fi; ",
    "if [ -z \"$NIRI_SOCKET\" ]; then ",
    "N_SOCK=$(ls -t \"$XDG_RUNTIME_DIR\"/niri.*.sock 2>/dev/null | head -n 1); ",
    "[ -n \"$N_SOCK\" ] && export NIRI_SOCKET=\"$N_SOCK\"; ",
    "fi"
);

impl MolexVideoClient {
    pub(crate) async fn fetch_monitors_detailed_internal(&self) -> Result<Vec<MonitorInfo>, MolexError> {
        let cmd = format!(
            "{}; if command -v hyprctl >/dev/null 2>&1 && [ -n \"$HYPRLAND_INSTANCE_SIGNATURE\" ]; then \
               echo \"__COMPOSITOR__:hyprland\"; hyprctl monitors -j 2>/dev/null || hyprctl monitors; \
             elif command -v niri >/dev/null 2>&1 && [ -n \"$NIRI_SOCKET\" ]; then \
               echo \"__COMPOSITOR__:niri\"; niri msg -j outputs 2>/dev/null || niri msg outputs; \
             elif command -v wlr-randr >/dev/null 2>&1; then \
               echo \"__COMPOSITOR__:wlr-randr\"; wlr-randr --json 2>/dev/null || wlr-randr; \
             fi",
            WAYLAND_ENV_PREAMBLE
        );

        let output = self.run_command(&cmd).await?;
        if output.starts_with("[No output") {
            return Ok(vec![]);
        }

        Ok(parse_compositor_monitors_output(&output))
    }

    pub(crate) async fn fetch_monitors_internal(&self) -> Result<Vec<String>, MolexError> {
        let detailed = self.fetch_monitors_detailed_internal().await?;
        Ok(detailed.into_iter().map(|m| m.name).collect())
    }

    pub(crate) async fn capture_frame_internal(
        &self,
        monitor_name: Option<String>,
    ) -> Result<String, MolexError> {
        let clean_name = monitor_name
            .unwrap_or_default()
            .trim()
            .replace(['\r', '\n'], "");

        let output_flag = if !clean_name.is_empty() {
            format!("-o {} ", clean_name)
        } else {
            String::new()
        };

        let cmd = format!(
            "{}; grim {} -t jpeg -q 30 - | base64 -w 0",
            WAYLAND_ENV_PREAMBLE, output_flag
        );

        let base64_image = self.run_command(&cmd).await?;

        if base64_image.is_empty()
            || base64_image.contains(' ')
            || base64_image.starts_with("[No output")
        {
            return Err(MolexError::Generic(format!(
                "Fallo en grim: {}",
                base64_image
            )));
        }

        let mut hasher = DefaultHasher::new();
        base64_image.hash(&mut hasher);
        let current_hash = hasher.finish();

        let mut last_hash = self.last_frame_hash.lock().await;
        if *last_hash == current_hash {
            return Ok("SAME_FRAME".to_string());
        }

        *last_hash = current_hash;
        Ok(base64_image)
    }
}

pub fn parse_compositor_monitors_output(raw: &str) -> Vec<MonitorInfo> {
    let mut lines = raw.lines();
    let first = lines.next().unwrap_or("").trim();

    let (compositor, body) = if let Some(comp) = first.strip_prefix("__COMPOSITOR__:") {
        (comp.trim(), lines.collect::<Vec<_>>().join("\n"))
    } else {
        ("hyprland", raw.to_string())
    };

    match compositor {
        "hyprland" => parse_hyprland_monitors(&body),
        "niri" => parse_niri_outputs(&body),
        "wlr-randr" => parse_wlr_randr_outputs(&body),
        _ => parse_hyprland_monitors(&body),
    }
}

fn parse_hyprland_monitors(raw: &str) -> Vec<MonitorInfo> {
    let trimmed = raw.trim();
    if trimmed.starts_with('[') {
        if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(trimmed) {
            let mut result = Vec::new();
            for item in arr {
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let width = item.get("width").and_then(|v| v.as_u64()).unwrap_or(1920) as u32;
                let height = item.get("height").and_then(|v| v.as_u64()).unwrap_or(1080) as u32;
                let refresh_hz = item
                    .get("refreshRate")
                    .and_then(|v| v.as_f64())
                    .map(|r| r.round() as u32)
                    .unwrap_or(60);
                let focused = item
                    .get("focused")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                result.push(MonitorInfo {
                    name,
                    width,
                    height,
                    refresh_hz,
                    focused,
                    compositor: "Hyprland".to_string(),
                });
            }
            if !result.is_empty() {
                return result;
            }
        }
    }

    // Fallback texto plano de `hyprctl monitors`
    let mut monitors = Vec::new();
    let mut current: Option<MonitorInfo> = None;

    for line in raw.lines() {
        let l = line.trim();
        if l.starts_with("Monitor ") {
            if let Some(m) = current.take() {
                monitors.push(m);
            }
            if let Some(name) = l.split_whitespace().nth(1) {
                current = Some(MonitorInfo {
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
                    if let Some(hz_str) = rest.split_whitespace().next() {
                        m.refresh_hz = hz_str.parse::<f64>().map(|v| v.round() as u32).unwrap_or(60);
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

fn parse_niri_outputs(raw: &str) -> Vec<MonitorInfo> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(trimmed) {
            let mut result = Vec::new();
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

                let current_idx = item.get("current_mode").and_then(|v| v.as_u64()).map(|v| v as usize);
                if let (Some(idx), Some(modes)) =
                    (current_idx, item.get("modes").and_then(|v| v.as_array()))
                {
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
                } else if let Some(logical) = item.get("logical") {
                    width = logical.get("width").and_then(|v| v.as_u64()).unwrap_or(1920) as u32;
                    height = logical.get("height").and_then(|v| v.as_u64()).unwrap_or(1080) as u32;
                }

                result.push(MonitorInfo {
                    name,
                    width,
                    height,
                    refresh_hz,
                    focused: false,
                    compositor: "Niri".to_string(),
                });
            }
            if !result.is_empty() {
                result[0].focused = true;
                return result;
            }
        }
    }

    // Fallback texto de `niri msg outputs`
    let mut monitors = Vec::new();
    let mut current: Option<MonitorInfo> = None;

    for line in raw.lines() {
        let l = line.trim();
        if l.starts_with("Output ") {
            if let Some(m) = current.take() {
                monitors.push(m);
            }
            // Output "eDP-1" (BOE ...)
            let name = if let Some(start) = l.find('(') {
                let before = l[..start].trim();
                before
                    .strip_prefix("Output ")
                    .unwrap_or(before)
                    .trim_matches('"')
                    .trim()
                    .to_string()
            } else {
                l.strip_prefix("Output ")
                    .unwrap_or(l)
                    .trim_matches('"')
                    .trim()
                    .to_string()
            };
            if !name.is_empty() {
                current = Some(MonitorInfo {
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
                    if let Some(hz_str) = rest.split_whitespace().next() {
                        m.refresh_hz = hz_str.parse::<f64>().map(|v| v.round() as u32).unwrap_or(60);
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

fn parse_wlr_randr_outputs(raw: &str) -> Vec<MonitorInfo> {
    let trimmed = raw.trim();
    if trimmed.starts_with('[') {
        if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(trimmed) {
            let mut result = Vec::new();
            for item in arr {
                let enabled = item.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true);
                if !enabled {
                    continue;
                }
                let name = item
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
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
                result.push(MonitorInfo {
                    name,
                    width,
                    height,
                    refresh_hz,
                    focused: result.is_empty(),
                    compositor: "Wayland".to_string(),
                });
            }
            if !result.is_empty() {
                return result;
            }
        }
    }

    let mut monitors = Vec::new();
    for line in raw.lines() {
        if !line.starts_with(' ') && !line.starts_with('\t') && !line.trim().is_empty() {
            if let Some(name) = line.split_whitespace().next() {
                monitors.push(MonitorInfo {
                    name: name.trim_matches('"').to_string(),
                    width: 1920,
                    height: 1080,
                    refresh_hz: 60,
                    focused: monitors.is_empty(),
                    compositor: "Wayland".to_string(),
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
    fn parses_hyprland_json() {
        let sample = "__COMPOSITOR__:hyprland\n[{\"name\":\"DP-4\",\"width\":2560,\"height\":1080,\"refreshRate\":143.99,\"focused\":true}]";
        let mons = parse_compositor_monitors_output(sample);
        assert_eq!(mons.len(), 1);
        assert_eq!(mons[0].name, "DP-4");
        assert_eq!(mons[0].width, 2560);
        assert_eq!(mons[0].height, 1080);
        assert_eq!(mons[0].refresh_hz, 144);
        assert!(mons[0].focused);
    }

    #[test]
    fn parses_niri_json() {
        let sample = "__COMPOSITOR__:niri\n{\"eDP-1\":{\"name\":\"eDP-1\",\"modes\":[{\"width\":1920,\"height\":1080,\"refresh_rate\":144003}],\"current_mode\":0}}";
        let mons = parse_compositor_monitors_output(sample);
        assert_eq!(mons.len(), 1);
        assert_eq!(mons[0].name, "eDP-1");
        assert_eq!(mons[0].width, 1920);
        assert_eq!(mons[0].refresh_hz, 144);
        assert_eq!(mons[0].compositor, "Niri");
    }
}
