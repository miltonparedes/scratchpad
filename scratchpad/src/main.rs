mod cli;
mod config;
mod hook;
mod markdown;
mod models;
mod names;
mod open;
mod project;
mod storage;
mod tui;

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Path, PathBuf};
use std::process;

use anyhow::{Context as _, Result};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use clap::Parser;
use serde::Serialize;
use serde_json::json;

use cli::{Cli, Command, ProjectAction};
use config::{config_path, load_config, save_config, upsert_project_alias};
use models::{Agent, Config, Session, SessionStatus};
use names::{generate_session_name, slugify, slugify_or_generate};
use open::{open_folder, open_path_blocking, open_with_editor};
use project::{
    Project, ProjectSource, default_short_form_hosts, find_git_repo_root, parse_remote_url,
    resolve_project,
};
use storage::{
    RevisionConflict, Storage, build_file_tree, find_entry_point_in_dir, last_modified_file,
};

const EXIT_REVISION_CONFLICT: i32 = 4;
const EXIT_NOT_IN_PROJECT: i32 = 5;
const EXIT_NOT_FOUND: i32 = 3;

#[derive(Debug, Clone)]
struct Ref {
    session: String,
    file: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    if let Err(err) = run(cli) {
        if let Some(conflict) = err.downcast_ref::<RevisionConflict>() {
            eprintln!("{conflict}");
            process::exit(EXIT_REVISION_CONFLICT);
        }
        eprintln!("error: {err:#}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let config = load_config()?;
    let cwd = std::env::current_dir().unwrap_or_default();
    let project = resolve_project(&cwd, &config, cli.project.as_deref());
    let storage = Storage::new(&config, project.clone());
    storage.ensure_workspace()?;

    match cli.command {
        None => {
            let projects = available_projects(&config, &project);
            tui::run(config, storage, projects, None)?;
        }
        Some(Command::New { name, tags, json }) => cmd_new(&storage, &config, name, tags, json)?,
        Some(Command::Quick { text, tags, json }) => {
            cmd_quick(&storage, &config, text, tags, json)?
        }
        Some(Command::Open { name }) => {
            let projects = available_projects(&config, &project);
            tui::run(config, storage, projects, name.as_deref())?;
        }
        Some(Command::Run { name, agent }) => cmd_run(&storage, &config, &project, name, agent)?,
        Some(Command::View { name }) => cmd_view(&storage, &config, name)?,
        Some(Command::Edit { name }) => cmd_edit(&storage, &config, name)?,
        Some(Command::List {
            all,
            shared,
            today,
            since,
            before,
            tags,
            status,
            json,
        }) => cmd_list(
            &config, &project, all, shared, today, since, before, tags, status, json,
        )?,
        Some(Command::Search {
            query,
            all,
            tags,
            status,
            json,
        }) => cmd_search(&config, &project, &query, all, tags, status, json)?,
        Some(Command::Last {
            count,
            r#in,
            all,
            session_only,
            uri,
            path,
            json,
        }) => cmd_last(
            &config,
            &project,
            count,
            r#in,
            all,
            session_only,
            uri,
            path,
            json,
        )?,
        Some(Command::Read { name, file }) => cmd_read(&storage, name, file)?,
        Some(Command::Resolve { reference }) => cmd_resolve(&storage, &reference)?,
        Some(Command::Write {
            reference,
            file,
            expect_revision,
            json,
        }) => cmd_write(&storage, &reference, file, expect_revision, json)?,
        Some(Command::Append {
            reference,
            file,
            json,
        }) => cmd_append(&storage, &reference, file, json)?,
        Some(Command::Attach {
            session,
            source,
            as_name,
            json,
        }) => cmd_attach(&storage, &session, &source, as_name, json)?,
        Some(Command::Files { name, flat, json }) => cmd_files(&storage, name, flat, json)?,
        Some(Command::Path { name }) => cmd_path(&storage, name)?,
        Some(Command::Folder { name }) => cmd_folder(&storage, name)?,
        Some(Command::Rename { current, new_name }) => cmd_rename(&storage, current, new_name)?,
        Some(Command::Archive { name }) => {
            cmd_set_status(&storage, &name, SessionStatus::Archived)?
        }
        Some(Command::Restore { name }) => cmd_set_status(&storage, &name, SessionStatus::Active)?,
        Some(Command::Tag {
            session,
            changes,
            json,
        }) => cmd_tag(&storage, &session, changes, json)?,
        Some(Command::Delete { name, yes }) => cmd_delete(&storage, &name, yes)?,
        Some(Command::Context) => cmd_context(&storage, &project)?,
        Some(Command::Project { action }) => cmd_project(action, &config, &project, &cwd)?,
        Some(Command::Link { reference, copy }) => cmd_link(&storage, &reference, copy)?,
        Some(Command::Config { action }) => config::handle_config(action, &config)?,
        Some(Command::Hook { name }) => hook::handle(&name)?,
        Some(Command::Sync) => {
            println!("Sync not yet implemented.");
            println!("Configure server in {}", config_path().display());
        }
    }
    Ok(())
}

fn cmd_new(
    storage: &Storage,
    config: &Config,
    name: Option<String>,
    tags: Vec<String>,
    json: bool,
) -> Result<()> {
    let existing = storage.existing_slugs()?;
    let slug = match name {
        Some(n) => slugify_or_generate(&n, &existing, config),
        None => generate_session_name(&existing, config),
    };
    let session = storage.create_session(&slug, None, &tags)?;
    print_session_created(&session, json)
}

fn cmd_quick(
    storage: &Storage,
    config: &Config,
    text: String,
    tags: Vec<String>,
    json: bool,
) -> Result<()> {
    let existing = storage.existing_slugs()?;
    let slug = generate_session_name(&existing, config);
    let session = storage.create_session(&slug, Some(&text), &tags)?;
    print_session_created(&session, json)
}

fn print_session_created(session: &Session, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string(&session_to_json(session))?);
    } else {
        println!("{}", session.path.display());
    }
    Ok(())
}

