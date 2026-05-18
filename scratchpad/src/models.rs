use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct Session {
    pub slug: String,
    pub project: String,
    pub path: PathBuf,
    pub status: SessionStatus,
    pub tags: Vec<String>,
    pub revision: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[default]
    Active,
    Archived,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionStatus::Active => "active",
            SessionStatus::Archived => "archived",
        }
    }
}

impl std::str::FromStr for SessionStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "active" => Ok(SessionStatus::Active),
            "archived" => Ok(SessionStatus::Archived),
            other => Err(format!("Unknown status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    #[default]
    Claude,
    Codex,
    Custom,
}

impl Agent {
    pub fn command(&self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
            Agent::Custom => "",
        }
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Agent::Claude => write!(f, "claude"),
            Agent::Codex => write!(f, "codex"),
            Agent::Custom => write!(f, "custom"),
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostsConfig {
    #[serde(default)]
    pub short_form: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAlias {
    pub name: String,
    #[serde(default)]
    pub repos: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentBinding {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub config_version: u32,

    #[serde(default = "default_workspace_path")]
    pub workspace_path: String,

    #[serde(default)]
    pub default_agent: Agent,

    #[serde(default)]
    pub editor: Option<String>,

    #[serde(default)]
    pub viewer: Option<String>,

    #[serde(default = "default_name_generator")]
    pub name_generator: String,

    #[serde(default)]
    pub server: Option<ServerConfig>,

    #[serde(default)]
    pub hosts: Option<HostsConfig>,

    #[serde(default, rename = "projects")]
    pub projects: Vec<ProjectAlias>,

    #[serde(default)]
    pub agents: std::collections::BTreeMap<String, AgentBinding>,
}

pub fn default_workspace_path() -> String {
    dirs_home()
        .join(".scratchpad")
        .to_string_lossy()
        .to_string()
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
            hosts: None,
            projects: Vec::new(),
            agents: std::collections::BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionMeta {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub status: SessionStatus,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}
