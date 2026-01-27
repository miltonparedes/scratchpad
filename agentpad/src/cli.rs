use clap::{Parser, Subcommand};

use crate::models::Agent;

#[derive(Parser)]
#[command(name = "ap")]
#[command(about = "Minimal TUI for organizing agent work sessions")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new session
    #[command(alias = "n")]
    New {
        /// Session title
        title: String,
        /// Tags (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// Create a quick session with initial note
    #[command(alias = "q")]
    Quick {
        /// Initial note text
        text: String,
        /// Session title (optional, derived from text if not provided)
        #[arg(short, long)]
        title: Option<String>,
    },

    /// Open a session in TUI
    #[command(alias = "o")]
    Open {
        /// Session ID (can be prefix)
        id: String,
    },

    /// Run an agent in the session context
    #[command(alias = "r")]
    Run {
        /// Session ID (can be prefix)
        id: String,
        /// Agent to use (claude or codex)
        #[arg(short, long)]
        agent: Option<Agent>,
    },

    /// View notes.md in external app
    View {
        /// Session ID (can be prefix)
        id: String,
    },

    /// List all sessions
    #[command(alias = "ls")]
    List,
}