fn cmd_run(
    storage: &Storage,
    config: &Config,
    project: &Project,
    name: Option<String>,
    agent: Option<String>,
) -> Result<()> {
    let session = resolve_session(storage, name)?;
    let session_dir = storage.session_dir(&session.slug);
    let agent_name = agent.unwrap_or_else(|| match config.default_agent {
        Agent::Claude => "claude".to_string(),
        Agent::Codex => "codex".to_string(),
        Agent::Custom => "claude".to_string(),
    });

    let (program, args) = if let Some(binding) = config.agents.get(&agent_name) {
        (binding.command.clone(), binding.args.clone())
    } else {
        (agent_name.clone(), Vec::new())
    };

    eprintln!("Running {agent_name} in {}", session.path.display());
    let status = process::Command::new(&program)
        .args(&args)
        .current_dir(&session_dir)
        .env("SP_SESSION", &session.slug)
        .env("SP_PROJECT", &project.slug)
        .env("SP_WORKSPACE", storage.workspace_root())
        .env("SP_SESSION_DIR", &session_dir)
        .status()
        .with_context(|| format!("Failed to launch {program}"))?;
    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn cmd_view(storage: &Storage, config: &Config, name: Option<String>) -> Result<()> {
    let session = resolve_session(storage, name)?;
    let session_dir = storage.session_dir(&session.slug);
    if let Some(entry_point) = storage.find_entry_point(&session.slug) {
        open_path_blocking(&entry_point, config.viewer.as_deref())?;
    } else {
        open_folder(&session_dir)?;
    }
    Ok(())
}

fn cmd_edit(storage: &Storage, config: &Config, name: Option<String>) -> Result<()> {
    let session = resolve_session(storage, name)?;
    let session_dir = storage.session_dir(&session.slug);
    if let Some(entry_point) = storage.find_entry_point(&session.slug) {
        open_with_editor(&entry_point, config.editor.as_deref())?;
    } else {
        let notes_path = session_dir.join("notes.md");
        if !notes_path.exists() {
            fs::write(&notes_path, "")?;
        }
        open_with_editor(&notes_path, config.editor.as_deref())?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_list(
    config: &Config,
    project: &Project,
    all: bool,
    shared: bool,
    today: bool,
    since: Option<String>,
    before: Option<String>,
    tags: Vec<String>,
    status: Option<String>,
    json: bool,
) -> Result<()> {
    let status_filter = parse_status_filter(status.as_deref())?;
    let since_cutoff = parse_since(since.as_deref(), today)?;
    let before_cutoff = parse_before(before.as_deref())?;

    let sessions = collect_sessions(config, project, all, shared)?;
    let filtered: Vec<Session> = sessions
        .into_iter()
        .filter(|s| filter_session(s, &tags, status_filter, since_cutoff, before_cutoff))
        .collect();

    if json {
        let arr: Vec<_> = filtered.iter().map(session_to_json).collect();
        println!("{}", serde_json::to_string(&arr)?);
        return Ok(());
    }

    if filtered.is_empty() {
        eprintln!("No sessions found.");
        return Ok(());
    }

    let is_tty = io::stdout().is_terminal();
    if !is_tty {
        for session in &filtered {
            println!(
                "{}\t{}\t{}\t{}",
                session.slug,
                session.project,
                session.status.as_str(),
                session.updated_at.to_rfc3339(),
            );
        }
        return Ok(());
    }

    println!(
        "{:<28}  {:<14}  {:<9}  {:<19}  TAGS",
        "NAME", "PROJECT", "STATUS", "UPDATED"
    );
    println!("{}", "-".repeat(90));
    for session in &filtered {
        let name = truncate(&session.slug, 28);
        let project = truncate(&session.project, 14);
        let status = session.status.as_str();
        let updated = session.updated_at.format("%Y-%m-%d %H:%M").to_string();
        let tags = if session.tags.is_empty() {
            "-".to_string()
        } else {
            session.tags.join(",")
        };
        println!("{name:<28}  {project:<14}  {status:<9}  {updated:<19}  {tags}");
    }
    Ok(())
}

fn cmd_search(
    config: &Config,
    project: &Project,
    query: &str,
    all: bool,
    tags: Vec<String>,
    status: Option<String>,
    json: bool,
) -> Result<()> {
    let query_lower = query.to_lowercase();
    let status_filter = parse_status_filter(status.as_deref())?;
    let sessions = collect_sessions(config, project, all, false)?;

    let mut matches: Vec<serde_json::Value> = Vec::new();
    let mut human: Vec<String> = Vec::new();
    for session in &sessions {
        if !filter_session(session, &tags, status_filter, None, None) {
            continue;
        }
        let mut found_in_files: Vec<(PathBuf, Vec<(usize, String)>)> = Vec::new();
        let session_dir = &session.path;
        for entry in walk_files(session_dir, 4) {
            let Ok(content) = fs::read_to_string(&entry) else {
                continue;
            };
            let mut hits = Vec::new();
            for (i, line) in content.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    hits.push((i + 1, line.to_string()));
                    if hits.len() >= 5 {
                        break;
                    }
                }
            }
            if !hits.is_empty() {
                found_in_files.push((entry, hits));
            }
        }
        let name_match = session.slug.to_lowercase().contains(&query_lower);
        if !name_match && found_in_files.is_empty() {
            continue;
        }

        if json {
            let hits_json: Vec<_> = found_in_files
                .iter()
                .map(|(path, hits)| {
                    json!({
                        "path": path.display().to_string(),
                        "hits": hits.iter().map(|(line, text)| {
                            json!({"line": line, "text": text})
                        }).collect::<Vec<_>>()
                    })
                })
                .collect();
            matches.push(json!({
                "session": session_to_json(session),
                "name_match": name_match,
                "files": hits_json,
            }));
        } else {
            human.push(format!(
                "{}/{}  ({})",
                session.project,
                session.slug,
                if name_match { "name" } else { "content" }
            ));
            for (path, hits) in &found_in_files {
                let rel = path
                    .strip_prefix(session_dir)
                    .unwrap_or(path)
                    .display()
                    .to_string();
                for (line, text) in hits {
                    human.push(format!("    {rel}:{line}  {}", text.trim()));
                }
            }
        }
    }

    if json {
        println!("{}", serde_json::to_string(&matches)?);
    } else if human.is_empty() {
        eprintln!("No matches.");
    } else {
        for line in human {
            println!("{line}");
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_last(
    config: &Config,
    project: &Project,
    count: usize,
    in_session: Option<String>,
    all: bool,
    session_only: bool,
    uri: bool,
    print_path: bool,
    json: bool,
) -> Result<()> {
    let sessions = collect_sessions(config, project, all, false)?;

    let mut artifacts: Vec<(Session, PathBuf, DateTime<Utc>)> = Vec::new();
    for session in &sessions {
        if let Some(filter) = in_session.as_deref()
            && !slug_matches(&session.slug, filter)
        {
            continue;
        }
        if let Some((path, mtime)) = last_modified_file(&session.path, 4) {
            artifacts.push((session.clone(), path, mtime));
        }
    }
    artifacts.sort_by(|a, b| b.2.cmp(&a.2));
    artifacts.truncate(count);

    if artifacts.is_empty() {
        process::exit(EXIT_NOT_FOUND);
    }

    if json {
        let arr: Vec<_> = artifacts
            .iter()
            .map(|(s, p, m)| {
                let size = fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                json!({
                    "session": s.slug,
                    "project": s.project,
                    "file": p.strip_prefix(&s.path).unwrap_or(p).display().to_string(),
                    "path": p.display().to_string(),
                    "size": size,
                    "mtime": m.to_rfc3339(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&arr)?);
        return Ok(());
    }

    for (session, path, _) in &artifacts {
        if session_only {
            println!("{}", session.slug);
        } else if uri {
            let rel = path
                .strip_prefix(&session.path)
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            if rel.is_empty() {
                println!("sp://{}", session.slug);
            } else {
                println!("sp://{}/{}", session.slug, rel);
            }
        } else if print_path || !io::stdout().is_terminal() {
            println!("{}", path.display());
        } else {
            let rel = path
                .strip_prefix(&session.path)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.display().to_string());
            println!("{}/{}", session.slug, rel);
        }
    }
    Ok(())
}

fn cmd_read(storage: &Storage, name: Option<String>, file: Option<String>) -> Result<()> {
    let session = resolve_session(storage, name)?;
    let content = match file {
        Some(f) => {
            let session_dir = storage.session_dir(&session.slug);
            let path = storage::sanitize_path(&session_dir, &f)?;
            fs::read_to_string(&path).with_context(|| format!("Failed to read {f}"))?
        }
        None => storage.read_notes(&session.slug)?,
    };
    print!("{content}");
    Ok(())
}

fn cmd_resolve(storage: &Storage, reference: &str) -> Result<()> {
    let r = parse_ref(reference)?;
    let session = match storage.find_session_by_name(&r.session)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {}", r.session);
            process::exit(EXIT_NOT_FOUND);
        }
    };
    let session_dir = storage.session_dir(&session.slug);
    let path = if let Some(file) = r.file {
        storage::sanitize_path(&session_dir, &file)?
    } else if let Some(ep) = storage.find_entry_point(&session.slug) {
        ep
    } else {
        session_dir
    };
    println!("{}", path.display());
    Ok(())
}

fn cmd_write(
    storage: &Storage,
    reference: &str,
    file: Option<String>,
    expect_revision: Option<u64>,
    json: bool,
) -> Result<()> {
    let mut r = parse_ref(reference)?;
    if r.file.is_none() {
        r.file = file.clone();
    }
    let session = match storage.find_session_by_name(&r.session)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {}", r.session);
            process::exit(EXIT_NOT_FOUND);
        }
    };
    let mut content = String::new();
    io::stdin().read_to_string(&mut content)?;
    let rel_file = r.file.as_deref().unwrap_or("notes.md");
    let (path, revision) =
        storage.write_file(&session.slug, rel_file, &content, expect_revision)?;
    print_write_result(&session.slug, &path, revision, json)
}

fn cmd_append(storage: &Storage, reference: &str, file: Option<String>, json: bool) -> Result<()> {
    let mut r = parse_ref(reference)?;
    if r.file.is_none() {
        r.file = file.clone();
    }
    let session = match storage.find_session_by_name(&r.session)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {}", r.session);
            process::exit(EXIT_NOT_FOUND);
        }
    };
    let mut content = String::new();
    io::stdin().read_to_string(&mut content)?;
    let rel_file = r.file.as_deref().unwrap_or("notes.md");
    let (path, revision) = storage.append_file(&session.slug, rel_file, &content)?;
    print_write_result(&session.slug, &path, revision, json)
}

