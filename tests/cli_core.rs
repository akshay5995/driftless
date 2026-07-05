#[path = "common/core.rs"]
mod core;
#[path = "common/process.rs"]
mod process;

use core::rust_repo_with_doc;
use process::{driftless, text, write};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn version_flag_reports_package_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_driftless"))
        .arg("--version")
        .output()
        .expect("run driftless --version");

    assert!(output.status.success(), "stderr:\n{}", text(&output.stderr));
    assert!(text(&output.stdout).starts_with("driftless "));
}

#[test]
fn update_locks_refs_and_check_passes() {
    let repo = rust_repo_with_doc();

    let update = driftless(repo.path(), &["update"]);
    assert!(update.status.success(), "stderr:\n{}", text(&update.stderr));
    assert!(text(&update.stdout).contains("locked"));

    let lock = fs::read_to_string(repo.path().join(".driftless.lock")).expect("read lockfile");
    assert!(lock.contains("src/lib.rs#login"));

    let check = driftless(repo.path(), &["check"]);
    assert!(check.status.success(), "stderr:\n{}", text(&check.stderr));
    assert_eq!(text(&check.stdout), "driftless: ok\n");
}

#[test]
fn check_fails_for_unlocked_ref() {
    let repo = rust_repo_with_doc();

    let check = driftless(repo.path(), &["check"]);
    assert!(!check.status.success());
    assert!(text(&check.stderr).contains("not in lockfile"));
}

#[test]
fn update_fails_and_does_not_write_lockfile_for_unresolved_refs() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(dir.path(), "README.md", "See `src/missing.rs#thing`.\n");

    let update = driftless(dir.path(), &["update"]);
    assert!(!update.status.success());
    assert!(text(&update.stderr).contains("file not found"));
    assert!(text(&update.stderr).contains("lockfile not updated"));
    assert!(!dir.path().join(".driftless.lock").exists());
}

#[test]
fn malformed_lockfile_fails_check_instead_of_becoming_empty() {
    let repo = rust_repo_with_doc();
    write(repo.path(), ".driftless.lock", "{not json");

    let check = driftless(repo.path(), &["check"]);
    assert!(!check.status.success());
    assert!(text(&check.stderr).contains("failed to read"));
}

#[test]
fn json_check_reports_body_drift_with_doc_context_and_current_source() {
    let repo = rust_repo_with_doc();
    assert!(driftless(repo.path(), &["update"]).status.success());
    write(
        repo.path(),
        "src/lib.rs",
        r#"pub fn login(user: &str) -> bool {
    user == "root"
}
"#,
    );

    let check = driftless(repo.path(), &["check", "--json"]);
    assert!(!check.status.success());

    let records: Value = serde_json::from_slice(&check.stdout).expect("json records");
    let record = records
        .as_array()
        .expect("json array")
        .first()
        .expect("record");
    assert_eq!(record["status"], "body_drift");
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["severity"], "error");
    assert_eq!(record["blocks_exit"], true);
    assert_eq!(record["ref"], "src/lib.rs#login");
    assert!(record["expected_hash"].as_str().is_some());
    assert!(record["actual_hash"].as_str().is_some());
    assert_eq!(record["doc"]["heading"], "Login");
    assert!(record["doc"]["section"]
        .as_str()
        .unwrap()
        .contains("Documented by"));
    assert!(record["symbol_source"]
        .as_str()
        .unwrap()
        .contains("user == \"root\""));
}

#[test]
fn json_check_reports_warn_body_records_as_non_blocking_warnings() {
    let repo = rust_repo_with_doc();
    assert!(driftless(repo.path(), &["update"]).status.success());
    write(
        repo.path(),
        "src/lib.rs",
        r#"pub fn login(user: &str) -> bool {
    user == "root"
}
"#,
    );

    let check = driftless(repo.path(), &["check", "--warn-body", "--json"]);
    assert!(check.status.success(), "stderr:\n{}", text(&check.stderr));

    let records: Value = serde_json::from_slice(&check.stdout).expect("json records");
    let record = records
        .as_array()
        .expect("json array")
        .first()
        .expect("record");
    assert_eq!(record["status"], "body_drift");
    assert_eq!(record["severity"], "warning");
    assert_eq!(record["blocks_exit"], false);
    assert!(text(&check.stderr).contains("warning"));
}

