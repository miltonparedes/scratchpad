mod cli;
mod markdown;
mod models;
mod open;
mod storage;
mod tui;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, Command};
use models::Session;
use open::open_path_blocking;
use storage::{load_config, Storage};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config()?;
    let storage = Storage::new(config.clone());

    storage.ensure_workspace()?;

    match cli.command {
        None => {
            tui::run(config, None)?;
        }
        Some(Command::New { title, tags }) => {
            let tags = tags
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();
            let session = Session::new(title).with_tags(tags);
            storage.create_session(&session, None)?;
            println!("Created session: {} ({})", session.title, session.id);
        }
        Some(Command::Quick { text, title }) => {
            let title = title.unwrap_or_else(|| {
                text.chars().take(50).collect::<String>()
                    + if text.len() > 50 { "..." } else { "" }
            });
            let session = Session::new(title);
            storage.create_session(&session, Some(&text))?;
            println!("Created quick session: {} ({})", session.title, session.id);
        }
        Some(Command::Open { id }) => {
            tui::run(config, Some(&id))?;
        }
        Some(Command::Run { id, agent }) => {
            match storage.find_session_by_prefix(&id)? {
                Some(session) => {
                    let agent = agent.unwrap_or(config.default_agent);
                    let session_dir = storage.session_dir(&session.id);
                    println!("Running {} in session: {}", agent, session.title);

                    let status = std::process::Command::new(agent.command())
                        .current_dir(&session_dir)
                        .status()?;

                    if !status.success() {
                        std::process::exit(status.code().unwrap_or(1));
                    }
                }
                None => {
                    eprintln!("Session not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        Some(Command::View { id }) => {
            match storage.find_session_by_prefix(&id)? {
                Some(session) => {
                    let notes_path = storage.session_notes_path(&session.id);
                    open_path_blocking(&notes_path, config.viewer.as_deref())?;
                }
                None => {
                    eprintln!("Session not found: {}", id);
                    std::process::exit(1);
                }
            }
        }
        Some(Command::List) => {
            let sessions = storage.list_sessions()?;
            if sessions.is_empty() {
                println!("No sessions found.");
            } else {
                println!("{:<36}  {:<30}  {}", "ID", "TITLE", "UPDATED");
                println!("{}", "-".repeat(80));
                for session in sessions {
                    let id_short = session.id.to_string()[..8].to_string();
                    let title = if session.title.len() > 30 {
                        format!("{}...", &session.title[..27])
                    } else {
                        session.title.clone()
                    };
                    println!(
                        "{:<36}  {:<30}  {}",
                        id_short,
                        title,
                        session.updated_at.format("%Y-%m-%d %H:%M")
                    );
                }
            }
        }
    }

    Ok(())
}
