use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use chrono::{TimeZone, Utc};

use crate::models::{Config, Context, FileTreeEntry, Session};

pub struct Storage {
    config: Config,
    context: Context,
}

impl Storage {
    pub fn new(config: Config, context: Context) -> Self {
        Self { config, context }
    }

    pub fn workspace_path(&self) -> PathBuf {
        match &self.context {
            Context::User => PathBuf::from(&self.config.workspace_path),
            Context::Project(path) => path.clone(),
        }
    }

    #[allow(dead_code)]
    pub fn context(&self) -> &Context {
        &self.context
    }

    pub fn switch_context(&mut self, context: Context) {
        self.context = context;
    }

    /// Get the directory for a session by slug
    pub fn session_dir(&self, slug: &str) -> PathBuf {
        self.workspace_path().join(slug)
    }

    pub fn ensure_workspace(&self) -> Result<()> {
        fs::create_dir_all(self.workspace_path())
            .context("Failed to create workspace directory")?;
        Ok(())
    }

    pub fn create_session(&self, session: &Session, initial_note: Option<&str>) -> Result<()> {
        if session.slug.is_empty() {
            anyhow::bail!("Session slug cannot be empty");
        }

        let session_dir = self.session_dir(&session.slug);

        // Prevent overwriting existing sessions
        if session_dir.exists() {
            anyhow::bail!("Session '{}' already exists", session.slug);
        }

        fs::create_dir_all(&session_dir).context("Failed to create session directory")?;

        let notes_content = initial_note.unwrap_or("");
        fs::write(session_dir.join("notes.md"), notes_content)
            .context("Failed to create notes.md")?;

        Ok(())
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let workspace = self.workspace_path();
        if !workspace.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for entry in fs::read_dir(&workspace).context("Failed to read workspace directory")? {
            let entry = entry?;
            let path = entry.path();

            // Only include directories (not files like config)
            if !path.is_dir() {
                continue;
            }

            // Skip hidden directories
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }

            let slug = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            if slug.is_empty() {
                continue;
            }

            // Get timestamps from filesystem metadata
            let metadata = fs::metadata(&path).ok();
            let (created_at, updated_at) = if let Some(meta) = metadata {
                let mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).unwrap())
                    })
                    .unwrap_or_else(Utc::now);

                // Try to get creation time, fall back to mtime
                let ctime = meta
                    .created()
                    .ok()
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).unwrap())
                    })
                    .unwrap_or(mtime);

                (ctime, mtime)
            } else {
                let now = Utc::now();
                (now, now)
            };

            sessions.push(Session {
                slug,
                created_at,
                updated_at,
            });
        }

        // Sort by updated_at descending (most recent first)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Find the entry point file for a session (main.md, notes.md, readme.md, or first .md)
    pub fn find_entry_point(&self, slug: &str) -> Option<PathBuf> {
        let session_dir = self.session_dir(slug);
        find_entry_point_in_dir(&session_dir)
    }

    /// Read the entry point file content
    pub fn read_notes(&self, slug: &str) -> Result<String> {
        if let Some(entry_point) = self.find_entry_point(slug) {
            fs::read_to_string(&entry_point)
                .with_context(|| format!("Failed to read {}", entry_point.display()))
        } else {
            Ok(String::new())
        }
    }

    pub fn write_notes(&self, slug: &str, content: &str) -> Result<()> {
        let notes_path = self.session_dir(slug).join("notes.md");
        fs::write(&notes_path, content).context("Failed to write notes.md")
    }

    pub fn delete_session(&self, slug: &str) -> Result<()> {
        let session_dir = self.session_dir(slug);
        if session_dir.exists() {
            fs::remove_dir_all(&session_dir).context("Failed to delete session directory")?;
        }
        Ok(())
    }

    /// Find a session by exact name or prefix match
    pub fn find_session_by_name(&self, name: &str) -> Result<Option<Session>> {
        let sessions = self.list_sessions()?;
        let name_lower = name.to_lowercase();

        // First try exact match
        for session in &sessions {
            if session.slug.to_lowercase() == name_lower {
                return Ok(Some(session.clone()));
            }
        }

        // Then try prefix match
        for session in sessions {
            if session.slug.to_lowercase().starts_with(&name_lower) {
                return Ok(Some(session));
            }
        }

        Ok(None)
    }

    /// Rename a session (move its directory)
    pub fn rename_session(&self, old_slug: &str, new_slug: &str) -> Result<()> {
        let old_dir = self.session_dir(old_slug);
        let new_dir = self.session_dir(new_slug);

        if !old_dir.exists() {
            anyhow::bail!("Session '{old_slug}' not found");
        }
        if new_dir.exists() {
            anyhow::bail!("Session '{new_slug}' already exists");
        }

        fs::rename(&old_dir, &new_dir).context("Failed to rename session directory")?;
        Ok(())
    }

    /// Get list of existing session slugs (for collision checking)
    pub fn existing_slugs(&self) -> Result<Vec<String>> {
        Ok(self.list_sessions()?.into_iter().map(|s| s.slug).collect())
    }
}

