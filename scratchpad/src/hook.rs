use std::io::Read;
use std::path::Path;

use anyhow::{Context as _, Result};

pub fn handle(name: &str) -> Result<()> {
    match name {
        "check-write" => check_write(),
        _ => {
            eprintln!("Unknown hook: {name}");
            std::process::exit(1);
        }
    }
}

fn check_write() -> Result<()> {
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .context("Failed to read stdin")?;

    let json: serde_json::Value =
        serde_json::from_str(&input).context("Failed to parse JSON input")?;

    let file_path = json
        .get("tool_input")
        .and_then(|ti| ti.get("file_path"))
        .and_then(|fp| fp.as_str())
        .unwrap_or("");

    if file_path.is_empty() {
        return Ok(());
    }
    if !file_path.ends_with(".md") {
        return Ok(());
    }
    let path = Path::new(file_path);
    if is_inside_scratchpad(path) {
        return Ok(());
    }
    if is_known_project_file(path) {
        return Ok(());
    }

    let response = serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "ask",
            "permissionDecisionReason":
                "Consider using a scratchpad session instead of a loose .md file. \
                 Run `sp new` to create a session, then `sp write <session>/<file>` to write there."
        }
    });
    println!("{}", serde_json::to_string(&response)?);
    Ok(())
}

fn is_inside_scratchpad(path: &Path) -> bool {
    let path_str = path.to_string_lossy();
    if let Some(home) = dirs_home_str() {
        let workspace = format!("{home}/.scratchpad/");
        if path_str.starts_with(&workspace) {
            return true;
        }
    }
    false
}

fn is_known_project_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    matches!(
        name.as_str(),
        "README.md"
            | "readme.md"
            | "CLAUDE.md"
            | "CHANGELOG.md"
            | "CONTRIBUTING.md"
            | "LICENSE.md"
            | "SECURITY.md"
            | "CODE_OF_CONDUCT.md"
            | "AGENTS.md"
            | "AGENTS.override.md"
            | "SKILL.md"
    )
}

fn dirs_home_str() -> Option<String> {
    directories::BaseDirs::new().map(|d| d.home_dir().to_string_lossy().to_string())
}