fn cmd_attach(
    storage: &Storage,
    session_name: &str,
    source: &str,
    as_name: Option<String>,
    json: bool,
) -> Result<()> {
    let session = match storage.find_session_by_name(session_name)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {session_name}");
            process::exit(EXIT_NOT_FOUND);
        }
    };
    let source_path = Path::new(source);
    if !source_path.exists() {
        anyhow::bail!("Source file not found: {source}");
    }
    let path = storage.attach_file(&session.slug, source_path, as_name.as_deref())?;
    let revision = storage.read_meta(&session.slug).revision;
    print_write_result(&session.slug, &path, revision, json)
}

fn print_write_result(slug: &str, path: &Path, revision: u64, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "session": slug,
                "path": path.display().to_string(),
                "revision": revision,
            }))?
        );
    } else {
        println!("{}", path.display());
    }
    Ok(())
}

fn cmd_files(storage: &Storage, name: Option<String>, flat: bool, json: bool) -> Result<()> {
    let session = resolve_session(storage, name)?;
    let session_dir = storage.session_dir(&session.slug);
    let entry_point = storage.find_entry_point(&session.slug);
    let tree = build_file_tree(&session_dir, entry_point.as_deref(), 3);

    if json {
        let arr: Vec<_> = tree
            .iter()
            .map(|e| {
                json!({
                    "name": e.name,
                    "depth": e.depth,
                    "is_dir": e.is_dir,
                    "is_entry_point": e.is_entry_point,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&arr)?);
        return Ok(());
    }

    if flat || !io::stdout().is_terminal() {
        for entry in &tree {
            if entry.is_dir {
                continue;
            }
            let mut parts: Vec<&str> = Vec::new();
            let mut depth = entry.depth;
            for prior in tree.iter().rev().skip_while(|e| !std::ptr::eq(*e, entry)) {
                if prior.is_dir && prior.depth + 1 == depth {
                    parts.push(prior.name.trim_end_matches('/'));
                    depth = prior.depth;
                    if depth == 0 {
                        break;
                    }
                }
            }
            parts.reverse();
            parts.push(&entry.name);
            println!("{}", parts.join("/"));
        }
        return Ok(());
    }

    println!("{}/", session.slug);
    for entry in &tree {
        let mut prefix = String::new();
        for &ancestor_last in &entry.ancestor_is_last {
            prefix.push_str(if ancestor_last { "    " } else { "│   " });
        }
        let connector = if entry.is_last {
            "└── "
        } else {
            "├── "
        };
        let indicator = if entry.is_entry_point { "  *" } else { "" };
        println!("{prefix}{connector}{}{indicator}", entry.name);
    }
    Ok(())
}

fn cmd_path(storage: &Storage, name: Option<String>) -> Result<()> {
    let session = resolve_session(storage, name)?;
    print!("{}", storage.session_dir(&session.slug).display());
    Ok(())
}

fn cmd_folder(storage: &Storage, name: Option<String>) -> Result<()> {
    let session = resolve_session(storage, name)?;
    let dir = storage.session_dir(&session.slug);
    open_folder(&dir)?;
    Ok(())
}

fn cmd_rename(storage: &Storage, current: Option<String>, new_name: String) -> Result<()> {
    let session = resolve_session(storage, current)?;
    let new_slug = match slugify(&new_name) {
        Some(s) => s,
        None => {
            eprintln!("Invalid session name: '{new_name}'");
            process::exit(1);
        }
    };
    storage.rename_session(&session.slug, &new_slug)?;
    println!("Renamed '{}' to '{new_slug}'", session.slug);
    Ok(())
}

fn cmd_set_status(storage: &Storage, name: &str, status: SessionStatus) -> Result<()> {
    let session = match storage.find_session_by_name(name)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {name}");
            process::exit(EXIT_NOT_FOUND);
        }
    };
    storage.set_status(&session.slug, status)?;
    eprintln!("{} → {}", session.slug, status.as_str());
    Ok(())
}

