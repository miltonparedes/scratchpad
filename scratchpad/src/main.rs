mod cli;
mod hook;
mod markdown;
mod models;
mod names;
mod open;
mod storage;
mod tui;

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use std::process;

use anyhow::{Context as _, Result};
use clap::Parser;

use cli::{Cli, Command};
use models::{Context, Session};
use names::{generate_session_name, slugify, slugify_or_generate};
use open::{open_folder, open_path_blocking, open_with_editor};
use storage::{Storage, available_contexts, build_file_tree, detect_context, load_config};

fn pick_session_fzf(storage: &Storage) -> Result<Session> {
    let sessions = storage.list_sessions()?;
    if sessions.is_empty() {
        eprintln!("No sessions found.");
        process::exit(1);
    }

    let input: String = sessions.iter().map(|s| format!("{}\n", s.slug)).collect();

    let workspace = storage.workspace_path();
    let ws = workspace.display();
    let preview_cmd = format!("ls -1 {ws}/{{}}/");

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
        .spawn()
        .inspect_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                eprintln!("fzf not found. Install fzf or provide a session name.");
                process::exit(1);
            }
        })?;

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
            process::exit(1);
        }
    }
}

fn resolve_session(storage: &Storage, name: Option<String>) -> Result<Session> {
    match name {
        Some(n) => match storage.find_session_by_name(&n)? {
            Some(session) => Ok(session),
            None => {
                eprintln!("Session not found: {n}");
                process::exit(1);
            }
        },
        None => pick_session_fzf(storage),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config()?;

    // Determine context based on flags or auto-detection
    let cwd = std::env::current_dir().unwrap_or_default();
    let context = if cli.user {
        Context::User
    } else if cli.project {
        // Find or error if no project context
        let contexts = available_contexts(&cwd, &config);
        contexts
            .into_iter()
            .find(|c| matches!(c, Context::Project(_)))
            .unwrap_or_else(|| {
                eprintln!("No .scratchpad/ found in current directory or parents.");
                eprintln!("Run 'sp init' to create one.");
                process::exit(1);
            })
    } else {
        detect_context(&cwd, &config)
    };

    let storage = Storage::new(config.clone(), context.clone());
    storage.ensure_workspace()?;

    match cli.command {
        None => {
            let contexts = available_contexts(&cwd, &config);
            tui::run(config, context, contexts, None)?;
        }
        Some(Command::New { name }) => {
            let existing = storage.existing_slugs()?;
            let slug = match name {
                Some(n) => slugify_or_generate(&n, &existing, &config),
                None => generate_session_name(&existing, &config),
            };
            let session = Session::new(&slug);
            storage.create_session(&session, None)?;
            println!("Created session: {slug}");
            println!("  {}", storage.session_dir(&slug).display());
        }
        Some(Command::Quick { text }) => {
            let existing = storage.existing_slugs()?;
            let slug = generate_session_name(&existing, &config);
            let session = Session::new(&slug);
            storage.create_session(&session, Some(&text))?;
            println!("Created quick session: {slug}");
            println!("  {}", storage.session_dir(&slug).display());
        }
        Some(Command::Open { name }) => {
            let session = resolve_session(&storage, name)?;
            let contexts = available_contexts(&cwd, &config);
            tui::run(config, context, contexts, Some(&session.slug))?;
        }
        Some(Command::Run { name, agent }) => {
            let session = resolve_session(&storage, name)?;
            let agent = agent.unwrap_or(config.default_agent);
            let session_dir = storage.session_dir(&session.slug);
            let context_label = match &context {
                Context::User => "user",
                Context::Project(_) => "project",
            };
            println!("Running {agent} in session: {}", session.display_title());

            let status = process::Command::new(agent.command())
                .current_dir(&session_dir)
                .env("SP_SESSION", &session.slug)
                .env("SP_CONTEXT", context_label)
                .env("SP_WORKSPACE", storage.workspace_path())
                .status()?;

            if !status.success() {
                process::exit(status.code().unwrap_or(1));
            }
        }
        Some(Command::View { name }) => {
            let session = resolve_session(&storage, name)?;
            let session_dir = storage.session_dir(&session.slug);
            if let Some(entry_point) = storage.find_entry_point(&session.slug) {
                open_path_blocking(&entry_point, config.viewer.as_deref())?;
            } else {
                open_folder(&session_dir)?;
            }
        }
        Some(Command::Edit { name }) => {
            let session = resolve_session(&storage, name)?;
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
        }
        Some(Command::List) => {
            let sessions = storage.list_sessions()?;
            if sessions.is_empty() {
                eprintln!("No sessions found.");
            } else if io::stdout().is_terminal() {
                let context_label = match &context {
                    Context::User => "User".to_string(),
                    Context::Project(_) => format!("Project: {}", context.display_name()),
                };
                println!("[{context_label}]");
                println!("{:<25}  UPDATED", "NAME");
                println!("{}", "-".repeat(50));
                for session in sessions {
                    let name = if session.slug.len() > 25 {
                        format!("{}...", &session.slug[..22])
                    } else {
                        session.slug.clone()
                    };
                    println!(
                        "{:<25}  {}",
                        name,
                        session.updated_at.format("%Y-%m-%d %H:%M")
                    );
                }
            } else {
                for session in sessions {
                    println!("{}\t{}", session.slug, session.updated_at.to_rfc3339());
                }
            }
        }
        Some(Command::Init { gitignore, exclude }) => {
            handle_init(gitignore, exclude)?;
        }
        Some(Command::Rename { current, new_name }) => {
            let session = resolve_session(&storage, current)?;
            let new_slug = match slugify(&new_name) {
                Some(s) => s,
                None => {
                    eprintln!("Invalid session name: '{new_name}'");
                    process::exit(1);
                }
            };
            storage.rename_session(&session.slug, &new_slug)?;
            println!("Renamed '{}' to '{new_slug}'", session.slug);
        }
        Some(Command::Path { name }) => {
            let session = resolve_session(&storage, name)?;
            print!("{}", storage.session_dir(&session.slug).display());
        }
        Some(Command::Folder { name }) => {
            let session = resolve_session(&storage, name)?;
            let session_dir = storage.session_dir(&session.slug);
            open_folder(&session_dir)?;
        }
        Some(Command::Files { name, flat }) => {
            let session = resolve_session(&storage, name)?;
            let session_dir = storage.session_dir(&session.slug);
            let entry_point = storage.find_entry_point(&session.slug);
            let tree = build_file_tree(&session_dir, entry_point.as_deref(), 3);

            if flat || !io::stdout().is_terminal() {
                print_file_tree_flat(&tree);
            } else {
                println!("{}/", session.slug);
                print_file_tree_ansi(&tree);
            }
        }
        Some(Command::Read { name, file }) => {
            let session = resolve_session(&storage, name)?;
            let content = match file {
                Some(f) => {
                    let path = storage.session_dir(&session.slug).join(&f);
                    fs::read_to_string(&path).with_context(|| format!("Failed to read {f}"))?
                }
                None => storage.read_notes(&session.slug)?,
            };
            print!("{content}");
        }
        Some(Command::Write { name, file }) => {
            let session = resolve_session(&storage, Some(name))?;
            let mut content = String::new();
            io::stdin().read_to_string(&mut content)?;
            match file {
                Some(f) => {
                    let path = storage.session_dir(&session.slug).join(&f);
                    fs::write(&path, &content).with_context(|| format!("Failed to write {f}"))?;
                }
                None => storage.write_notes(&session.slug, &content)?,
            };
        }
        Some(Command::Delete { name, yes }) => {
            let session = resolve_session(&storage, Some(name))?;
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
        }
        Some(Command::Context) => match &context {
            Context::User => {
                println!("user\t{}", storage.workspace_path().display());
            }
            Context::Project(_) => {
                println!("project\t{}", storage.workspace_path().display());
            }
        },
        Some(Command::Hook { name }) => {
            hook::handle(&name)?;
        }
        Some(Command::Sync) => {
            println!("Sync not yet implemented.");
            println!("Configure server in ~/.config/scratchpad/config.toml");
        }
    }

    Ok(())
}

fn handle_init(gitignore: bool, exclude: bool) -> Result<()> {
    // 1. Create .scratchpad/ directory
    let scratchpad_dir = Path::new(".scratchpad");
    if scratchpad_dir.exists() {
        println!(".scratchpad/ already exists");
    } else {
        fs::create_dir_all(scratchpad_dir)?;
        println!("Created .scratchpad/");
    }

    // 2. Determine ignore method
    let use_gitignore = if gitignore {
        true
    } else if exclude {
        false
    } else {
        // Interactive prompt
        println!("\nWhere should .scratchpad/ be ignored?");
        println!("  1) .gitignore (visible to collaborators)");
        println!("  2) .git/info/exclude (local only)");
        print!("\nChoice [1/2]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim() == "1"
    };

    // 3. Write ignore entry
    let entry = ".scratchpad/";
    if use_gitignore {
        let gitignore_path = Path::new(".gitignore");
        let existing = if gitignore_path.exists() {
            fs::read_to_string(gitignore_path)?
        } else {
            String::new()
        };

        if existing.lines().any(|l| l.trim() == entry) {
            println!(".scratchpad/ already in .gitignore");
        } else {
            let mut file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(gitignore_path)?;
            // Add newline if file doesn't end with one
            if !existing.is_empty() && !existing.ends_with('\n') {
                writeln!(file)?;
            }
            writeln!(file, "{entry}")?;
            println!("Added .scratchpad/ to .gitignore");
        }
    } else {
        let exclude_path = Path::new(".git/info/exclude");
        if let Some(parent) = exclude_path.parent() {
            if parent.exists() {
                let existing = if exclude_path.exists() {
                    fs::read_to_string(exclude_path)?
                } else {
                    String::new()
                };

                if existing.lines().any(|l| l.trim() == entry) {
                    println!(".scratchpad/ already in .git/info/exclude");
                } else {
                    let mut file = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(exclude_path)?;
                    if !existing.is_empty() && !existing.ends_with('\n') {
                        writeln!(file)?;
                    }
                    writeln!(file, "{entry}")?;
                    println!("Added .scratchpad/ to .git/info/exclude");
                }
            } else {
                println!("Warning: .git/info/ not found, skipping ignore");
            }
        }
    }

    Ok(())
}

fn file_type_ansi_color(name: &str, is_dir: bool) -> &'static str {
    if is_dir {
        return "\x1b[34m"; // Blue
    }
    match name.rsplit('.').next() {
        Some("md") => "\x1b[36m", // Cyan
        Some("rs" | "py" | "js" | "ts" | "go" | "rb" | "c" | "cpp" | "h" | "java" | "sh") => {
            "\x1b[32m" // Green
        }
        Some("toml" | "json" | "yaml" | "yml" | "xml" | "ini" | "env") => "\x1b[33m", // Yellow
        Some("png" | "jpg" | "jpeg" | "gif" | "svg" | "webp" | "ico") => "\x1b[35m",  // Magenta
        Some("log") => "\x1b[90m",                                                    // DarkGray
        _ => "\x1b[0m",                                                               // Reset/White
    }
}

fn print_file_tree_ansi(tree: &[models::FileTreeEntry]) {
    for entry in tree {
        let mut prefix = String::new();
        for &ancestor_last in &entry.ancestor_is_last {
            prefix.push_str(if ancestor_last {
                "    "
            } else {
                "\x1b[90m│\x1b[0m   "
            });
        }

        let connector = if entry.is_last {
            "└── "
        } else {
            "├── "
        };
        let color = file_type_ansi_color(&entry.name, entry.is_dir);
        let indicator = if entry.is_entry_point {
            "  \x1b[36m●\x1b[0m"
        } else {
            ""
        };

        println!(
            "{prefix}\x1b[90m{connector}\x1b[0m{color}{}{}\x1b[0m{indicator}",
            if entry.is_entry_point { "\x1b[1m" } else { "" },
            entry.name,
        );
    }
}

fn print_file_tree_flat(tree: &[models::FileTreeEntry]) {
    for entry in tree {
        if entry.is_dir {
            continue;
        }
        print_flat_path(tree, entry);
    }
}

fn print_flat_path(tree: &[models::FileTreeEntry], target: &models::FileTreeEntry) {
    let target_idx = tree.iter().position(|e| std::ptr::eq(e, target)).unwrap();
    let mut path_parts: Vec<&str> = vec![&target.name];

    let mut current_depth = target.depth;
    for entry in tree[..target_idx].iter().rev() {
        if entry.is_dir && entry.depth == current_depth - 1 {
            let dir_name = entry.name.trim_end_matches('/');
            path_parts.push(dir_name);
            if current_depth == 0 {
                break;
            }
            current_depth = entry.depth;
        }
    }

    path_parts.reverse();
    println!("{}", path_parts.join("/"));
}
