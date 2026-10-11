use clap::{Args, Parser, Subcommand, ValueEnum};
#[derive(Parser)]
#[command(
    name = "clipchain-cli",
    version,
    about = "Headless clipboard transformation and pipeline management"
)]
pub(super) struct Cli {
    /// Use an isolated or custom SQLite database.
    #[arg(long, global = true, value_name = "PATH")]
    pub db: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub(super) enum Command {
    /// Transform one UTF-8 payload from TEXT or stdin (up to 1 MiB).
    Transform {
        text: Option<String>,
        /// Output a JSON preview report; never writes history or clipboard.
        #[arg(long)]
        dry_run: bool,
        /// When --type is supplied, use this draft instead of saved rules.
        #[command(flatten)]
        draft: Draft,
    },
    /// Transform the current clipboard once and record history when enabled.
    Clip,
    /// Watch the clipboard every 250 ms until Ctrl+C (or SIGTERM on Unix).
    Daemon,
    /// List, preview, save, delete, or reorder pipeline rules.
    Rules {
        #[command(subcommand)]
        command: RulesCommand,
    },
    /// Inspect or clear retained clipboard transformations.
    History {
        #[command(subcommand)]
        command: HistoryCommand,
    },
    /// Report clipboard owner health, rule counts, settings, and database path.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Read or change monitoring and history settings; startup uses the desktop UI.
    Settings {
        #[command(subcommand)]
        command: SettingsCommand,
    },
}

