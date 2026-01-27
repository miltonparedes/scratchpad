use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Session {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            created_at: now,
            updated_at: now,
            tags: Vec::new(),
        }
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    #[allow(dead_code)]
    pub fn touch(&mut self) {
        self.updated_at = Utc::now();
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
            _ => Err(format!("Unknown agent: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_workspace_path")]
    pub workspace_path: String,
    #[serde(default)]
    pub default_agent: Agent,
    pub editor: Option<String>,
    pub viewer: Option<String>,
}

fn default_workspace_path() -> String {
    dirs_home().join("agentpad").to_string_lossy().to_string()
}

fn dirs_home() -> std::path::PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspace_path: default_workspace_path(),
            default_agent: Agent::default(),
            editor: None,
            viewer: None,
        }
    }
}
