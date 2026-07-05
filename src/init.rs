use crate::{LOCKFILE_NAME, TOOL_NAME};
use clap::ValueEnum;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CiProvider {
    Github,
    Gitlab,
    None,
}

const AGENT_GUIDE: &str = r#"# Driftless Agent Guide

Use Driftless to keep Markdown docs linked to code while you work.

## Ref syntax

- Inline: `src/lib.rs#symbol_name`
- Link: `[symbol](src/lib.rs#symbol_name)`
- Fenced code info string: `rust ref=src/lib.rs#symbol_name`

Supported symbols include functions, classes/types, methods, modules, and Go receiver methods such as `src/auth.go#User.Login`.

## Agent loop

1. When code changes, run `driftless check --json`.
2. For each JSON record, review the doc section and current `symbol_source`.
3. Patch docs when they are stale. Ask for human review when the code/doc relationship is ambiguous.
4. After docs are reviewed, run `driftless update`.
5. Run `driftless check` before handing off.

## Required checks

```sh
driftless check
driftless check --json
driftless coverage --include src/
```

Commit `.driftless.lock` with docs. Do not run `driftless update` to silence failures before the docs have been reviewed.
"#;

const GITHUB_WORKFLOW: &str = r#"name: Driftless

on:
  push:
  pull_request:

permissions:
  contents: read

jobs:
  docs:
    name: Documentation drift
    runs-on: ubuntu-latest

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Install Driftless
        run: cargo install driftless --locked

      - name: Check documented refs
        run: driftless check

      - name: Check public symbol coverage
        run: driftless coverage --include src/
"#;

const GITLAB_WORKFLOW: &str = r#"stages:
  - test

driftless:
  stage: test
  image: rust:latest
  before_script:
    - cargo install driftless --locked
  script:
    - driftless check
    - driftless coverage --include src/
"#;

pub(crate) fn print_prompt(ci: CiProvider) {
    let ci_arg = match ci {
        CiProvider::Github => "github",
        CiProvider::Gitlab => "gitlab",
        CiProvider::None => "none",
    };
    let ci_note = match ci {
        CiProvider::Github => "Use the GitHub Actions scaffold.",
        CiProvider::Gitlab => "Use the GitLab CI scaffold.",
        CiProvider::None => "Do not write a CI scaffold.",
    };
    println!(
        r#"Set up Driftless in this repository.

Goal: keep Markdown docs linked to source symbols so documentation drift is caught while code is being written.

CI target: `{ci_arg}`. {ci_note}

Steps:
1. Inspect this repository's docs, source layout, and CI provider.
2. Run `driftless init --print --ci {ci_arg}` to view the recommended AGENTS.md and CI scaffold.
3. If the scaffold fits, run `driftless init --ci {ci_arg}`. Use `--force` only when intentionally replacing generated files.
4. Add Markdown refs to important docs using inline refs like `src/lib.rs#symbol`, links like `[symbol](src/lib.rs#symbol)`, or fenced-code info strings like `rust ref=src/lib.rs#symbol`.
5. Run `driftless update` after the docs have been reviewed.
6. Run `driftless check`, `driftless check --json`, and `driftless coverage --include src/`.
7. Commit `.driftless.lock` with the docs.

Important: do not run `driftless update` merely to silence failures. Update docs first, then refresh the lockfile.
"#
    );
}

pub(crate) fn print_scaffold(ci: CiProvider) {
    println!("# {TOOL_NAME} project scaffold\n");
    println!("## AGENTS.md\n\n{AGENT_GUIDE}");
    if let Some((path, contents)) = ci_file(ci) {
        println!("## {path}\n\n```yaml\n{contents}```");
    }
}

pub(crate) fn run_init(root: &Path, print: bool, prompt: bool, force: bool, ci: CiProvider) -> i32 {
    if prompt {
        print_prompt(ci);
        return 0;
    }
    if print {
        print_scaffold(ci);
        return 0;
    }

    let mut failures = 0usize;
    if write_file(root, "AGENTS.md", AGENT_GUIDE, force).is_err() {
        failures += 1;
    }
    if let Some((path, contents)) = ci_file(ci) {
        if write_file(root, path, contents, force).is_err() {
            failures += 1;
        }
    }

    if failures == 0 {
        let ci_suffix = match ci {
            CiProvider::Github => " and GitHub CI",
            CiProvider::Gitlab => " and GitLab CI",
            CiProvider::None => "",
        };
        eprintln!(
            "{}: initialized agent guide{}. Next: add refs, run `{} update`, commit {}.",
            TOOL_NAME, ci_suffix, TOOL_NAME, LOCKFILE_NAME
        );
        0
    } else {
        eprintln!(
            "{}: {} file(s) already exist; use `{} init --print` to copy snippets or `{} init --force` to overwrite",
            TOOL_NAME, failures, TOOL_NAME, TOOL_NAME
        );
        1
    }
}

fn ci_file(ci: CiProvider) -> Option<(&'static str, &'static str)> {
    match ci {
        CiProvider::Github => Some((".github/workflows/driftless.yml", GITHUB_WORKFLOW)),
        CiProvider::Gitlab => Some((".gitlab-ci.yml", GITLAB_WORKFLOW)),
        CiProvider::None => None,
    }
}

fn write_file(root: &Path, rel: &str, contents: &str, force: bool) -> std::io::Result<()> {
    let path = root.join(rel);
    if path.exists() && !force {
        eprintln!("skip     {} already exists", rel);
        return Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "file exists",
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    eprintln!("wrote    {rel}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_mentions_core_commands() {
        assert!(AGENT_GUIDE.contains("driftless check --json"));
        assert!(AGENT_GUIDE.contains("driftless update"));
        assert!(GITHUB_WORKFLOW.contains("driftless coverage --include src/"));
        assert!(GITLAB_WORKFLOW.contains("driftless coverage --include src/"));
    }
}
