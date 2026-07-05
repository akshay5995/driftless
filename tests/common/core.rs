use crate::process::write;
use tempfile::TempDir;

pub fn rust_repo_with_doc() -> TempDir {
    let dir = tempfile::tempdir().expect("create temp repo");
    write(
        dir.path(),
        "src/lib.rs",
        r#"pub fn login(user: &str) -> bool {
    user == "admin"
}
"#,
    );
    write(
        dir.path(),
        "README.md",
        "# Login\n\nDocumented by `src/lib.rs#login`.\n",
    );
    dir
}
