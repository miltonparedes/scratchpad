mod cli;
mod markdown;
mod models;
mod names;
mod open;
mod storage;
mod tui;

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};
use models::{Context, Session};
use names::{generate_session_name, slugify, slugify_or_generate};
use open::{open_folder, open_path_blocking, open_with_editor};
use storage::{available_contexts, detect_context, load_config, Storage};

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
                std::process::exit(1);
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
            println!("Created session: {}", slug);
            println!("  {}", storage.session_dir(&slug).display());
        }
        Some(Command::Quick { text }) => {
            let existing = storage.existing_slugs()?;
            let slug = generate_session_name(&existing, &config);
            let session = Session::new(&slug);
            storage.create_session(&session, Some(&text))?;
            println!("Created quick session: {}", slug);
            println!("  {}", storage.session_dir(&slug).display());
        }
        Some(Command::Open { name }) => {
            let contexts = available_contexts(&cwd, &config);
            tui::run(config, context, contexts, Some(&name))?;
        }
        Some(Command::Run { name, agent }) => {
            match storage.find_session_by_name(&name)? {
                Some(session) => {
                    let agent = agent.unwrap_or(config.default_agent);
                    let session_dir = storage.session_dir(&session.slug);
                    println!("Running {} in session: {}", agent, session.display_title());

                    let status = std::process::Command::new(agent.command())
                        .current_dir(&session_dir)
                        .status()?;

                    if !status.success() {
                        std::process::exit(status.code().unwrap_or(1));
                    }
                }
                None => {
                    eprintln!("Session not found: {}", name);
                    std::process::exit(1);
                }
            }
        }
        Some(Command::View { name }) => {
            match storage.find_session_by_name(&name)? {
                Some(session) => {
                    let session_dir = storage.session_dir(&session.slug);
                    if let Some(entry_point) = storage.find_entry_point(&session.slug) {
                        open_path_blocking(&entry_point, config.viewer.as_deref())?;
                    } else {
                        // No entry point, open the folder
                        open_folder(&session_dir)?;
                    }
                }
                None => {
                    eprintln!("Session not found: {}", name);
                    std::process::exit(1);
                }
            }
        }
        Some(Command::Edit { name }) => {
            match storage.find_session_by_name(&name)? {
                Some(session) => {
                    let session_dir = storage.session_dir(&session.slug);
                    if let Some(entry_point) = storage.find_entry_point(&session.slug) {
                        open_with_editor(&entry_point, config.editor.as_deref())?;
                    } else {
                        // No entry point, create notes.md and open it
                        let notes_path = session_dir.join("notes.md");
                        if !notes_path.exists() {
                            fs::write(&notes_path, "")?;
                        }
                        open_with_editor(&notes_path, config.editor.as_deref())?;
                    }
                }
                None => {
                    eprintln!("Session not found: {}", name);
                    std::process::exit(1);
                }
            }
        }
        Some(Command::List) => {
            let sessions = storage.list_sessions()?;
            if sessions.is_empty() {
                println!("No sessions found.");
            } else {
                let context_label = match &context {
                    Context::User => "User".to_string(),
                    Context::Project(_) => format!("Project: {}", context.display_name()),
                };
                println!("[{}]", context_label);
                println!("{:<25}  {}", "NAME", "UPDATED");
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
            }
        }
        Some(Command::Init { gitignore, exclude }) => {
            handle_init(gitignore, exclude)?;
        }
        Some(Command::Rename { current, new_name }) => {
            match storage.find_session_by_name(&current)? {
                Some(session) => {
                    let new_slug = match slugify(&new_name) {
                        Some(s) => s,
                        None => {
                            eprintln!("Invalid session name: '{}'", new_name);
                            std::process::exit(1);
                        }
                    };
                    storage.rename_session(&session.slug, &new_slug)?;
                    println!("Renamed '{}' to '{}'", session.slug, new_slug);
                }
                None => {
                    eprintln!("Session not found: {}", current);
                    std::process::exit(1);
                }
            }
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
            writeln!(file, "{}", entry)?;
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
                    writeln!(file, "{}", entry)?;
                    println!("Added .scratchpad/ to .git/info/exclude");
                }
            } else {
                println!("Warning: .git/info/ not found, skipping ignore");
            }
        }
    }

    Ok(())
}