fn cmd_tag(storage: &Storage, session_name: &str, changes: Vec<String>, json: bool) -> Result<()> {
    let session = match storage.find_session_by_name(session_name)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {session_name}");
            process::exit(EXIT_NOT_FOUND);
        }
    };
    let mut add = Vec::new();
    let mut remove = Vec::new();
    for change in changes {
        if let Some(tag) = change.strip_prefix('-') {
            remove.push(tag.to_string());
        } else if let Some(tag) = change.strip_prefix('+') {
            add.push(tag.to_string());
        } else {
            add.push(change);
        }
    }
    let tags = storage.set_tags(&session.slug, &add, &remove)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&json!({
                "session": session.slug,
                "tags": tags,
            }))?
        );
    } else if tags.is_empty() {
        eprintln!("{}: no tags", session.slug);
    } else {
        println!("{}", tags.join(","));
    }
    Ok(())
}

fn cmd_delete(storage: &Storage, name: &str, yes: bool) -> Result<()> {
    let session = match storage.find_session_by_name(name)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {name}");
            process::exit(EXIT_NOT_FOUND);
        }
    };
    if !yes {
        eprint!("Delete session '{}'? [y/N]: ", session.slug);
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim().to_lowercase() != "y" {
            process::exit(0);
        }
    }
    storage.delete_session(&session.slug)?;
    eprintln!("Deleted: {}", session.slug);
    Ok(())
}

