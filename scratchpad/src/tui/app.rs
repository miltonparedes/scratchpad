use std::path::PathBuf;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::text::{Line, Text};

use crate::markdown;
use crate::models::{Agent, Config, FileTreeEntry, Session};
use crate::names::{generate_session_name, slugify_or_generate};
use crate::project::{Project, ProjectSource};
use crate::storage::{Storage, build_file_tree, list_session_files};

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
    RunAgent(String, Agent),
    ViewExternal(PathBuf),
    EditExternal(PathBuf),
    OpenFolder(PathBuf),
}

pub struct App {
    pub storage: Storage,
    pub config: Config,
    pub project: Project,
    pub available_projects: Vec<String>,
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
    pub session_files: Vec<PathBuf>,
    pub file_tree: Vec<FileTreeEntry>,
}

impl App {
    pub fn new(storage: Storage, config: Config, available_projects: Vec<String>) -> Self {
        let project = storage.project().clone();
        Self {
            storage,
            config,
            project,
            available_projects,
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
            session_files: Vec::new(),
            file_tree: Vec::new(),
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
                .filter(|(_, s)| {
                    s.slug.to_lowercase().contains(&query)
                        || s.display_title().to_lowercase().contains(&query)
                        || s.tags.iter().any(|t| t.to_lowercase().contains(&query))
                })
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
        self.session_files.clear();
        self.file_tree.clear();

        if let Some(session) = self.selected_session() {
            let slug = session.slug.clone();
            let session_dir = self.storage.session_dir(&slug);
            let entry_point = self.storage.find_entry_point(&slug);
            self.file_tree = build_file_tree(&session_dir, entry_point.as_deref(), 3);

            if let Some(ref ep) = entry_point {
                match std::fs::read_to_string(ep) {
                    Ok(content) => self.notes_content = content,
                    Err(_) => self.notes_content = String::new(),
                }
            } else {
                self.notes_content = String::new();
                self.session_files = list_session_files(&session_dir);
                self.session_files.sort();
            }
        } else {
            self.notes_content = String::new();
        }
        self.notes_scroll = 0;
        self.invalidate_rendered_notes();
    }

    pub fn select_session_by_name(&mut self, name: &str) {
        let name_lower = name.to_lowercase();
        for (i, idx) in self.filtered_sessions.iter().enumerate() {
            if let Some(session) = self.sessions.get(*idx)
                && (session.slug.to_lowercase() == name_lower
                    || session.slug.to_lowercase().starts_with(&name_lower))
            {
                self.selected_index = i;
                self.load_selected_notes();
                return;
            }
        }
    }

    pub fn set_error(&mut self, msg: String) {
        self.error_message = Some(msg);
    }

    pub fn ensure_rendered_notes(&mut self, width: u16) {
        if !self.session_files.is_empty() {
            self.rendered_notes = None;
            return;
        }
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
            Ok(text) => self.rendered_notes = Some(text),
            Err(e) => {
                self.rendered_notes = Some(Text::from(Line::from(format!("glow error: {e}"))));
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
            KeyCode::Char('g') => {
                self.cycle_project();
                Action::Continue
            }
            KeyCode::Char('e') => {
                if let Some(session) = self.selected_session() {
                    let slug = session.slug.clone();
                    if let Some(entry_point) = self.storage.find_entry_point(&slug) {
                        Action::EditExternal(entry_point)
                    } else {
                        let notes_path = self.storage.session_dir(&slug).join("notes.md");
                        if !notes_path.exists() {
                            let _ = std::fs::write(&notes_path, "");
                        }
                        Action::EditExternal(notes_path)
                    }
                } else {
                    Action::Continue
                }
            }
            KeyCode::Char('v') => {
                if let Some(session) = self.selected_session() {
                    let slug = session.slug.clone();
                    if let Some(entry_point) = self.storage.find_entry_point(&slug) {
                        Action::ViewExternal(entry_point)
                    } else {
                        let session_dir = self.storage.session_dir(&slug);
                        Action::OpenFolder(session_dir)
                    }
                } else {
                    Action::Continue
                }
            }
            KeyCode::Char('o') => {
                if let Some(session) = self.selected_session() {
                    let session_dir = self.storage.session_dir(&session.slug);
                    Action::OpenFolder(session_dir)
                } else {
                    Action::Continue
                }
            }
            KeyCode::Char('r') => {
                if let Some(session) = self.selected_session() {
                    let slug = session.slug.clone();
                    let agent = self.config.default_agent;
                    Action::RunAgent(slug, agent)
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

    fn cycle_project(&mut self) {
        if self.available_projects.is_empty() {
            return;
        }
        let current_idx = self
            .available_projects
            .iter()
            .position(|n| n == &self.project.slug)
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.available_projects.len();
        let next_name = self.available_projects[next_idx].clone();
        let next_project = Project {
            slug: next_name.clone(),
            source: ProjectSource::Alias {
                alias_name: next_name.clone(),
                repo: String::new(),
            },
        };
        self.project = next_project.clone();
        self.storage.switch_project(next_project);
        let _ = self.refresh_sessions();
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                self.search_query = self.input.clone();
                self.apply_filter();
                self.load_selected_notes();
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Action::Continue
    }

    fn handle_new_session_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                let existing = self.storage.existing_slugs().unwrap_or_default();
                let slug = if self.input.is_empty() {
                    generate_session_name(&existing, &self.config)
                } else {
                    slugify_or_generate(&self.input, &existing, &self.config)
                };
                if let Err(e) = self.storage.create_session(&slug, None, &[]) {
                    self.set_error(format!("Failed to create session: {e}"));
                } else {
                    let _ = self.refresh_sessions();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Action::Continue
    }

    fn handle_quick_session_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => {
                if !self.input.is_empty() {
                    let existing = self.storage.existing_slugs().unwrap_or_default();
                    let slug = generate_session_name(&existing, &self.config);
                    if let Err(e) = self.storage.create_session(&slug, Some(&self.input), &[]) {
                        self.set_error(format!("Failed to create session: {e}"));
                    } else {
                        let _ = self.refresh_sessions();
                    }
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Action::Continue
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => self.mode = Mode::Normal,
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