/// Find the entry point markdown file in a directory
pub fn find_entry_point_in_dir(dir: &Path) -> Option<PathBuf> {
    // Priority order per spec
    for name in ["main.md", "notes.md", "readme.md", "README.md"] {
        let path = dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }

    // Fallback: first .md file alphabetically
    let mut md_files: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .map(|e| e.eq_ignore_ascii_case("md"))
                .unwrap_or(false)
        })
        .collect();

    md_files.sort();
    md_files.first().cloned()
}

/// List all files in a session directory
pub fn list_session_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .ok()
        .map(|entries| entries.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default()
}

/// Build a file tree for a session directory (pre-order traversal, flat list)
pub fn build_file_tree(
    dir: &Path,
    entry_point: Option<&Path>,
    max_depth: usize,
) -> Vec<FileTreeEntry> {
    let mut entries = Vec::new();
    build_file_tree_recursive(dir, entry_point, 0, max_depth, &[], &mut entries);
    entries
}

fn build_file_tree_recursive(
    dir: &Path,
    entry_point: Option<&Path>,
    depth: usize,
    max_depth: usize,
    ancestor_is_last: &[bool],
    entries: &mut Vec<FileTreeEntry>,
) {
    if depth > max_depth {
        return;
    }

    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };

    let mut children: Vec<_> = read_dir
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| !n.starts_with('.'))
                .unwrap_or(false)
        })
        .collect();

    children.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (false, true) => std::cmp::Ordering::Less,
            (true, false) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let total = children.len();
    for (i, child) in children.into_iter().enumerate() {
        let path = child.path();
        let is_dir = path.is_dir();
        let is_last = i == total - 1;
        let name = if is_dir {
            format!("{}/", child.file_name().to_string_lossy())
        } else {
            child.file_name().to_string_lossy().to_string()
        };

        let is_entry_point = entry_point.map(|ep| ep == path).unwrap_or(false);

        entries.push(FileTreeEntry {
            name,
            is_dir,
            depth,
            is_last,
            is_entry_point,
            ancestor_is_last: ancestor_is_last.to_vec(),
        });

        if is_dir {
            let mut next_ancestors = ancestor_is_last.to_vec();
            next_ancestors.push(is_last);
            build_file_tree_recursive(
                &path,
                entry_point,
                depth + 1,
                max_depth,
                &next_ancestors,
                entries,
            );
        }
    }
}

/// Detect the current context based on cwd
pub fn detect_context(cwd: &Path, _config: &Config) -> Context {
    // Walk up from cwd looking for .scratchpad/
    for ancestor in cwd.ancestors() {
        let project_pad = ancestor.join(".scratchpad");
        if project_pad.is_dir() {
            return Context::Project(project_pad);
        }
    }
    Context::User
}

/// Get all available contexts from cwd
pub fn available_contexts(cwd: &Path, _config: &Config) -> Vec<Context> {
    let mut contexts = vec![Context::User];

    for ancestor in cwd.ancestors() {
        let project_pad = ancestor.join(".scratchpad");
        if project_pad.is_dir() {
            contexts.push(Context::Project(project_pad));
            break;
        }
    }

    contexts
}
