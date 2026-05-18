use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context as _, Result};
use chrono::{DateTime, TimeZone, Utc};

use crate::models::{Config, FileTreeEntry, Session, SessionMeta, SessionStatus};
use crate::project::Project;

pub const META_DIR: &str = ".sp";
pub const META_FILE: &str = "meta.toml";

pub struct Storage {
    root: PathBuf,
    project: Project,
}

impl Storage {
    pub fn new(config: &Config, project: Project) -> Self {
        let root = PathBuf::from(&config.workspace_path);
        Self { root, project }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.root
    }

    pub fn project_dir(&self) -> PathBuf {
        if self.project.slug == "shared" {
            self.root.join("shared")
        } else {
            self.root.join("projects").join(&self.project.slug)
        }
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn switch_project(&mut self, project: Project) {
        self.project = project;
    }

    pub fn session_dir(&self, slug: &str) -> PathBuf {
        self.project_dir().join(slug)
    }

    pub fn ensure_workspace(&self) -> Result<()> {
        fs::create_dir_all(self.project_dir())
            .with_context(|| format!("Failed to create {}", self.project_dir().display()))?;
        Ok(())
    }

    pub fn meta_path(&self, slug: &str) -> PathBuf {
        self.session_dir(slug).join(META_DIR).join(META_FILE)
    }

    pub fn read_meta(&self, slug: &str) -> SessionMeta {
        let path = self.meta_path(slug);
        if !path.exists() {
            return SessionMeta::default();
        }
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return SessionMeta::default(),
        };
        toml::from_str(&content).unwrap_or_default()
    }

    pub fn write_meta(&self, slug: &str, meta: &SessionMeta) -> Result<()> {
        let dir = self.session_dir(slug).join(META_DIR);
        fs::create_dir_all(&dir).context("Failed to create .sp directory")?;
        let path = dir.join(META_FILE);
        let toml_str = toml::to_string_pretty(meta).context("Failed to serialize session meta")?;
        fs::write(&path, toml_str).context("Failed to write session meta")?;
        Ok(())
    }

    pub fn create_session(
        &self,
        slug: &str,
        initial_note: Option<&str>,
        tags: &[String],
    ) -> Result<Session> {
        if slug.is_empty() {
            anyhow::bail!("Session slug cannot be empty");
        }
        let session_dir = self.session_dir(slug);
        if session_dir.exists() {
            anyhow::bail!("Session '{slug}' already exists");
        }
        fs::create_dir_all(&session_dir).context("Failed to create session directory")?;

        let notes_content = initial_note.unwrap_or("");
        fs::write(session_dir.join("notes.md"), notes_content)
            .context("Failed to create notes.md")?;

        let now = Utc::now();
        let meta = SessionMeta {
            project: Some(self.project.slug.clone()),
            status: SessionStatus::Active,
            tags: tags.to_vec(),
            revision: if notes_content.is_empty() { 0 } else { 1 },
            created_at: Some(now),
        };
        self.write_meta(slug, &meta)?;

        Ok(self.load_session(slug)?.expect("freshly created session"))
    }

    pub fn load_session(&self, slug: &str) -> Result<Option<Session>> {
        let session_dir = self.session_dir(slug);
        if !session_dir.is_dir() {
            return Ok(None);
        }
        let metadata = fs::metadata(&session_dir).ok();
        let (created_at, updated_at) = fs_timestamps(metadata);
        let meta = self.read_meta(slug);
        Ok(Some(Session {
            slug: slug.to_string(),
            project: meta
                .project
                .clone()
                .unwrap_or_else(|| self.project.slug.clone()),
            path: session_dir,
            status: meta.status,
            tags: meta.tags.clone(),
            revision: meta.revision,
            created_at: meta.created_at.unwrap_or(created_at),
            updated_at,
        }))
    }

