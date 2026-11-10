//! Claude Code CLI detection and sandboxed launch.
//!
//! Launching generates a one-shot `.cmd` script in the manager's launches dir
//! (env `set` lines are written by us — model names are strictly validated, so
//! nothing user-supplied is ever re-parsed by the shell), then opens it in
//! Windows Terminal when available, falling back to a plain console window.
//! Only the launched console's environment is touched; the user's global
//! Claude Code settings are never modified.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeInfo {
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

fn hide(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
}

/// Run a command with stdout piped, stderr dropped, no console window.
fn output_hidden(cmd: &mut Command) -> Option<std::process::Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    hide(cmd);
    cmd.output().ok()
}

pub fn detect() -> ClaudeInfo {
    // 1) PATH lookup
    let mut where_cmd = Command::new("where.exe");
    where_cmd.arg("claude");
    let mut path: Option<String> = output_hidden(&mut where_cmd)
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()?
                    .lines()
                    .next()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
            } else {
                None
            }
        });

    // 2) Well-known install locations
    if path.is_none() {
        if let Some(home) = std::env::var("USERPROFILE").ok() {
            let candidates = [
                PathBuf::from(&home).join(r"AppData\Roaming\npm\claude.cmd"),
                PathBuf::from(&home).join(r".local\bin\claude.exe"),
            ];
            path = candidates
                .iter()
                .find(|p| p.is_file())
                .map(|p| p.display().to_string());
        }
    }

    let version = path.as_ref().and_then(|_| version());

    ClaudeInfo {
        found: path.is_some() || version.is_some(),
        version,
        path,
    }
}

fn version() -> Option<String> {
    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/d", "/c", "claude", "--version"]);
    let out = output_hidden(&mut cmd)?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Strict model-name validation — the value is embedded in a `.cmd` script,
/// so the charset is deliberately minimal (letters, digits, `. _ - [ ]`).
pub fn valid_model_name(model: &str) -> bool {
    let mut chars = model.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphanumeric() => {}
        _ => return false,
    }
    let rest: Vec<char> = chars.collect();
    rest.len() <= 63
        && rest.iter().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '[' | ']')
        })
}

pub struct LaunchOptions<'a> {
    /// "proxy" | "normal"
    pub mode: &'a str,
    pub model: Option<&'a str>,
    pub folder: Option<&'a str>,
    pub endpoint: Option<&'a str>,
    pub proxy_api_key: Option<&'a str>,
}

const MANAGED_ENV: &[&str] = &[
    "ANTHROPIC_BASE_URL",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_MODEL",
    "ANTHROPIC_DEFAULT_OPUS_MODEL",
    "ANTHROPIC_DEFAULT_SONNET_MODEL",
    "ANTHROPIC_DEFAULT_HAIKU_MODEL",
    "ANTHROPIC_SMALL_FAST_MODEL",
    "CLAUDE_CODE_SUBAGENT_MODEL",
];

/// Pure script builder — unit-tested without spawning anything.
pub fn build_launch_script(folder: &Path, opts: &LaunchOptions) -> Result<Vec<String>, String> {
    let mode = match opts.mode {
        "proxy" | "normal" => opts.mode,
        other => return Err(format!("geçersiz mod: {other}")),
    };

    let mut lines: Vec<String> = vec![
        "@echo off".into(),
        "title ZCode Manager ^| Claude Code".into(),
    ];

    if mode == "proxy" {
        let endpoint = opts
            .endpoint
            .filter(|e| e.starts_with("http://127.0.0.1:"))
            .ok_or_else(|| "Proxy endpoint hazır değil".to_string())?;
        // Claude Code requires a token when a base URL is set; when the proxy
        // enforces a key we use it, otherwise any non-empty value works.
        let token = opts
            .proxy_api_key
            .filter(|k| !k.trim().is_empty())
            .unwrap_or("zcode-manager-local");
        lines.push(format!("set \"ANTHROPIC_BASE_URL={endpoint}\""));
        lines.push(format!("set \"ANTHROPIC_AUTH_TOKEN={token}\""));
        lines.push("set \"ANTHROPIC_API_KEY=\"".into());
        if let Some(model) = opts.model.filter(|m| valid_model_name(m)) {
            for var in [
                "ANTHROPIC_MODEL",
                "ANTHROPIC_DEFAULT_OPUS_MODEL",
                "ANTHROPIC_DEFAULT_SONNET_MODEL",
                "ANTHROPIC_DEFAULT_HAIKU_MODEL",
                "ANTHROPIC_SMALL_FAST_MODEL",
                "CLAUDE_CODE_SUBAGENT_MODEL",
            ] {
                lines.push(format!("set \"{var}={model}\""));
            }
        }
    } else {
        // Normal mode: clear exactly the variables the manager itself sets —
        // the user's own environment is otherwise untouched.
        for var in MANAGED_ENV {
            lines.push(format!("set \"{var}=\""));
        }
    }

    lines.push(format!("cd /d \"{}\"", folder.display()));
    lines.push("claude".into());
    lines.push("if errorlevel 1 pause".into());
    Ok(lines)
}

