use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sp")]
#[command(about = "Workspace manager for human + agent sessions")]
#[command(version)]
pub struct Cli {
    /// Override active project (skips git autodetection)
    #[arg(short = 'P', long, global = true)]
    pub project: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create a new session
    #[command(alias = "n")]
    New {
        name: Option<String>,
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
        #[arg(long)]
        json: bool,
    },

    /// Create a quick session with initial note
    #[command(alias = "q")]
    Quick {
        text: String,
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
        #[arg(long)]
        json: bool,
    },

    /// Open a session in TUI
    #[command(alias = "o")]
    Open { name: Option<String> },

    /// Run an agent in the session context
    #[command(alias = "r")]
    Run {
        name: Option<String>,
        #[arg(short, long)]
        agent: Option<String>,
    },

    /// View session entry point in external app
    View { name: Option<String> },

    /// Edit session entry point in editor
    Edit { name: Option<String> },

    /// List sessions in the active project (or all with --all)
    #[command(alias = "ls")]
    List {
        #[arg(long)]
        all: bool,
        #[arg(long)]
        shared: bool,
        #[arg(long)]
        today: bool,
        #[arg(long, value_name = "SPEC")]
        since: Option<String>,
        #[arg(long, value_name = "DATE")]
        before: Option<String>,
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
        #[arg(long, value_name = "STATUS")]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Search sessions by name and content
    Search {
        query: String,
        #[arg(long)]
        all: bool,
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
        #[arg(long, value_name = "STATUS")]
        status: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Show the last (most recently modified) artifact in the active project
    Last {
        /// Number of artifacts to return
        #[arg(short = 'n', long, default_value = "1")]
        count: usize,
        /// Search within a specific session
        #[arg(long, value_name = "SESSION")]
        r#in: Option<String>,
        /// Look across all projects
        #[arg(long)]
        all: bool,
        /// Print only the session slug
        #[arg(long)]
        session_only: bool,
        /// Print as URI-style sp://session/file
        #[arg(long)]
        uri: bool,
        /// Print absolute path (default)
        #[arg(long)]
        path: bool,
        #[arg(long)]
        json: bool,
    },

    /// Read session entry point or a specific file to stdout
    #[command(alias = "cat")]
    Read {
        name: Option<String>,
        file: Option<String>,
    },

    /// Print absolute path for a session or file reference
    Resolve {
        #[arg(value_name = "REF")]
        reference: String,
    },

    /// Write stdin to a session file
    Write {
        #[arg(value_name = "REF")]
        reference: String,
        /// Optional explicit file (alternative to ref form session/file)
        file: Option<String>,
        /// Expected revision (exit 4 if mismatch)
        #[arg(long)]
        expect_revision: Option<u64>,
        #[arg(long)]
        json: bool,
    },

    /// Append stdin to a session file
    Append {
        #[arg(value_name = "REF")]
        reference: String,
        file: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Copy a local file into a session
    Attach {
        session: String,
        source: String,
        #[arg(long = "as", value_name = "NAME")]
        as_name: Option<String>,
        #[arg(long)]
        json: bool,
    },

    /// Show file tree for a session
    Files {
        name: Option<String>,
        #[arg(long)]
        flat: bool,
        #[arg(long)]
        json: bool,
    },

    /// Print session directory path
    Path { name: Option<String> },

    /// Open session folder in file manager
    #[command(alias = "f")]
    Folder { name: Option<String> },

    /// Rename a session
    Rename {
        current: Option<String>,
        new_name: String,
    },

    /// Archive a session (sets status = archived)
    Archive { name: String },

    /// Restore an archived session (sets status = active)
    Restore { name: String },

    /// Add or remove tags from a session
    Tag {
        session: String,
        /// Tag changes: bare name adds, +name adds, -name removes
        changes: Vec<String>,
        #[arg(long)]
        json: bool,
    },

    /// Delete a session
    #[command(alias = "rm")]
    Delete {
        name: String,
        #[arg(long)]
        yes: bool,
    },

    /// Show active project, source, and workspace location
    Context,

    /// Manage projects and aliases
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },

    /// Print path for a session or file ref (alias: print path to stdout, optionally copy)
    Link {
        #[arg(value_name = "REF")]
        reference: String,
        #[arg(long)]
        copy: bool,
    },

    /// Manage configuration
    #[command(alias = "cfg")]
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Internal: hook handler for agent integrations
    #[command(hide = true)]
    Hook { name: String },

    /// Sync sessions with server (not yet implemented)
    Sync,
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// Show the active project, its source, and stats
    Current {
        #[arg(long)]
        json: bool,
    },
    /// List configured aliases and detected projects on disk
    List {
        #[arg(long)]
        json: bool,
    },
    /// Save the auto-detected project to config under a custom name
    Save {
        #[arg(long = "as", value_name = "NAME")]
        as_name: Option<String>,
    },
    /// Link the current repo (or a specific repo) to a named project alias
    Link {
        name: String,
        /// Repo identifier (owner/repo, host/owner/repo, or remote URL). Defaults to current repo.
        repo: Option<String>,
    },
    /// Rename a project (moves directory + updates aliases)
    Rename { old: String, new: String },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Create default config file with documentation
    Init {
        #[arg(long)]
        force: bool,
    },
    Path,
    Show,
    Edit,
}
