use std::path::PathBuf;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::text::{Line, Text};
use uuid::Uuid;

use crate::markdown;
use crate::models::{Agent, Config, Session};
use crate::storage::Storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search,
    NewSession,
    QuickSession,
    Help,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    List,
    Detail,
}

pub enum Action {
    Continue,
    Quit,
    RunAgent(Uuid, Agent),
    OpenExternal(PathBuf),
}

pub struct App {
    pub storage: Storage,
    pub config: Config,
    pub sessions: Vec<Session>,
    pub selected_index: usize,
    pub mode: Mode,
    pub focus: Focus,
    pub input: String,
    pub search_query: String,
    pub filtered_sessions: Vec<usize>,
    pub notes_content: String,
    pub notes_scroll: u16,
    pub error_message: Option<String>,
    pub show_preview: bool,
    pub rendered_notes: Option<Text<'static>>,
    rendered_notes_hash: u64,
    rendered_notes_width: u16,
}

impl App {
    pub fn new(storage: Storage, config: Config) -> Self {
        Self {
            storage,
            config,
            sessions: Vec::new(),
            selected_index: 0,
            mode: Mode::Normal,
            focus: Focus::List,
            input: String::new(),
            search_query: String::new(),
            filtered_sessions: Vec::new(),
            notes_content: String::new(),
            notes_scroll: 0,
            error_message: None,
            show_preview: true,
            rendered_notes: None,
            rendered_notes_hash: 0,
            rendered_notes_width: 0,
        }
    }

    pub fn refresh_sessions(&mut self) -> Result<()> {
        self.sessions = self.storage.list_sessions()?;
        self.apply_filter();
        self.load_selected_notes();
        Ok(())
    }

    fn apply_filter(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_sessions = (0..self.sessions.len()).collect();
        } else {
            let query = self.search_query.to_lowercase();
            self.filtered_sessions = self
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, s)| s.title.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect();
        }

        if self.selected_index >= self.filtered_sessions.len() {
            self.selected_index = self.filtered_sessions.len().saturating_sub(1);
        }
    }

    pub fn selected_session(&self) -> Option<&Session> {
        self.filtered_sessions
            .get(self.selected_index)
            .and_then(|&i| self.sessions.get(i))
    }

    fn load_selected_notes(&mut self) {
        if let Some(session) = self.selected_session() {
            match self.storage.read_notes(&session.id) {
                Ok(content) => self.notes_content = content,
                Err(_) => self.notes_content = String::new(),
            }
        } else {
            self.notes_content = String::new();
        }
        self.notes_scroll = 0;
        self.invalidate_rendered_notes();
    }

    pub fn select_session_by_prefix(&mut self, prefix: &str) {
        let prefix_lower = prefix.to_lowercase();
        for (i, idx) in self.filtered_sessions.iter().enumerate() {
            if let Some(session) = self.sessions.get(*idx) {
                if session.id.to_string().starts_with(&prefix_lower) {
                    self.selected_index = i;
                    self.load_selected_notes();
                    return;
                }
            }
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }

    pub fn ensure_rendered_notes(&mut self, width: u16) {
        if self.notes_content.is_empty() {
            self.rendered_notes = Some(Text::from(Line::from("")));
            self.rendered_notes_hash = 0;
            self.rendered_notes_width = width;
            return;
        }

        let width = width.max(20);
        let hash = calculate_hash(&self.notes_content);
        if self.rendered_notes.is_some()
            && self.rendered_notes_hash == hash
            && self.rendered_notes_width == width
        {
            return;
        }

        match markdown::render_markdown(&self.notes_content, width) {
            Ok(text) => {
                self.rendered_notes = Some(text);
            }
            Err(e) => {
                self.rendered_notes = Some(Text::from(Line::from(format!(
                    "glow error: {}",
                    e
                ))));
            }
        }

        self.rendered_notes_hash = hash;
        self.rendered_notes_width = width;
    }

    fn invalidate_rendered_notes(&mut self) {
        self.rendered_notes = None;
        self.rendered_notes_hash = 0;
        self.rendered_notes_width = 0;
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        self.error_message = None;

        match self.mode {
            Mode::Normal => self.handle_normal_key(key),
            Mode::Search => self.handle_search_key(key),
            Mode::NewSession => self.handle_new_session_key(key),
            Mode::QuickSession => self.handle_quick_session_key(key),
            Mode::Help => self.handle_help_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Char('q') => Action::Quit,
            KeyCode::Char('?') => {
                self.mode = Mode::Help;
                Action::Continue
            }
            KeyCode::Char('/') => {
                self.mode = Mode::Search;
                self.input.clear();
                Action::Continue
            }
            KeyCode::Char('n') => {
                self.mode = Mode::NewSession;
                self.input.clear();
                Action::Continue
            }
            KeyCode::Char('Q') => {
                self.mode = Mode::QuickSession;
                self.input.clear();
                Action::Continue
            }
            KeyCode::Char('p') => {
                self.show_preview = !self.show_preview;
                Action::Continue
            }
            KeyCode::Char('e') => {
                if let Some(session) = self.selected_session() {
                    let path = self.storage.session_notes_path(&session.id);
                    Action::OpenExternal(path)
                } else {
                    Action::Continue
                }
            }
            KeyCode::Char('r') => {
                if let Some(session) = self.selected_session() {
                    let id = session.id;
                    let agent = self.config.default_agent;
                    Action::RunAgent(id, agent)
                } else {
                    Action::Continue
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                    self.load_selected_notes();
                }
                Action::Continue
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.selected_index < self.filtered_sessions.len().saturating_sub(1) {
                    self.selected_index += 1;
                    self.load_selected_notes();
                }
                Action::Continue
            }
            KeyCode::Tab => {
                self.focus = match self.focus {
                    Focus::List => Focus::Detail,
                    Focus::Detail => Focus::List,
                };
                Action::Continue
            }
            KeyCode::PageUp => {
                self.notes_scroll = self.notes_scroll.saturating_sub(10);
                Action::Continue
            }
            KeyCode::PageDown => {
                self.notes_scroll = self.notes_scroll.saturating_add(10);
                Action::Continue
            }
            KeyCode::Esc => {
                if !self.search_query.is_empty() {
                    self.search_query.clear();
                    self.apply_filter();
                    self.load_selected_notes();
                }
                Action::Continue
            }
            _ => Action::Continue,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                self.search_query = self.input.clone();
                self.apply_filter();
                self.load_selected_notes();
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            _ => {}
        }
        Action::Continue
    }

    fn handle_new_session_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let session = Session::new(&self.input);
                    if let Err(e) = self.storage.create_session(&session, None) {
                        self.set_error(format!("Failed to create session: {}", e));
                    } else {
                        let _ = self.refresh_sessions();
                    }
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            _ => {}
        }
        Action::Continue
    }

    fn handle_quick_session_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let title: String = self.input.chars().take(50).collect();
                    let title = if self.input.len() > 50 {
                        format!("{}...", title)
                    } else {
                        title
                    };
                    let session = Session::new(title);
                    if let Err(e) = self.storage.create_session(&session, Some(&self.input)) {
                        self.set_error(format!("Failed to create session: {}", e));
                    } else {
                        let _ = self.refresh_sessions();
                    }
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => {
                self.mode = Mode::Normal;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => {
                self.input.push(c);
            }
            _ => {}
        }
        Action::Continue
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        Action::Continue
    }
}

fn calculate_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}
