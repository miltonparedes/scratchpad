use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::models::{Config, Session};

pub struct Storage {
    config: Config,
}

impl Storage {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn workspace_path(&self) -> &Path {
        Path::new(&self.config.workspace_path)
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.workspace_path().join("sessions")
    }

    pub fn session_dir(&self, session_id: &uuid::Uuid) -> PathBuf {
        self.sessions_dir().join(session_id.to_string())
    }

    pub fn session_json_path(&self, session_id: &uuid::Uuid) -> PathBuf {
        self.session_dir(session_id).join("session.json")
    }

    pub fn session_notes_path(&self, session_id: &uuid::Uuid) -> PathBuf {
        self.session_dir(session_id).join("notes.md")
    }

    pub fn session_files_dir(&self, session_id: &uuid::Uuid) -> PathBuf {
        self.session_dir(session_id).join("files")
    }

    pub fn ensure_workspace(&self) -> Result<()> {
        fs::create_dir_all(self.sessions_dir())
            .context("Failed to create sessions directory")?;
        Ok(())
    }

    pub fn create_session(&self, session: &Session, initial_note: Option<&str>) -> Result<()> {
        let session_dir = self.session_dir(&session.id);
        fs::create_dir_all(&session_dir)
            .context("Failed to create session directory")?;
        fs::create_dir_all(self.session_files_dir(&session.id))
            .context("Failed to create session files directory")?;

        self.save_session(session)?;

        let notes_content = initial_note.unwrap_or("");
        fs::write(self.session_notes_path(&session.id), notes_content)
            .context("Failed to create notes.md")?;

        Ok(())
    }

    pub fn save_session(&self, session: &Session) -> Result<()> {
        let json = serde_json::to_string_pretty(session)
            .context("Failed to serialize session")?;
        fs::write(self.session_json_path(&session.id), json)
            .context("Failed to write session.json")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn load_session(&self, session_id: &uuid::Uuid) -> Result<Session> {
        let json = fs::read_to_string(self.session_json_path(session_id))
            .context("Failed to read session.json")?;
        let session: Session = serde_json::from_str(&json)
            .context("Failed to parse session.json")?;
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let sessions_dir = self.sessions_dir();
        if !sessions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for entry in fs::read_dir(&sessions_dir).context("Failed to read sessions directory")? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let session_json = path.join("session.json");
                if session_json.exists() {
                    match fs::read_to_string(&session_json) {
                        Ok(json) => match serde_json::from_str::<Session>(&json) {
                            Ok(session) => sessions.push(session),
                            Err(e) => eprintln!("Warning: Failed to parse {:?}: {}", session_json, e),
                        },
                        Err(e) => eprintln!("Warning: Failed to read {:?}: {}", session_json, e),
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn read_notes(&self, session_id: &uuid::Uuid) -> Result<String> {
        fs::read_to_string(self.session_notes_path(session_id))
            .context("Failed to read notes.md")
    }

    #[allow(dead_code)]
    pub fn write_notes(&self, session_id: &uuid::Uuid, content: &str) -> Result<()> {
        fs::write(self.session_notes_path(session_id), content)
            .context("Failed to write notes.md")
    }

    #[allow(dead_code)]
    pub fn delete_session(&self, session_id: &uuid::Uuid) -> Result<()> {
        let session_dir = self.session_dir(session_id);
        if session_dir.exists() {
            fs::remove_dir_all(&session_dir)
                .context("Failed to delete session directory")?;
        }
        Ok(())
    }

    pub fn find_session_by_prefix(&self, prefix: &str) -> Result<Option<Session>> {
        let sessions = self.list_sessions()?;
        let prefix_lower = prefix.to_lowercase();

        for session in sessions {
            if session.id.to_string().starts_with(&prefix_lower) {
                return Ok(Some(session));
            }
        }
        Ok(None)
    }
}

pub fn config_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "agentpad")
        .map(|d| d.config_dir().join("config.json"))
        .unwrap_or_else(|| PathBuf::from("~/.config/agentpad/config.json"))
}

pub fn load_config() -> Result<Config> {
    let path = config_path();
    if path.exists() {
        let json = fs::read_to_string(&path)
            .context("Failed to read config file")?;
        let config: Config = serde_json::from_str(&json)
            .context("Failed to parse config file")?;
        Ok(config)
    } else {
        Ok(Config::default())
    }
}

#[allow(dead_code)]
pub fn save_config(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .context("Failed to create config directory")?;
    }
    let json = serde_json::to_string_pretty(config)
        .context("Failed to serialize config")?;
    fs::write(&path, json)
        .context("Failed to write config file")?;
    Ok(())
}