fn cmd_context(storage: &Storage, project: &Project) -> Result<()> {
    println!("project:   {}", project.slug);
    println!("source:    {}", project.source.label());
    println!("workspace: {}", storage.project_dir().display());
    Ok(())
}

fn cmd_project(
    action: ProjectAction,
    config: &Config,
    project: &Project,
    cwd: &Path,
) -> Result<()> {
    match action {
        ProjectAction::Current { json } => {
            let storage = Storage::new(config, project.clone());
            let stats = project_stats(&storage)?;
            if json {
                let mut obj = serde_json::Map::new();
                obj.insert("project".into(), json!(project.slug));
                obj.insert("source".into(), serde_json::to_value(&project.source)?);
                obj.insert(
                    "workspace".into(),
                    json!(storage.project_dir().display().to_string()),
                );
                obj.insert("stats".into(), serde_json::to_value(&stats)?);
                if let ProjectSource::GitRemoteOrigin {
                    remote_url,
                    repo_root,
                } = &project.source
                {
                    obj.insert("remote_url".into(), json!(remote_url));
                    obj.insert("repo_root".into(), json!(repo_root.display().to_string()));
                } else if let ProjectSource::RepoBasename { repo_root } = &project.source {
                    obj.insert("repo_root".into(), json!(repo_root.display().to_string()));
                } else if let ProjectSource::Alias { alias_name, repo } = &project.source {
                    obj.insert("alias_name".into(), json!(alias_name));
                    obj.insert("repo".into(), json!(repo));
                }
                println!("{}", serde_json::to_string(&obj)?);
            } else {
                println!("project:   {}", project.slug);
                println!("source:    {}", source_description(&project.source));
                println!("workspace: {}", storage.project_dir().display());
                println!("active:    {}", stats.active);
                println!("archived:  {}", stats.archived);
            }
        }
        ProjectAction::List { json } => {
            let mut entries = project_entries(config)?;
            entries.sort_by(|a, b| a.name.cmp(&b.name));
            if json {
                let arr: Vec<_> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "name": e.name,
                            "path": e.path.display().to_string(),
                            "alias": e.alias,
                            "repos": e.repos,
                            "sessions": e.session_count,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string(&arr)?);
            } else {
                println!("{:<28}  {:<8}  REPOS", "NAME", "SESSIONS");
                println!("{}", "-".repeat(70));
                for e in &entries {
                    let repos = e.repos.join(", ");
                    println!("{:<28}  {:<8}  {}", e.name, e.session_count, repos);
                }
            }
        }
        ProjectAction::Save { as_name } => {
            let mut new_config = config.clone();
            let name = as_name.unwrap_or_else(|| project.slug.clone());
            let repos = match &project.source {
                ProjectSource::GitRemoteOrigin { remote_url, .. } => {
                    if let Some(ident) = parse_remote_url(remote_url) {
                        vec![
                            ident.canonical_slug(&effective_short_form(config)),
                            remote_url.clone(),
                        ]
                    } else {
                        vec![remote_url.clone()]
                    }
                }
                ProjectSource::Alias { repo, .. } => vec![repo.clone()],
                _ => Vec::new(),
            };
            upsert_project_alias(&mut new_config, &name, repos);
            save_config(&new_config)?;
            println!("Saved alias '{name}' to {}", config_path().display());
        }
        ProjectAction::Link { name, repo } => {
            let mut new_config = config.clone();
            let repo_id = match repo {
                Some(r) => {
                    if let Some(ident) = parse_remote_url(&r) {
                        ident.canonical_slug(&effective_short_form(config))
                    } else {
                        r
                    }
                }
                None => {
                    let root = find_git_repo_root(cwd).ok_or_else(|| {
                        anyhow::anyhow!("Not inside a git repository; pass repo explicitly")
                    })?;
                    let url = git_origin_url(&root).ok_or_else(|| {
                        anyhow::anyhow!("No 'origin' remote in {}", root.display())
                    })?;
                    let ident = parse_remote_url(&url)
                        .ok_or_else(|| anyhow::anyhow!("Could not parse remote URL: {url}"))?;
                    ident.canonical_slug(&effective_short_form(config))
                }
            };
            upsert_project_alias(&mut new_config, &name, vec![repo_id.clone()]);
            save_config(&new_config)?;
            println!("Linked '{repo_id}' → '{name}'");
        }
        ProjectAction::Rename { old, new } => {
            let workspace = workspace_root_from_config(config);
            let old_dir = workspace.join("projects").join(&old);
            let new_dir = workspace.join("projects").join(&new);
            if !old_dir.exists() {
                anyhow::bail!("Project '{old}' does not exist on disk");
            }
            if new_dir.exists() {
                anyhow::bail!("Project '{new}' already exists on disk");
            }
            if let Some(parent) = new_dir.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&old_dir, &new_dir)?;

            let mut new_config = config.clone();
            for alias in new_config.projects.iter_mut() {
                if alias.name == old {
                    alias.name = new.clone();
                }
            }
            save_config(&new_config)?;
            println!("Renamed project '{old}' → '{new}'");
        }
    }
    Ok(())
}

