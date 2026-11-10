//! Comment-preserving YAML patcher for the proxy's `config.yaml`.
//!
//! The proxy's own config writer (`src/config/edit.ts`) patches YAML values in
//! place to keep user comments; we do the same from Rust. Only the small set
//! of keys the manager owns is supported: top-level scalars/lists and
//! two-level scalars under a fixed parent (`server.port`, `auth.proxyApiKey`).

#[derive(Debug, Clone, PartialEq)]
pub enum YamlValue {
    /// Pre-rendered scalar text, e.g. `127.0.0.1` (quoted by caller if needed).
    Scalar(String),
    /// Block list of scalar items.
    List(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct YamlUpdate {
    /// Key path, e.g. `["server", "port"]`. Max depth 2.
    pub path: Vec<&'static str>,
    pub value: YamlValue,
}

pub fn update(path: &'static str, value: impl Into<String>) -> YamlUpdate {
    YamlUpdate {
        path: vec![path],
        value: YamlValue::Scalar(value.into()),
    }
}

pub fn update_under(parent: &'static str, key: &'static str, value: impl Into<String>) -> YamlUpdate {
    YamlUpdate {
        path: vec![parent, key],
        value: YamlValue::Scalar(value.into()),
    }
}

pub fn update_list(path: &'static str, items: Vec<String>) -> YamlUpdate {
    YamlUpdate {
        path: vec![path],
        value: YamlValue::List(items),
    }
}

/// If the line opens a top-level mapping key (`key:` at column 0), return it.
fn top_key(line: &str) -> Option<String> {
    let line = line.trim_end();
    if line.starts_with('#') || line.starts_with(' ') || line.starts_with('-') || line.is_empty() {
        return None;
    }
    let key: String = line.chars().take_while(|c| *c != ':' && *c != ' ').collect();
    if key.is_empty() {
        return None;
    }
    let after = &line[key.len()..];
    if after.starts_with(':') && (after.len() == 1 || after[1..].starts_with(char::is_whitespace)) {
        Some(key)
    } else {
        None
    }
}

/// If the line is an indented child mapping key, return `(indent, key)`.
fn child_key(line: &str) -> Option<(usize, String)> {
    let line = line.trim_end();
    let body = line.trim_start_matches(' ');
    if body.starts_with('#') || body.is_empty() || body.starts_with('-') {
        return None;
    }
    let indent = line.len() - body.len();
    if indent == 0 {
        return None;
    }
    let key: String = body.chars().take_while(|c| *c != ':' && *c != ' ').collect();
    if key.is_empty() {
        return None;
    }
    let after = &body[key.len()..];
    if after.starts_with(':') && (after.len() == 1 || after[1..].starts_with(char::is_whitespace)) {
        Some((indent, key))
    } else {
        None
    }
}

fn is_list_item(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("- ") || t == "-"
}

/// End index (exclusive) of the top-level section starting at `start`.
fn section_end(lines: &[String], start: usize) -> usize {
    let mut i = start + 1;
    while i < lines.len() {
        if top_key(&lines[i]).is_some() {
            break;
        }
        i += 1;
    }
    i
}

pub fn patch_yaml(source: &str, updates: &[YamlUpdate]) -> String {
    let mut lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
    let trailing_newline = source.ends_with('\n');

    for u in updates {
        match u.path.len() {
            1 => patch_top(&mut lines, u.path[0], &u.value),
            2 => patch_nested(&mut lines, u.path[0], u.path[1], &u.value),
            _ => {}
        }
    }

    let mut out = lines.join("\n");
    if trailing_newline {
        out.push('\n');
    }
    out
}

fn render_scalar_line(indent: usize, key: &str, value: &str) -> String {
    format!("{}{}: {}", " ".repeat(indent), key, value)
}

fn render_list_block(indent: usize, key: &str, items: &[String]) -> Vec<String> {
    let mut out = vec![format!("{}{}:", " ".repeat(indent), key)];
    for item in items {
        out.push(format!("{}- {}", " ".repeat(indent + 2), item));
    }
    out
}

/// Ensure the file ends with exactly one blank line before appended keys.
fn push_blank_separator(lines: &mut Vec<String>) {
    if !lines.is_empty() && !lines.last().map(|l| l.trim().is_empty()).unwrap_or(true) {
        lines.push(String::new());
    }
}

fn patch_top(lines: &mut Vec<String>, key: &str, value: &YamlValue) {
    if let Some(idx) = lines.iter().position(|l| top_key(l).as_deref() == Some(key)) {
        match value {
            YamlValue::Scalar(v) => {
                lines[idx] = render_scalar_line(0, key, v);
            }
            YamlValue::List(items) => {
                // Replace the key line plus its block (list items / children).
                let mut end = idx + 1;
                while end < lines.len()
                    && (is_list_item(&lines[end]) || child_key(&lines[end]).is_some())
                {
                    end += 1;
                }
                lines.splice(idx..end, render_list_block(0, key, items));
            }
        }
        return;
    }
    // Missing: append at the end.
    push_blank_separator(lines);
    match value {
        YamlValue::Scalar(v) => lines.push(render_scalar_line(0, key, v)),
        YamlValue::List(items) => lines.extend(render_list_block(0, key, items)),
    }
}

fn patch_nested(lines: &mut Vec<String>, parent: &str, key: &str, value: &YamlValue) {
    let YamlValue::Scalar(v) = value else {
        return; // Nested lists are not needed by the manager.
    };

    if let Some(p) = lines.iter().position(|l| top_key(l).as_deref() == Some(parent)) {
        let end = section_end(lines, p);
        let found = (p + 1..end).find_map(|i| {
            child_key(&lines[i]).and_then(|(indent, k)| (k == key).then_some((i, indent)))
        });
        if let Some((idx, indent)) = found {
            lines[idx] = render_scalar_line(indent, key, v);
        } else {
            // Insert as the last entry of the section (skip trailing blanks).
            let mut insert_at = end;
            while insert_at > p + 1 && lines[insert_at - 1].trim().is_empty() {
                insert_at -= 1;
            }
            lines.insert(insert_at, render_scalar_line(2, key, v));
        }
        return;
    }

    // Parent section missing entirely: append it.
    push_blank_separator(lines);
    lines.push(format!("{parent}:"));
    lines.push(render_scalar_line(2, key, v));
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# leading comment
server:
  # port comment
  port: 8080
  host: "0.0.0.0"

auth:
  proxyApiKey: "old"

provider: zai
plan: coding-plan

models:
  - glm-5.3
  - glm-5.3-flash

# tail comment
logging:
  level: info
"#;

    #[test]
    fn patches_nested_scalar_preserving_comments() {
        let out = patch_yaml(
            SAMPLE,
            &[
                update_under("server", "port", "8317"),
                update_under("server", "host", "\"127.0.0.1\""),
                update_under("auth", "proxyApiKey", "\"newkey\""),
            ],
        );
        assert!(out.contains("# port comment"), "comment preserved");
        assert!(out.contains("  port: 8317"));
        assert!(out.contains("  host: \"127.0.0.1\""));
        assert!(out.contains("  proxyApiKey: \"newkey\""));
        assert!(out.contains("# tail comment"));
        assert!(out.contains("# leading comment"));
        let v: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(v["server"]["port"].as_u64(), Some(8317));
        assert_eq!(v["server"]["host"].as_str(), Some("127.0.0.1"));
        assert_eq!(v["auth"]["proxyApiKey"].as_str(), Some("newkey"));
    }

    #[test]
    fn patches_top_level_scalar_and_list() {
        let out = patch_yaml(
            SAMPLE,
            &[
                update("plan", "start-plan"),
                update_list("models", vec!["glm-5.3".into(), "glm-5.2".into(), "custom-x".into()]),
            ],
        );
        assert!(out.contains("plan: start-plan"));
        assert!(out.contains("- custom-x"));
        assert!(!out.contains("- glm-5.3-flash"));
        let v: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(v["plan"].as_str(), Some("start-plan"));
        assert_eq!(v["models"].as_sequence().unwrap().len(), 3);
        assert_eq!(v["server"]["port"].as_u64(), Some(8080));
    }

    #[test]
    fn appends_missing_keys() {
        let base = "server:\n  port: 8080\n";
        let out = patch_yaml(
            base,
            &[
                update("defaultModel", "glm-5.2"),
                update_list("planPriority", vec!["coding-plan".into(), "start-plan".into()]),
                update("planAutoSwitch", "true"),
                update_under("auth", "proxyApiKey", "\"k\""),
            ],
        );
        let v: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(v["defaultModel"].as_str(), Some("glm-5.2"));
        assert_eq!(v["planAutoSwitch"].as_bool(), Some(true));
        assert_eq!(v["auth"]["proxyApiKey"].as_str(), Some("k"));
        let prio = v["planPriority"].as_sequence().unwrap();
        assert_eq!(prio[0].as_str(), Some("coding-plan"));
        assert_eq!(prio[1].as_str(), Some("start-plan"));
    }

    #[test]
    fn replaces_inline_list_with_block() {
        let base = "models: [\"glm-5.3\"]\n";
        let out = patch_yaml(base, &[update_list("models", vec!["glm-5.2".into()])]);
        let v: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(v["models"][0].as_str(), Some("glm-5.2"));
    }

    #[test]
    fn does_not_touch_commented_lookalikes() {
        let base = "# plan: start-plan\nplan: coding-plan\n";
        let out = patch_yaml(base, &[update("plan", "start-plan")]);
        assert!(out.contains("# plan: start-plan"));
        let v: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(v["plan"].as_str(), Some("start-plan"));
    }

    #[test]
    fn quoted_values_round_trip() {
        let out = patch_yaml(SAMPLE, &[update_under("auth", "proxyApiKey", "\"a b c\"")]);
        let v: serde_yaml::Value = serde_yaml::from_str(&out).unwrap();
        assert_eq!(v["auth"]["proxyApiKey"].as_str(), Some("a b c"));
    }
}