pub fn launch(launches_dir: &Path, opts: LaunchOptions) -> Result<(), String> {
    let folder = opts
        .folder
        .map(PathBuf::from)
        .ok_or_else(|| "Proje klasörü seçilmedi".to_string())?;
    if !folder.is_dir() {
        return Err(format!("Klasör bulunamadı: {}", folder.display()));
    }

    let lines = build_launch_script(&folder, &opts)?;

    std::fs::create_dir_all(launches_dir).map_err(|e| format!("klasör oluşturulamadı: {e}"))?;
    let script = launches_dir.join(format!(
        "claude-{}.cmd",
        chrono::Utc::now().timestamp_millis()
    ));
    std::fs::write(&script, lines.join("\r\n"))
        .map_err(|e| format!("başlatma betiği yazılamadı: {e}"))?;

    // Prefer Windows Terminal, fall back to a classic console window.
    let mut wt = Command::new("wt.exe");
    wt.args(["new-tab", "--title", "Claude Code"])
        .arg("-d")
        .arg(&folder)
        .args(["cmd", "/c"])
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide(&mut wt);
    if wt.spawn().is_ok() {
        return Ok(());
    }

    let mut cmd = Command::new("cmd.exe");
    cmd.args(["/d", "/c", "start", "ZCode Manager - Claude Code", "cmd", "/c"])
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide(&mut cmd);
    cmd.spawn()
        .map_err(|e| format!("Claude Code penceresi açılamadı: {e}"))?;
    Ok(())
}

/// Run `<exe> --cli version` (used by the About panel).
pub fn proxy_exe_version(exe: &Path) -> Option<String> {
    let mut cmd = Command::new(exe);
    cmd.args(["--cli", "version"]);
    let out = output_hidden(&mut cmd)?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_script_proxy_mode_sets_env() {
        let opts = LaunchOptions {
            mode: "proxy",
            model: Some("glm-5.3"),
            folder: Some(r"C:\Projeler\demo"),
            endpoint: Some("http://127.0.0.1:8317"),
            proxy_api_key: Some("key12345"),
        };
        let script = build_launch_script(Path::new(r"C:\Projeler\demo"), &opts).unwrap();
        let text = script.join("\n");
        assert!(text.contains("set \"ANTHROPIC_BASE_URL=http://127.0.0.1:8317\""));
        assert!(text.contains("set \"ANTHROPIC_AUTH_TOKEN=key12345\""));
        assert!(text.contains("set \"ANTHROPIC_API_KEY=\""));
        assert!(text.contains("set \"ANTHROPIC_MODEL=glm-5.3\""));
        assert!(text.contains("set \"CLAUDE_CODE_SUBAGENT_MODEL=glm-5.3\""));
        assert!(text.contains("cd /d \"C:\\Projeler\\demo\""));
        assert!(text.contains("claude"));
        // No shell-metacharacter model is ever accepted into the script.
        assert!(!text.contains("glm&calc"));
    }

    #[test]
    fn launch_script_normal_mode_clears_managed_env() {
        let opts = LaunchOptions {
            mode: "normal",
            model: None,
            folder: Some(r"C:\x"),
            endpoint: None,
            proxy_api_key: None,
        };
        let script = build_launch_script(Path::new(r"C:\x"), &opts).unwrap();
        let text = script.join("\n");
        for var in MANAGED_ENV {
            assert!(text.contains(&format!("set \"{var}=\"")), "{var} cleared");
        }
        assert!(!text.contains("ANTHROPIC_BASE_URL=http"));
    }

    #[test]
    fn launch_script_rejects_foreign_endpoint() {
        let opts = LaunchOptions {
            mode: "proxy",
            model: None,
            folder: Some(r"C:\x"),
            endpoint: Some("http://evil.example.com"),
            proxy_api_key: None,
        };
        assert!(build_launch_script(Path::new(r"C:\x"), &opts).is_err());
    }

    #[test]
    fn model_name_validation() {
        assert!(valid_model_name("glm-5.3"));
        assert!(valid_model_name("glm-5.3[1m]"));
        assert!(valid_model_name("a"));
        assert!(!valid_model_name(""));
        assert!(!valid_model_name("-glm"));
        assert!(!valid_model_name("glm 5"));
        assert!(!valid_model_name("glm&calc"));
        assert!(!valid_model_name("glm%x%"));
        assert!(!valid_model_name("glm|x"));
        assert!(!valid_model_name("glm>x"));
        assert!(!valid_model_name(&"x".repeat(70)));
    }
}