fn cmd_link(storage: &Storage, reference: &str, copy: bool) -> Result<()> {
    let r = parse_ref(reference)?;
    let session = match storage.find_session_by_name(&r.session)? {
        Some(s) => s,
        None => {
            eprintln!("Session not found: {}", r.session);
            process::exit(EXIT_NOT_FOUND);
        }
    };
    let session_dir = storage.session_dir(&session.slug);
    let path = if let Some(file) = r.file {
        storage::sanitize_path(&session_dir, &file)?
    } else {
        session_dir
    };
    let display = path.display().to_string();
    println!("{display}");
    if copy && let Err(e) = copy_to_clipboard(&display) {
        eprintln!("(clipboard copy failed: {e})");
    }
    Ok(())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "windows") {
        &[("clip.exe", &[])]
    } else {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
    };
    for (cmd, args) in candidates {
        if which::which(cmd).is_err() {
            continue;
        }
        let mut child = process::Command::new(cmd)
            .args(*args)
            .stdin(process::Stdio::piped())
            .spawn()?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        return Ok(());
    }
    Err(anyhow::anyhow!("no clipboard tool available"))
}

fn resolve_session(storage: &Storage, name: Option<String>) -> Result<Session> {
    match name {
        Some(n) => match storage.find_session_by_name(&n)? {
            Some(s) => Ok(s),
            None => {
                eprintln!("Session not found: {n}");
                process::exit(EXIT_NOT_FOUND);
            }
        },
        None => pick_session_fzf(storage),
    }
}

fn pick_session_fzf(storage: &Storage) -> Result<Session> {
    let sessions = storage.list_sessions()?;
    if sessions.is_empty() {
        eprintln!("No sessions found in project '{}'.", storage.project().slug);
        process::exit(EXIT_NOT_IN_PROJECT);
    }
    if which::which("fzf").is_err() {
        eprintln!("fzf not found. Provide a session name or install fzf.");
        process::exit(1);
    }

    let input: String = sessions.iter().map(|s| format!("{}\n", s.slug)).collect();
    let project_dir = storage.project_dir();
    let preview_cmd = format!("ls -1 {}/{{}}/", project_dir.display());

    let mut child = process::Command::new("fzf")
        .args([
            "--height=~50%",
            "--reverse",
            "--prompt=session> ",
            "--preview",
            &preview_cmd,
        ])
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::inherit())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        process::exit(1);
    }
    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    match storage.find_session_by_name(&selected)? {
        Some(session) => Ok(session),
        None => {
            eprintln!("Session not found: {selected}");
            process::exit(EXIT_NOT_FOUND);
        }
    }
}

fn parse_ref(reference: &str) -> Result<Ref> {
    let trimmed = reference.trim();
    let trimmed = trimmed.strip_prefix("sp://").unwrap_or(trimmed);
    if trimmed.is_empty() {
        anyhow::bail!("Reference is empty");
    }
    if let Some((session, file)) = trimmed.split_once('/') {
        if session.is_empty() {
            anyhow::bail!("Reference missing session: {reference}");
        }
        Ok(Ref {
            session: session.to_string(),
            file: if file.is_empty() {
                None
            } else {
                Some(file.to_string())
            },
        })
    } else {
        Ok(Ref {
            session: trimmed.to_string(),
            file: None,
        })
    }
}

fn parse_status_filter(value: Option<&str>) -> Result<Option<SessionStatus>> {
    match value {
        None => Ok(None),
        Some(v) => v
            .parse::<SessionStatus>()
            .map(Some)
            .map_err(|e| anyhow::anyhow!(e)),
    }
}

