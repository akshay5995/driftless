use crate::process::{driftless, text, write};
use std::fs;

pub fn assert_language_roundtrip(path: &str, source: &str, reference: &str) {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(dir.path(), path, source);
    write(
        dir.path(),
        "README.md",
        &format!("# API\n\nDocumented by `{reference}`.\n"),
    );

    let update = driftless(dir.path(), &["update"]);
    assert!(update.status.success(), "stderr:\n{}", text(&update.stderr));
    assert!(text(&update.stdout).contains("locked"));

    let lock = fs::read_to_string(dir.path().join(".driftless.lock")).expect("read lockfile");
    assert!(lock.contains(reference), "lockfile:\n{lock}");

    let check = driftless(dir.path(), &["check"]);
    assert!(check.status.success(), "stderr:\n{}", text(&check.stderr));

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
}

pub fn assert_language_roundtrip_refs(path: &str, source: &str, references: &[&str]) {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(dir.path(), path, source);
    let refs = references
        .iter()
        .map(|reference| format!("`{reference}`"))
        .collect::<Vec<_>>()
        .join(", ");
    write(
        dir.path(),
        "README.md",
        &format!("# API\n\nDocumented by {refs}.\n"),
    );

    let update = driftless(dir.path(), &["update"]);
    assert!(update.status.success(), "stderr:\n{}", text(&update.stderr));

    let lock = fs::read_to_string(dir.path().join(".driftless.lock")).expect("read lockfile");
    for reference in references {
        assert!(
            lock.contains(reference),
            "missing {reference} in lockfile:\n{lock}"
        );
    }

    let check = driftless(dir.path(), &["check"]);
    assert!(check.status.success(), "stderr:\n{}", text(&check.stderr));

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(
        coverage.status.success(),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
}

pub fn assert_missing_doc_is_reported(path: &str, source: &str, expected: &str) {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(dir.path(), path, source);
    write(dir.path(), "README.md", "# API\n\nNo symbol refs yet.\n");

    let coverage = driftless(dir.path(), &["coverage", "--include", "src/"]);
    assert!(!coverage.status.success());
    assert!(
        text(&coverage.stderr).contains(expected),
        "stderr:\n{}",
        text(&coverage.stderr)
    );
}
