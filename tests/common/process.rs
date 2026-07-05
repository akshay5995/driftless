use std::fs;
use std::path::Path;
use std::process::{Command, Output};

pub fn driftless(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_driftless"))
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .expect("run driftless")
}

pub fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, contents).expect("write fixture");
}

pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}
