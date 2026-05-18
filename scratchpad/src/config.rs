use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};

use crate::cli::ConfigAction;
use crate::models::{Config, ProjectAlias, default_workspace_path};
use crate::open::open_with_editor;

pub const CURRENT_CONFIG_VERSION: u32 = 2;

pub fn config_path() -> PathBuf {
    workspace_default_path().join("config.toml")
}

fn workspace_default_path() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".scratchpad"))
        .unwrap_or_else(|| PathBuf::from(".scratchpad"))
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
            "Note: your config has no version field. Run `sp config init --force` to update."
        );
    }
}

fn config_template() -> String {
    let default_ws = default_workspace_path();
    format!(
        r#"# Scratchpad configuration
config_version = {CURRENT_CONFIG_VERSION}

# Workspace root (sessions live under this directory)
# workspace_path = "{default_ws}"

# Default agent: "claude" or "codex"
# default_agent = "claude"

# Editor / viewer overrides (fall back to $EDITOR / system default)
# editor = "nvim"
# viewer = "bat --paging=always"

# Session name generation: "auto", "claude", "codex", or "static"
# name_generator = "auto"

# Git hosts that produce the short slug "owner/repo".
# Hosts not listed here produce the long slug "host/owner/repo".
# [hosts]
# short_form = ["github.com", "gitlab.com", "bitbucket.org", "codeberg.org"]

# Project aliases (groups multiple repos under one workspace folder)
# [[projects]]
# name = "payments"
# repos = ["acme/payments-api", "acme/payments-worker"]

# Custom agents for `sp run --agent <name>`
# [agents.gemini]
# command = "gemini"
# args = []

# Sync server (optional, not yet implemented)
# [server]
# url = "http://localhost:3000"
# token = "your-token"
"#
    )
}

fn save_config_atomic(path: &PathBuf, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create config directory")?;
    }
    let tmp_path = path.with_extension("toml.tmp");
    #[cfg(unix)]
    {
        fs::write(&tmp_path, "").context("Failed to create temp config file")?;
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o600))
            .context("Failed to set config file permissions")?;
    }
    fs::write(&tmp_path, content).context("Failed to write temp config file")?;
    fs::rename(&tmp_path, path).context("Failed to finalize config file")?;
    Ok(())
}

pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path();
    let toml_str = toml::to_string_pretty(config).context("Failed to serialize config")?;
    save_config_atomic(&path, &toml_str)
}

pub fn upsert_project_alias(config: &mut Config, name: &str, repos: Vec<String>) {
    if let Some(existing) = config.projects.iter_mut().find(|p| p.name == name) {
        for repo in repos {
            if !existing.repos.iter().any(|r| r == &repo) {
                existing.repos.push(repo);
            }
        }
    } else {
        config.projects.push(ProjectAlias {
            name: name.to_string(),
            repos,
        });
    }
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
    fn template_is_valid_toml_after_uncomment() {
        let template = config_template();
        let uncommented: String = template
            .lines()
            .filter(|line| !line.starts_with('#'))
            .collect::<Vec<_>>()
            .join("\n");
        let result: Result<Config, _> = toml::from_str(&uncommented);
        assert!(result.is_ok(), "Template is not valid TOML: {result:?}");
    }

    #[test]
    fn upsert_alias_adds_repo() {
        let mut config = Config::default();
        upsert_project_alias(&mut config, "payments", vec!["acme/api".into()]);
        assert_eq!(config.projects.len(), 1);
        assert_eq!(config.projects[0].repos, vec!["acme/api"]);

        upsert_project_alias(&mut config, "payments", vec!["acme/worker".into()]);
        assert_eq!(config.projects[0].repos.len(), 2);
    }

    #[test]
    fn upsert_alias_dedups() {
        let mut config = Config::default();
        upsert_project_alias(&mut config, "payments", vec!["acme/api".into()]);
        upsert_project_alias(&mut config, "payments", vec!["acme/api".into()]);
        assert_eq!(config.projects[0].repos.len(), 1);
    }
}
