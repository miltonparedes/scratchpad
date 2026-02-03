use clap::{Parser, Subcommand};

use crate::models::Agent;

#[derive(Parser)]
#[command(name = "sp")]
#[command(about = "Minimal TUI for organizing agent work sessions")]
#[command(version)]
pub struct Cli {
    /// Force user context (~/.scratchpad)
    #[arg(long)]
    pub user: bool,

    /// Force project context (.scratchpad/)
    #[arg(long)]
    pub project: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new session
    #[command(alias = "n")]
    New {
        /// Session name (slug). If not provided, one will be generated.
        name: Option<String>,
    },

    /// Create a quick session with initial note
    #[command(alias = "q")]
    Quick {
        /// Initial note text
        text: String,
    },

    /// Open a session in TUI
    #[command(alias = "o")]
    Open {
        /// Session name (can be prefix)
        name: String,
    },

    /// Run an agent in the session context
    #[command(alias = "r")]
    Run {
        /// Session name (can be prefix)
        name: String,
        /// Agent to use (claude or codex)
        #[arg(short, long)]
        agent: Option<Agent>,
    },

    /// View session entry point in external app
    View {
        /// Session name (can be prefix)
        name: String,
    },

    /// Edit session entry point in editor
    Edit {
        /// Session name (can be prefix)
        name: String,
    },

    /// List all sessions
    #[command(alias = "ls")]
    List,

    /// Initialize a project-local scratchpad
    Init {
        /// Add to .gitignore (otherwise prompts)
        #[arg(long)]
        gitignore: bool,

        /// Add to .git/info/exclude (otherwise prompts)
        #[arg(long)]
        exclude: bool,
    },

    /// Rename a session
    Rename {
        /// Current session name (or prefix)
        current: String,
        /// New session name
        new_name: String,
    },

    /// Sync sessions with server (not yet implemented)
    Sync,
}
