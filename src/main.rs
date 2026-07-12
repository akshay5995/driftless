#![forbid(unsafe_code)]

mod check;
mod init;
mod lockfile;
mod lsp;
mod refs;
mod resolve;

use clap::{Parser as ClapParser, Subcommand};
use std::path::PathBuf;

pub(crate) const TOOL_NAME: &str = "driftless";
pub(crate) const LOCKFILE_NAME: &str = ".driftless.lock";
pub(crate) const SOURCE_EXTENSIONS: [&str; 11] = [
    "go", "java", "js", "jsx", "kt", "kts", "py", "rb", "rs", "ts", "tsx",
];

#[derive(ClapParser)]
#[command(
    name = "driftless",
    version,
    about = "Catch stale Markdown docs by linking them to source symbols",
    after_help = "Agent loop:
  driftless init                              Print the setup prompt
  driftless check --json                      Inspect stale-doc records before editing
  driftless update                            Refresh .driftless.lock after docs are reviewed

Ref examples:
  Inline code: `src/lib.rs#login`
  Markdown link: [login](src/lib.rs#login)
  Fenced code info: rust ref=src/lib.rs#login"
)]
struct Cli {
    /// Project root (defaults to cwd)
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Verify Markdown refs against .driftless.lock
    Check {
        /// Demote body-only drift to a warning (signature drift still fails)
        #[arg(long)]
        warn_body: bool,
        /// Emit machine-readable JSON records (for agent loops)
        #[arg(long)]
        json: bool,
    },
    /// Refresh .driftless.lock after docs and source are reviewed
    Update,
    /// Print the copyable Driftless setup prompt
    Init,
    /// Run as LSP server over stdio
    Lsp,
}

fn main() {
    let cli = Cli::parse();
    let root = cli
        .root
        .or_else(|| std::env::current_dir().ok())
        .expect("cannot determine root");
    match cli.cmd {
        Cmd::Check { warn_body, json } => {
            std::process::exit(check::run_check(&root, false, warn_body, json))
        }
        Cmd::Update => std::process::exit(check::run_check(&root, true, false, false)),
        Cmd::Init => std::process::exit(init::run_init()),
        Cmd::Lsp => {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(lsp::run());
        }
    }
}