#[test]
fn warn_body_demotes_body_drift_to_success_but_still_prints_warning() {
    let repo = rust_repo_with_doc();
    assert!(driftless(repo.path(), &["update"]).status.success());
    write(
        repo.path(),
        "src/lib.rs",
        r#"pub fn login(user: &str) -> bool {
    user == "root"
}
"#,
    );

    let check = driftless(repo.path(), &["check", "--warn-body"]);
    assert!(check.status.success(), "stderr:\n{}", text(&check.stderr));
    assert!(text(&check.stderr).contains("body changed"));
}

#[test]
fn check_fails_when_symbol_is_missing() {
    let repo = rust_repo_with_doc();
    assert!(driftless(repo.path(), &["update"]).status.success());
    write(
        repo.path(),
        "src/lib.rs",
        r#"pub fn logout(user: &str) -> bool {
    !user.is_empty()
}
"#,
    );

    let check = driftless(repo.path(), &["check"]);
    assert!(!check.status.success());
    assert!(text(&check.stderr).contains("symbol not found"));
}

#[test]
fn coverage_json_reports_undocumented_public_symbols() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/lib.rs",
        r#"pub fn documented() {}

pub fn undocumented() {}
"#,
    );
    write(dir.path(), "README.md", "See `src/lib.rs#documented`.\n");

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/", "--json"]);
    assert!(!coverage.status.success());
    assert_eq!(text(&coverage.stderr), "");

    let records: Value = serde_json::from_slice(&coverage.stdout).expect("json records");
    let record = records
        .as_array()
        .expect("json array")
        .first()
        .expect("record");
    assert_eq!(record["schema_version"], 1);
    assert_eq!(record["status"], "undocumented");
    assert_eq!(record["severity"], "error");
    assert_eq!(record["blocks_exit"], true);
    assert_eq!(record["kind"], "fn");
    assert_eq!(record["ref"], "src/lib.rs#undocumented");
}

#[test]
fn coverage_fails_for_public_symbols_without_docs() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/lib.rs",
        r#"pub fn documented() {}

pub fn undocumented() {}
"#,
    );
    write(dir.path(), "README.md", "See `src/lib.rs#documented`.\n");

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(!coverage.status.success());
    assert!(text(&coverage.stderr).contains("src/lib.rs#undocumented"));
}

#[test]
fn coverage_accepts_documented_public_symbols() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/lib.rs",
        r#"pub fn documented() {}
"#,
    );
    write(dir.path(), "README.md", "See `src/lib.rs#documented`.\n");

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
    assert_eq!(text(&coverage.stdout), "driftless: coverage ok\n");
}

#[test]
fn coverage_json_reports_empty_array_when_all_symbols_are_documented() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/lib.rs",
        r#"pub fn documented() {}
"#,
    );
    write(dir.path(), "README.md", "See `src/lib.rs#documented`.\n");

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/", "--json"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
    assert_eq!(text(&coverage.stderr), "");
    assert_eq!(text(&coverage.stdout), "[]\n");
}

#[test]
fn coverage_ignores_rust_crate_visible_symbols() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/lib.rs",
        r#"pub(crate) fn internal_helper() {}
"#,
    );
    write(dir.path(), "README.md", "# API\n\nNo public symbols.\n");

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
}

#[test]
fn link_refs_resolve_relative_to_the_markdown_file() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/auth.rs",
        r#"pub fn login() -> bool {
    true
}
"#,
    );
    write(
        dir.path(),
        "docs/guide.md",
        "See [login](../src/auth.rs#login).\n",
    );

    let update = driftless(dir.path(), &["update"]);
    assert!(update.status.success(), "stderr:\n{}", text(&update.stderr));

    let lock = fs::read_to_string(dir.path().join(".driftless.lock")).expect("read lockfile");
    assert!(lock.contains("src/auth.rs#login"));
}

#[test]
fn file_missing_is_reported_for_locked_refs() {
    let repo = rust_repo_with_doc();
    assert!(driftless(repo.path(), &["update"]).status.success());
    fs::remove_file(repo.path().join("src/lib.rs")).expect("remove source");

    let check = driftless(repo.path(), &["check"]);
    assert!(!check.status.success());
    assert!(text(&check.stderr).contains("file not found"));
}

#[test]
fn root_flag_allows_running_outside_the_project_directory() {
    let repo = rust_repo_with_doc();

    let output = Command::new(env!("CARGO_BIN_EXE_driftless"))
        .current_dir(PathBuf::from("/"))
        .arg("--root")
        .arg(repo.path())
        .arg("update")
        .output()
        .expect("run driftless outside repo");

    assert!(output.status.success(), "stderr:\n{}", text(&output.stderr));
    assert!(repo.path().join(".driftless.lock").exists());
}