    pub fn list_sessions(&self) -> Result<Vec<Session>> {
        let project_dir = self.project_dir();
        if !project_dir.exists() {
            return Ok(Vec::new());
        }
        let mut sessions = Vec::new();
        for entry in fs::read_dir(&project_dir).context("Failed to read project directory")? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if name.is_empty() || name.starts_with('.') {
                continue;
            }
            if let Some(session) = self.load_session(&name)? {
                sessions.push(session);
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    pub fn find_entry_point(&self, slug: &str) -> Option<PathBuf> {
        let session_dir = self.session_dir(slug);
        find_entry_point_in_dir(&session_dir)
    }

    pub fn read_notes(&self, slug: &str) -> Result<String> {
        if let Some(entry_point) = self.find_entry_point(slug) {
            fs::read_to_string(&entry_point)
                .with_context(|| format!("Failed to read {}", entry_point.display()))
        } else {
            Ok(String::new())
        }
    }

    pub fn write_file(
        &self,
        slug: &str,
        rel_file: &str,
        content: &str,
        expect_revision: Option<u64>,
    ) -> Result<(PathBuf, u64)> {
        let session_dir = self.session_dir(slug);
        if !session_dir.exists() {
            anyhow::bail!("Session '{slug}' not found");
        }
        let target = sanitize_path(&session_dir, rel_file)?;

        let mut meta = self.read_meta(slug);
        if let Some(expected) = expect_revision
            && meta.revision != expected
        {
            return Err(RevisionConflict {
                current: meta.revision,
                expected,
            }
            .into());
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory for {}", target.display())
            })?;
        }
        fs::write(&target, content)
            .with_context(|| format!("Failed to write {}", target.display()))?;

        meta.revision = meta.revision.saturating_add(1);
        if meta.project.is_none() {
            meta.project = Some(self.project.slug.clone());
        }
        if meta.created_at.is_none() {
            meta.created_at = Some(Utc::now());
        }
        self.write_meta(slug, &meta)?;

        Ok((target, meta.revision))
    }

    pub fn append_file(&self, slug: &str, rel_file: &str, content: &str) -> Result<(PathBuf, u64)> {
        let session_dir = self.session_dir(slug);
        if !session_dir.exists() {
            anyhow::bail!("Session '{slug}' not found");
        }
        let target = sanitize_path(&session_dir, rel_file)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create parent directory for {}", target.display())
            })?;
        }
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)
            .with_context(|| format!("Failed to open {} for append", target.display()))?;
        file.write_all(content.as_bytes())
            .with_context(|| format!("Failed to append to {}", target.display()))?;

        let mut meta = self.read_meta(slug);
        meta.revision = meta.revision.saturating_add(1);
        if meta.project.is_none() {
            meta.project = Some(self.project.slug.clone());
        }
        if meta.created_at.is_none() {
            meta.created_at = Some(Utc::now());
        }
        self.write_meta(slug, &meta)?;

        Ok((target, meta.revision))
    }

    pub fn attach_file(&self, slug: &str, source: &Path, name: Option<&str>) -> Result<PathBuf> {
        let session_dir = self.session_dir(slug);
        if !session_dir.exists() {
            anyhow::bail!("Session '{slug}' not found");
        }
        let derived_name = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "attachment".to_string());
        let final_name = name.unwrap_or(&derived_name);
        let target = sanitize_path(&session_dir, final_name)?;
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, &target).with_context(|| {
            format!(
                "Failed to copy {} to {}",
                source.display(),
                target.display()
            )
        })?;

        let mut meta = self.read_meta(slug);
        meta.revision = meta.revision.saturating_add(1);
        if meta.project.is_none() {
            meta.project = Some(self.project.slug.clone());
        }
        self.write_meta(slug, &meta)?;
        Ok(target)
    }

    pub fn delete_session(&self, slug: &str) -> Result<()> {
        let session_dir = self.session_dir(slug);
        if session_dir.exists() {
            fs::remove_dir_all(&session_dir).context("Failed to delete session directory")?;
        }
        Ok(())
    }

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

    pub fn set_status(&self, slug: &str, status: SessionStatus) -> Result<()> {
        let mut meta = self.read_meta(slug);
        meta.status = status;
        if meta.project.is_none() {
            meta.project = Some(self.project.slug.clone());
        }
        self.write_meta(slug, &meta)
    }

    pub fn set_tags(&self, slug: &str, add: &[String], remove: &[String]) -> Result<Vec<String>> {
        let mut meta = self.read_meta(slug);
        for tag in remove {
            meta.tags.retain(|t| t != tag);
        }
        for tag in add {
            if !meta.tags.iter().any(|t| t == tag) {
                meta.tags.push(tag.clone());
            }
        }
        meta.tags.sort();
        meta.tags.dedup();
        if meta.project.is_none() {
            meta.project = Some(self.project.slug.clone());
        }
        self.write_meta(slug, &meta)?;
        Ok(meta.tags)
    }

    pub fn find_session_by_name(&self, name: &str) -> Result<Option<Session>> {
        let sessions = self.list_sessions()?;
        let name_lower = name.to_lowercase();
        for session in &sessions {
            if session.slug.to_lowercase() == name_lower {
                return Ok(Some(session.clone()));
            }
        }
        for session in sessions {
            if session.slug.to_lowercase().starts_with(&name_lower) {
                return Ok(Some(session));
            }
        }
        Ok(None)
    }

    pub fn existing_slugs(&self) -> Result<Vec<String>> {
        Ok(self.list_sessions()?.into_iter().map(|s| s.slug).collect())
    }
}

