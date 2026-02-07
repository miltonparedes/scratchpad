use clap::{Parser, Subcommand};

use crate::models::Agent;

#[derive(Parser)]
#[command(name = "sp")]
#[command(about = "Minimal TUI for organizing agent work sessions")]
#[command(version)]
pub struct Cli {
    /// Force user context (~/.scratchpad)
    #[arg(short = 'u', long)]
    pub user: bool,

    /// Force project context (.scratchpad/)
    #[arg(short = 'p', long)]
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
        name: Option<String>,
    },

    /// Run an agent in the session context
    #[command(alias = "r")]
    Run {
        /// Session name (can be prefix)
        name: Option<String>,
        /// Agent to use (claude or codex)
        #[arg(short, long)]
        agent: Option<Agent>,
    },

    /// View session entry point in external app
    View {
        /// Session name (can be prefix)
        name: Option<String>,
    },

    /// Edit session entry point in editor
    Edit {
        /// Session name (can be prefix)
        name: Option<String>,
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
        current: Option<String>,
        /// New session name
        new_name: String,
    },

    /// Print session directory path
    Path {
        /// Session name (can be prefix)
        name: Option<String>,
    },

    /// Open session folder in file manager
    #[command(alias = "f")]
    Folder {
        /// Session name (can be prefix)
        name: Option<String>,
    },

    /// Show file tree for a session
    Files {
        /// Session name (can be prefix)
        name: Option<String>,
        /// Output flat list (no tree chars, for piping)
        #[arg(long)]
        flat: bool,
    },

    /// Read session entry point or a specific file
    #[command(alias = "cat")]
    Read {
        /// Session name (can be prefix)
        name: Option<String>,
        /// Specific file to read (relative to session dir)
        file: Option<String>,
    },

    /// Write stdin to session entry point or a specific file
    Write {
        /// Session name
        name: String,
        /// Specific file to write (relative to session dir, default: notes.md)
        file: Option<String>,
    },

    /// Delete a session
    #[command(alias = "rm")]
    Delete {
        /// Session name (can be prefix)
        name: String,
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },

    /// Show active context and workspace path
    Context,

    /// Sync sessions with server (not yet implemented)
    Sync,
}
