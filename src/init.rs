use crate::TOOL_NAME;

const SETUP_PROMPT: &str = include_str!("../docs/setup-prompt.txt");

pub(crate) fn print_prompt() {
    print!("{SETUP_PROMPT}");
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
        assert!(prompt.contains("public APIs, commands, config keys"));
        assert!(prompt.contains("Prefer maintainer docs"));
        assert!(prompt.contains("driftless check --json"));
        assert!(prompt.contains("driftless update"));
        assert!(!prompt.contains("GitHub Actions scaffold"));
        assert!(!prompt.contains("GitLab CI scaffold"));
        assert!(crate::LOCKFILE_NAME.ends_with(".lock"));
    }
}
