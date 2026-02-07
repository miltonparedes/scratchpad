use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A session is identified by its slug (folder name).
/// Timestamps are derived from filesystem metadata.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Session {
    /// Folder name, e.g., "quantum-reactor"
    pub slug: String,
    /// From filesystem creation time (or mtime as fallback)
    pub created_at: DateTime<Utc>,
    /// From filesystem mtime
    pub updated_at: DateTime<Utc>,
}

impl Session {
    pub fn new(slug: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            slug: slug.into(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Display the slug as a readable title (dashes become spaces, title case)
    pub fn display_title(&self) -> String {
        self.slug
            .split('-')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().chain(chars).collect(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Context determines where sessions are stored
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// User-global scratchpad at ~/scratchpad
    User,
    /// Project-local scratchpad at .scratchpad/
    Project(PathBuf),
}

impl Context {
    pub fn display_name(&self) -> String {
        match self {
            Context::User => "User".to_string(),
            Context::Project(path) => path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Project".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    #[default]
    Claude,
    Codex,
}

impl Agent {
    pub fn command(&self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Agent::Claude => write!(f, "claude"),
            Agent::Codex => write!(f, "codex"),
        }
    }
}

impl std::str::FromStr for Agent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(Agent::Claude),
            "codex" => Ok(Agent::Codex),
            _ => Err(format!("Unknown agent: {s}")),
        }
    }
}

/// A single entry in a file tree (pre-order traversal, flat list)
#[derive(Debug, Clone)]
pub struct FileTreeEntry {
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
    pub is_last: bool,
    pub is_entry_point: bool,
    pub ancestor_is_last: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub url: String,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Config schema version for forward compatibility
    #[serde(default)]
    pub config_version: u32,

    #[serde(default = "default_workspace_path")]
    pub workspace_path: String,

    #[serde(default)]
    pub default_agent: Agent,

    /// Editor for `e` key / editing (e.g., "nvim", "code")
    #[serde(default)]
    pub editor: Option<String>,

    /// Viewer for `v` key / viewing (uses system default if None)
    #[serde(default)]
    pub viewer: Option<String>,

    /// Name generator: "auto", "claude", "codex", or "static"
    #[serde(default = "default_name_generator")]
    pub name_generator: String,

    /// Optional sync server configuration
    #[serde(default)]
    pub server: Option<ServerConfig>,
}

pub fn default_workspace_path() -> String {
    dirs_home().join("scratchpad").to_string_lossy().to_string()
}

fn default_name_generator() -> String {
    "auto".to_string()
}

fn dirs_home() -> std::path::PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: crate::config::CURRENT_CONFIG_VERSION,
            workspace_path: default_workspace_path(),
            default_agent: Agent::default(),
            editor: None,
            viewer: None,
            name_generator: default_name_generator(),
            server: None,
        }
    }
}