#[derive(Debug)]
pub struct RevisionConflict {
    pub current: u64,
    pub expected: u64,
}

impl std::fmt::Display for RevisionConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "revision conflict (current: {}, expected: {})",
            self.current, self.expected
        )
    }
}

impl std::error::Error for RevisionConflict {}

pub fn find_entry_point_in_dir(dir: &Path) -> Option<PathBuf> {
    for name in ["main.md", "notes.md", "readme.md", "README.md"] {
        let path = dir.join(name);
        if path.exists() {
            return Some(path);
        }
    }
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

pub fn list_session_files(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| !n.starts_with('.'))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default()
}

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

pub fn sanitize_path(session_dir: &Path, rel_file: &str) -> Result<PathBuf> {
    let path = Path::new(rel_file);
    if path.is_absolute() {
        anyhow::bail!("File path must be relative: {rel_file}");
    }
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("File path cannot escape the session directory: {rel_file}");
            }
        }
    }
    if clean.as_os_str().is_empty() {
        anyhow::bail!("File path cannot be empty");
    }
    Ok(session_dir.join(clean))
}

fn fs_timestamps(metadata: Option<fs::Metadata>) -> (DateTime<Utc>, DateTime<Utc>) {
    let Some(meta) = metadata else {
        let now = Utc::now();
        return (now, now);
    };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| Utc.timestamp_opt(d.as_secs() as i64, 0).unwrap())
        })
        .unwrap_or_else(Utc::now);
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
}

pub fn last_modified_file(dir: &Path, recurse_depth: usize) -> Option<(PathBuf, DateTime<Utc>)> {
    let mut best: Option<(PathBuf, DateTime<Utc>)> = None;
    visit_files(dir, recurse_depth, &mut |path, mtime| {
        if best
            .as_ref()
            .map(|(_, current)| mtime > *current)
            .unwrap_or(true)
        {
            best = Some((path.to_path_buf(), mtime));
        }
    });
    best
}

fn visit_files(dir: &Path, depth: usize, visitor: &mut dyn FnMut(&Path, DateTime<Utc>)) {
    let read_dir = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            if depth > 0 {
                visit_files(&path, depth - 1, visitor);
            }
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let (_, mtime) = fs_timestamps(Some(meta));
        visitor(&path, mtime);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_accepts_nested_relative_paths() {
        let p = sanitize_path(Path::new("/tmp/session"), "docs/notes.md").unwrap();
        assert_eq!(p, Path::new("/tmp/session/docs/notes.md"));
    }

    #[test]
    fn sanitize_rejects_parent_components() {
        let err = sanitize_path(Path::new("/tmp/session"), "../outside.md").unwrap_err();
        assert!(err.to_string().contains("cannot escape"));
    }

    #[test]
    fn sanitize_rejects_absolute_paths() {
        let err = sanitize_path(Path::new("/tmp/session"), "/etc/passwd").unwrap_err();
        assert!(err.to_string().contains("must be relative"));
    }
}
