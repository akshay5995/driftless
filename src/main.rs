#![forbid(unsafe_code)]

mod check;
mod coverage;
mod init;
mod lockfile;
mod lsp;
mod refs;
mod resolve;

use clap::{Parser as ClapParser, Subcommand};
use init::CiProvider;
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
    about = "Keep markdown docs linked to code so drift is caught while you work",
    after_help = "Quick start:
  driftless init --prompt    Print an agent setup prompt
  driftless init --print     Print AGENTS.md and CI snippets
  driftless init --ci gitlab Write AGENTS.md and .gitlab-ci.yml
  driftless update           Review docs, then lock current refs
  driftless check --json     Get machine-readable drift records

Ref examples:
  `src/lib.rs#login`
  [login](src/lib.rs#login)
  fenced code info: rust ref=src/lib.rs#login"
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
    /// Verify all refs; non-zero exit on any error
    Check {
        /// Demote body-only drift to a warning (signature drift still fails)
        #[arg(long)]
        warn_body: bool,
        /// Emit machine-readable JSON records (for agent loops)
        #[arg(long)]
        json: bool,
    },
    /// Fail if any public symbol lacks a doc reference
    Coverage {
        /// Restrict to path prefixes, e.g. --include src/
        #[arg(long)]
        include: Vec<String>,
        /// Emit machine-readable JSON records
        #[arg(long)]
        json: bool,
    },
    /// Write/refresh .driftless.lock with current hashes
    Update,
    /// Scaffold AGENTS.md and CI setup for a project
    Init {
        /// Print the agent guide and CI snippets instead of writing files
        #[arg(long)]
        print: bool,
        /// Print a copyable prompt for an agent to set up Driftless
        #[arg(long)]
        prompt: bool,
        /// Overwrite generated files if they already exist
        #[arg(long)]
        force: bool,
        /// CI scaffold to print or write
        #[arg(long, value_enum, default_value_t = CiProvider::Github)]
        ci: CiProvider,
    },
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
        Cmd::Coverage { include, json } => {
            std::process::exit(coverage::run_coverage(&root, &include, json))
        }
        Cmd::Update => std::process::exit(check::run_check(&root, true, false, false)),
        Cmd::Init {
            print,
            prompt,
            force,
            ci,
        } => std::process::exit(init::run_init(&root, print, prompt, force, ci)),
        Cmd::Lsp => {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(lsp::run());
        }
    }
}
