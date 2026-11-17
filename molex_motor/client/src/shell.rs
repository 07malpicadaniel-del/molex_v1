/// Utilidades de manejo de shell SSH y persistencia de directorio de trabajo (`current_dir`).

/// Escapa caracteres especiales dentro de una cadena entrecomillada con comillas dobles en POSIX shell.
pub fn escape_double_quoted(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' | '"' | '$' | '`' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Elimina secuencias de escape ANSI (colores/cursor) de la salida de terminal Linux.
pub fn strip_ansi_codes(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1B' {
            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC sequence terminated by BEL (\x07) or ST (\x1B\\)
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1B' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Detecta si el comando es una invocación pura de `cd` (sin operadores compuestos `&&`, `||`, `;`, `|`).
pub fn parse_cd_target(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim();
    if trimmed == "cd" {
        return Some("~".to_string());
    }

    let rest = trimmed.strip_prefix("cd")?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }

    let target = rest.trim();
    if target.contains("&&") || target.contains("||") || target.contains(';') || target.contains('|') {
        return None;
    }

    if target.is_empty() {
        Some("~".to_string())
    } else {
        Some(target.to_string())
    }
}

/// Construye el comando de verificación que resuelve `pwd` tras un cambio de directorio.
pub fn build_cd_probe_command(cwd: &str, prev_dir: &str, target: &str) -> String {
    let resolved_target = if target == "-" && !prev_dir.is_empty() {
        format!("\"{}\"", escape_double_quoted(prev_dir))
    } else if target == "~" {
        "~".to_string()
    } else if let Some(subpath) = target.strip_prefix("~/") {
        // Permite expansión de `~` preservando espacios en el resto de la ruta.
        format!("~/{}", subpath)
    } else {
        target.to_string()
    };

    if cwd.is_empty() || resolved_target == "~" || resolved_target.starts_with("~/") {
        format!("cd {} && pwd", resolved_target)
    } else {
        format!("cd \"{}\" && cd {} && pwd", escape_double_quoted(cwd), resolved_target)
    }
}

/// Envuelve cualquier comando estándar dentro del directorio persistente actual.
pub fn wrap_command_with_cwd(cwd: &str, cmd: &str) -> String {
    let trimmed = cmd.trim();
    if cwd.is_empty() {
        trimmed.to_string()
    } else {
        format!("cd \"{}\" && ({})", escape_double_quoted(cwd), trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pure_cd_commands() {
        assert_eq!(parse_cd_target("cd"), Some("~".to_string()));
        assert_eq!(parse_cd_target("  cd   /var/log  "), Some("/var/log".to_string()));
        assert_eq!(parse_cd_target("cd ~/proyectos"), Some("~/proyectos".to_string()));
        assert_eq!(parse_cd_target("cd \"my dir\""), Some("\"my dir\"".to_string()));
        assert_eq!(parse_cd_target("cd /tmp && ls"), None);
        assert_eq!(parse_cd_target("echo cd"), None);
    }

    #[test]
    fn builds_cd_probe_and_wraps_cwd() {
        assert_eq!(
            build_cd_probe_command("/home/daniel", "", "proyectos"),
            "cd \"/home/daniel\" && cd proyectos && pwd"
        );
        assert_eq!(
            build_cd_probe_command("/home/daniel", "/etc", "-"),
            "cd \"/home/daniel\" && cd \"/etc\" && pwd"
        );
        assert_eq!(
            build_cd_probe_command("/etc", "", "~/code"),
            "cd ~/code && pwd"
        );
        assert_eq!(
            wrap_command_with_cwd("/home/daniel/my \"dir\"", "ls -la"),
            "cd \"/home/daniel/my \\\"dir\\\"\" && (ls -la)"
        );
    }

    #[test]
    fn strips_ansi_cleanly() {
        let colored = "\x1B[1;32mok\x1B[0m plain";
        assert_eq!(strip_ansi_codes(colored), "ok plain");
    }
}
