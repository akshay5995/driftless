use crate::TOOL_NAME;

const SETUP_PROMPT: &str = r#"Set up Driftless in this repository.

Goal: keep Markdown docs linked to source symbols so documentation drift is caught while code is being written.

Steps:
1. Inspect this repository's docs and source layout.
2. Add Markdown refs to important docs using inline refs like `src/lib.rs#symbol`, links like `[symbol](src/lib.rs#symbol)`, or fenced-code info strings like `rust ref=src/lib.rs#symbol`.
3. Run `driftless update` after the docs have been reviewed.
4. When code changes, run `driftless check --json` and use each record's doc section plus current `symbol_source` to repair stale prose.
5. Run `driftless check` and `driftless coverage --include src/`.
6. Commit .driftless.lock with the docs.

Important: do not run `driftless update` merely to silence failures. Update docs first, then refresh the lockfile.
"#;

pub(crate) fn print_prompt() {
    println!("{SETUP_PROMPT}");
}

pub(crate) fn run_init() -> i32 {
    print_prompt();
    eprintln!("{TOOL_NAME}: printed setup prompt; no files were written.");
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_mentions_core_commands_without_ci_scaffold() {
        let _ = print_prompt as fn();
        let prompt = SETUP_PROMPT;
        assert!(prompt.contains("driftless check --json"));
        assert!(prompt.contains("driftless update"));
        assert!(!prompt.contains("GitHub Actions scaffold"));
        assert!(!prompt.contains("GitLab CI scaffold"));
        assert!(crate::LOCKFILE_NAME.ends_with(".lock"));
    }
}