fn parse_since(value: Option<&str>, today: bool) -> Result<Option<DateTime<Utc>>> {
    if today {
        let start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .map(|n| DateTime::<Utc>::from_naive_utc_and_offset(n, Utc))
            .unwrap_or(Utc::now());
        return Ok(Some(start));
    }
    let Some(spec) = value else { return Ok(None) };
    parse_duration_spec(spec).map(Some)
}

fn parse_before(value: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    let Some(spec) = value else { return Ok(None) };
    if let Ok(date) = NaiveDate::parse_from_str(spec, "%Y-%m-%d") {
        let start = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(Some(DateTime::<Utc>::from_naive_utc_and_offset(start, Utc)));
    }
    Err(anyhow::anyhow!(
        "Invalid --before date: {spec} (expected YYYY-MM-DD)"
    ))
}

fn parse_duration_spec(spec: &str) -> Result<DateTime<Utc>> {
    let trimmed = spec.trim();
    if let Ok(date) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        let start = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(start, Utc));
    }
    let mut chars = trimmed.chars();
    let unit = chars
        .next_back()
        .ok_or_else(|| anyhow::anyhow!("Empty --since spec"))?;
    let head: String = chars.collect();
    let n: i64 = head
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid --since spec: {trimmed}"))?;
    let duration = match unit {
        'h' => Duration::hours(n),
        'd' => Duration::days(n),
        'w' => Duration::weeks(n),
        'm' => Duration::days(n * 30),
        _ => {
            return Err(anyhow::anyhow!(
                "Unknown unit '{unit}' in --since (use h/d/w/m)"
            ));
        }
    };
    Ok(Utc::now() - duration)
}

fn filter_session(
    session: &Session,
    tags: &[String],
    status: Option<SessionStatus>,
    since: Option<DateTime<Utc>>,
    before: Option<DateTime<Utc>>,
) -> bool {
    if let Some(status) = status
        && session.status != status
    {
        return false;
    }
    for tag in tags {
        if !session.tags.iter().any(|t| t == tag) {
            return false;
        }
    }
    if let Some(since) = since
        && session.updated_at < since
    {
        return false;
    }
    if let Some(before) = before
        && session.updated_at >= before
    {
        return false;
    }
    true
}

fn collect_sessions(
    config: &Config,
    project: &Project,
    all: bool,
    shared_only: bool,
) -> Result<Vec<Session>> {
    if all || shared_only {
        let root = workspace_root_from_config(config);
        let projects_dir = root.join("projects");
        let mut sessions = Vec::new();
        if projects_dir.exists() && !shared_only {
            walk_projects(&projects_dir, &mut Vec::new(), &mut |slug, _path| {
                let proj = Project {
                    slug: slug.to_string(),
                    source: ProjectSource::Alias {
                        alias_name: slug.to_string(),
                        repo: String::new(),
                    },
                };
                let scoped = Storage::new(config, proj);
                if let Ok(list) = scoped.list_sessions() {
                    sessions.extend(list);
                }
            });
        }
        let shared_dir = root.join("shared");
        if shared_dir.exists() {
            let proj = Project {
                slug: "shared".to_string(),
                source: ProjectSource::Shared,
            };
            let scoped = Storage::new(config, proj);
            if let Ok(list) = scoped.list_sessions() {
                sessions.extend(list);
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    } else {
        let storage = Storage::new(config, project.clone());
        storage.list_sessions()
    }
}

fn walk_projects(dir: &Path, stack: &mut Vec<String>, visitor: &mut dyn FnMut(&str, &Path)) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut has_sessions = false;
    let mut subdirs = Vec::new();
    for entry in read.flatten() {
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let sub_files = fs::read_dir(&p).ok().map(|r| {
            r.flatten()
                .any(|e| e.file_name().to_string_lossy() == "notes.md")
        });
        if sub_files == Some(true) {
            has_sessions = true;
        } else {
            subdirs.push((name, p));
        }
    }
    if has_sessions {
        let slug = stack.join("/");
        if !slug.is_empty() {
            visitor(&slug, dir);
        }
    }
    for (name, path) in subdirs {
        stack.push(name);
        walk_projects(&path, stack, visitor);
        stack.pop();
    }
}

#[derive(Serialize)]
struct ProjectStats {
    active: u64,
    archived: u64,
}

fn project_stats(storage: &Storage) -> Result<ProjectStats> {
    let sessions = storage.list_sessions()?;
    let mut stats = ProjectStats {
        active: 0,
        archived: 0,
    };
    for s in &sessions {
        match s.status {
            SessionStatus::Active => stats.active += 1,
            SessionStatus::Archived => stats.archived += 1,
        }
    }
    Ok(stats)
}

struct ProjectEntry {
    name: String,
    path: PathBuf,
    alias: bool,
    repos: Vec<String>,
    session_count: u64,
}

fn project_entries(config: &Config) -> Result<Vec<ProjectEntry>> {
    let root = workspace_root_from_config(config);
    let projects_dir = root.join("projects");
    let mut entries: Vec<ProjectEntry> = Vec::new();

    if projects_dir.exists() {
        walk_projects(&projects_dir, &mut Vec::new(), &mut |slug, path| {
            let count = fs::read_dir(path)
                .map(|r| {
                    r.flatten()
                        .filter(|e| {
                            e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.')
                        })
                        .count() as u64
                })
                .unwrap_or(0);
            entries.push(ProjectEntry {
                name: slug.to_string(),
                path: path.to_path_buf(),
                alias: false,
                repos: Vec::new(),
                session_count: count,
            });
        });
    }

    for alias in &config.projects {
        if let Some(existing) = entries.iter_mut().find(|e| e.name == alias.name) {
            existing.alias = true;
            existing.repos = alias.repos.clone();
        } else {
            entries.push(ProjectEntry {
                name: alias.name.clone(),
                path: projects_dir.join(&alias.name),
                alias: true,
                repos: alias.repos.clone(),
                session_count: 0,
            });
        }
    }
    let shared_dir = root.join("shared");
    if shared_dir.exists() {
        let count = fs::read_dir(&shared_dir)
            .map(|r| {
                r.flatten()
                    .filter(|e| {
                        e.path().is_dir() && !e.file_name().to_string_lossy().starts_with('.')
                    })
                    .count() as u64
            })
            .unwrap_or(0);
        entries.push(ProjectEntry {
            name: "shared".to_string(),
            path: shared_dir,
            alias: false,
            repos: Vec::new(),
            session_count: count,
        });
    }
    Ok(entries)
}

fn available_projects(config: &Config, active: &Project) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(entries) = project_entries(config) {
        for entry in entries {
            names.push(entry.name);
        }
    }
    if !names.iter().any(|n| n == &active.slug) {
        names.push(active.slug.clone());
    }
    names.sort();
    names.dedup();
    names
}

