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