#[test]
fn init_print_outputs_agent_and_ci_scaffold_without_writing() {
    let dir = tempfile::tempdir().expect("create temp repo");

    let output = driftless(dir.path(), &["init", "--print"]);
    assert!(output.status.success(), "stderr:\n{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("## AGENTS.md"));
    assert!(stdout.contains("driftless check --json"));
    assert!(stdout.contains("## .github/workflows/driftless.yml"));
    assert!(!dir.path().join("AGENTS.md").exists());
    assert!(!dir.path().join(".github/workflows/driftless.yml").exists());
}

#[test]
fn init_prompt_outputs_copyable_agent_setup_prompt_without_writing() {
    let dir = tempfile::tempdir().expect("create temp repo");

    let output = driftless(dir.path(), &["init", "--prompt"]);
    assert!(output.status.success(), "stderr:\n{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("Set up Driftless in this repository."));
    assert!(stdout.contains("CI target: `github`"));
    assert!(stdout.contains("driftless init --print --ci github"));
    assert!(stdout.contains("driftless check --json"));
    assert!(!dir.path().join("AGENTS.md").exists());
}

#[test]
fn init_prompt_respects_ci_choice() {
    let dir = tempfile::tempdir().expect("create temp repo");

    let output = driftless(dir.path(), &["init", "--prompt", "--ci", "gitlab"]);
    assert!(output.status.success(), "stderr:\n{}", text(&output.stderr));
    let stdout = text(&output.stdout);
    assert!(stdout.contains("CI target: `gitlab`"));
    assert!(stdout.contains("Use the GitLab CI scaffold."));
    assert!(stdout.contains("driftless init --print --ci gitlab"));
    assert!(stdout.contains("driftless init --ci gitlab"));
}

#[test]
fn init_can_print_and_write_gitlab_ci_scaffold() {
    let dir = tempfile::tempdir().expect("create temp repo");

    let printed = driftless(dir.path(), &["init", "--print", "--ci", "gitlab"]);
    assert!(
        printed.status.success(),
        "stderr:\n{}",
        text(&printed.stderr)
    );
    let stdout = text(&printed.stdout);
    assert!(stdout.contains("## .gitlab-ci.yml"));
    assert!(stdout.contains("image: rust:latest"));
    assert!(!stdout.contains(".github/workflows"));

    let init = driftless(dir.path(), &["init", "--ci", "gitlab"]);
    assert!(init.status.success(), "stderr:\n{}", text(&init.stderr));
    assert!(dir.path().join("AGENTS.md").exists());
    assert!(dir.path().join(".gitlab-ci.yml").exists());
    assert!(!dir.path().join(".github/workflows/driftless.yml").exists());
}

#[test]
fn init_can_skip_ci_scaffold() {
    let dir = tempfile::tempdir().expect("create temp repo");

    let init = driftless(dir.path(), &["init", "--ci", "none"]);
    assert!(init.status.success(), "stderr:\n{}", text(&init.stderr));
    assert!(dir.path().join("AGENTS.md").exists());
    assert!(!dir.path().join(".gitlab-ci.yml").exists());
    assert!(!dir.path().join(".github/workflows/driftless.yml").exists());
}

#[test]
fn init_writes_agent_guide_and_ci_without_overwriting_existing_files() {
    let dir = tempfile::tempdir().expect("create temp repo");

    let init = driftless(dir.path(), &["init"]);
    assert!(init.status.success(), "stderr:\n{}", text(&init.stderr));
    let agent_guide = fs::read_to_string(dir.path().join("AGENTS.md")).expect("read agent guide");
    let workflow = fs::read_to_string(dir.path().join(".github/workflows/driftless.yml"))
        .expect("read workflow");
    assert!(agent_guide.contains("Driftless Agent Guide"));
    assert!(workflow.contains("driftless coverage --include src/"));

    write(dir.path(), "AGENTS.md", "custom instructions\n");
    let second_init = driftless(dir.path(), &["init"]);
    assert!(!second_init.status.success());
    assert!(text(&second_init.stderr).contains("already exists"));
    let preserved = fs::read_to_string(dir.path().join("AGENTS.md")).expect("read agent guide");
    assert_eq!(preserved, "custom instructions\n");
}

#[test]
fn init_force_overwrites_scaffold_files() {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(dir.path(), "AGENTS.md", "custom instructions\n");

    let init = driftless(dir.path(), &["init", "--force"]);
    assert!(init.status.success(), "stderr:\n{}", text(&init.stderr));
    let agent_guide = fs::read_to_string(dir.path().join("AGENTS.md")).expect("read agent guide");
    assert!(agent_guide.contains("Driftless Agent Guide"));
}