fn source_description(source: &ProjectSource) -> String {
    match source {
        ProjectSource::Flag => "command-line flag".into(),
        ProjectSource::Env => "SP_PROJECT env var".into(),
        ProjectSource::GitConfig => "git config sp.project".into(),
        ProjectSource::Alias { alias_name, repo } => {
            format!("alias '{alias_name}' (repo {repo})")
        }
        ProjectSource::GitRemoteOrigin {
            remote_url,
            repo_root,
        } => format!("origin → {} ({})", remote_url, repo_root.display()),
        ProjectSource::RepoBasename { repo_root } => {
            format!("repo basename ({})", repo_root.display())
        }
        ProjectSource::Shared => "shared (no project)".into(),
    }
}

fn workspace_root_from_config(config: &Config) -> PathBuf {
    PathBuf::from(&config.workspace_path)
}

fn effective_short_form(config: &Config) -> Vec<String> {
    config
        .hosts
        .as_ref()
        .map(|h| h.short_form.clone())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(default_short_form_hosts)
}

fn git_origin_url(repo_root: &Path) -> Option<String> {
    let output = process::Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() { None } else { Some(url) }
}

fn slug_matches(actual: &str, query: &str) -> bool {
    actual.to_lowercase() == query.to_lowercase()
        || actual.to_lowercase().starts_with(&query.to_lowercase())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

fn walk_files(dir: &Path, depth: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk_files_inner(dir, depth, &mut out);
    out
}

fn walk_files_inner(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        let p = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            if depth > 0 {
                walk_files_inner(&p, depth - 1, out);
            }
            continue;
        }
        out.push(p);
    }
}

fn session_to_json(session: &Session) -> serde_json::Value {
    json!({
        "slug": session.slug,
        "project": session.project,
        "path": session.path.display().to_string(),
        "status": session.status.as_str(),
        "tags": session.tags,
        "revision": session.revision,
        "updated_at": session.updated_at.to_rfc3339(),
        "created_at": session.created_at.to_rfc3339(),
        "entry_point": find_entry_point_in_dir(&session.path)
            .map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ref_session_only() {
        let r = parse_ref("auth-refactor").unwrap();
        assert_eq!(r.session, "auth-refactor");
        assert!(r.file.is_none());
    }

    #[test]
    fn parse_ref_with_file() {
        let r = parse_ref("auth-refactor/spec.md").unwrap();
        assert_eq!(r.session, "auth-refactor");
        assert_eq!(r.file.as_deref(), Some("spec.md"));
    }

    #[test]
    fn parse_ref_strips_sp_prefix() {
        let r = parse_ref("sp://auth-refactor/logs/deploy.log").unwrap();
        assert_eq!(r.session, "auth-refactor");
        assert_eq!(r.file.as_deref(), Some("logs/deploy.log"));
    }

    #[test]
    fn parse_ref_rejects_empty() {
        assert!(parse_ref("").is_err());
        assert!(parse_ref("/file.md").is_err());
    }

    #[test]
    fn parse_duration_supports_units() {
        assert!(parse_duration_spec("3d").is_ok());
        assert!(parse_duration_spec("2w").is_ok());
        assert!(parse_duration_spec("12h").is_ok());
        assert!(parse_duration_spec("2026-01-15").is_ok());
        assert!(parse_duration_spec("3x").is_err());
    }

    #[test]
    fn truncate_appends_ellipsis() {
        assert_eq!(truncate("hello-world", 5), "hell…");
        assert_eq!(truncate("short", 10), "short");
    }
}
