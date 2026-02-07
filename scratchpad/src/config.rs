use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};

use crate::cli::ConfigAction;
use crate::models::{Config, default_workspace_path};
use crate::open::open_with_editor;

pub const CURRENT_CONFIG_VERSION: u32 = 1;

pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "scratchpad")
        .map(|d| d.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("~/.config/scratchpad/config.toml"))
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(&path).context("Failed to read config file")?;
    let config: Config = toml::from_str(&content).context("Failed to parse config file")?;

    if config.config_version < CURRENT_CONFIG_VERSION {
        warn_deprecated(&config);
    }

    Ok(config)
}

fn warn_deprecated(config: &Config) {
    if config.config_version == 0 {
        eprintln!(
            "Note: your config has no version field. Run `sp config init --force` to update to the latest format."
        );
    }
}

fn config_template() -> String {
    let default_ws = default_workspace_path();
    format!(
        r#"# Scratchpad configuration
config_version = {CURRENT_CONFIG_VERSION}

# Where user-context sessions are stored (absolute path)
# workspace_path = "{default_ws}"

# Default agent to launch: "claude" or "codex"
# default_agent = "claude"

# Editor command for `e` key / `sp edit` (falls back to $EDITOR, $VISUAL, vi)
# Supports arguments: "code --wait", "zed --wait"
# editor = "nvim"

# Viewer command for `v` key / `sp view` (falls back to system open)
# viewer = "bat --paging=always"

# Name generation strategy: "auto", "claude", "codex", or "static"
# name_generator = "auto"

# Sync server (optional)
# [server]
# url = "http://localhost:3000"
# token = "your-token"
"#
    )
}

/// Write content to a file atomically with restrictive permissions (0o600 on Unix).
fn save_config_atomic(path: &PathBuf, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }

    let tmp_path = path.with_extension("toml.tmp");

    // Set restrictive permissions before writing content (Unix only)
    #[cfg(unix)]
    {
        // Create/truncate the file first so we can set permissions
        fs::write(&tmp_path, "").context("Failed to create temp config file")?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))
            .context("Failed to set config file permissions")?;
    }

    fs::write(&tmp_path, content).context("Failed to write temp config file")?;
    fs::rename(&tmp_path, path).context("Failed to finalize config file")?;

    Ok(())
}

pub fn handle_config(action: ConfigAction, config: &Config) -> Result<()> {
    match action {
        ConfigAction::Init { force } => {
            let path = config_path();
            if path.exists() && !force {
                anyhow::bail!(
                    "Config file already exists at {}\nUse --force to overwrite.",
                    path.display()
                );
            }
            let content = config_template();
            save_config_atomic(&path, &content)?;
            println!("Created config at {}", path.display());
        }
        ConfigAction::Path => {
            print!("{}", config_path().display());
        }
        ConfigAction::Show => {
            let toml_str = toml::to_string_pretty(config).context("Failed to serialize config")?;
            print!("{toml_str}");
        }
        ConfigAction::Edit => {
            let path = config_path();
            if !path.exists() {
                let content = config_template();
                save_config_atomic(&path, &content)?;
                eprintln!("Created config at {}", path.display());
            }
            open_with_editor(&path, config.editor.as_deref())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_template_is_valid_toml() {
        let template = config_template();
        // Remove comment lines and parse — should be valid TOML
        let uncommented: String = template
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        let result: Result<Config, _> = toml::from_str(&uncommented);
        assert!(result.is_ok(), "Template is not valid TOML: {result:?}");
    }

    #[test]
    fn config_without_version_defaults_to_zero() {
        let toml_str = r#"
            workspace_path = "/tmp/test"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.config_version, 0);
    }

    #[test]
    fn config_with_current_version_loads() {
        let toml_str = format!(
            r#"
            config_version = {CURRENT_CONFIG_VERSION}
            workspace_path = "/tmp/test"
        "#
        );
        let config: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.config_version, CURRENT_CONFIG_VERSION);
    }

    #[test]
    #[cfg(unix)]
    fn atomic_save_sets_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        save_config_atomic(&path, "test = true\n").unwrap();

        let meta = fs::metadata(&path).unwrap();
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "Expected 0o600, got 0o{mode:o}");
    }
}
