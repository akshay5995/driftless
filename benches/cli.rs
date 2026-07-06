use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

fn driftless_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_driftless") {
        return PathBuf::from(path);
    }

    let exe = std::env::current_exe().expect("current bench executable path");
    let release_dir = exe
        .parent()
        .and_then(Path::parent)
        .expect("bench executable under target/release/deps");
    let bin = release_dir.join(format!("driftless{}", std::env::consts::EXE_SUFFIX));
    ensure_release_binary(&bin, &exe);
    bin
}

fn ensure_release_binary(bin: &Path, bench_exe: &Path) {
    let bench_mtime = modified(bench_exe);
    let bin_mtime = modified(bin);
    if bin.exists() && bin_mtime >= bench_mtime {
        return;
    }

    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let status = Command::new("cargo")
        .current_dir(manifest_dir)
        .args(["build", "--release", "--bin", "driftless"])
        .status()
        .expect("build driftless release binary for benchmark");
    assert!(status.success(), "failed to build driftless release binary");
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH)
}

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent dir");
    }
    fs::write(path, contents).expect("write benchmark fixture");
}

fn command_with_status(bin: &Path, root: &Path, args: &[&str], should_succeed: bool) {
    let output = Command::new(bin)
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .expect("run driftless benchmark command");
    assert!(
        output.status.success() == should_succeed,
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    black_box(output);
}

fn command(bin: &Path, root: &Path, args: &[&str]) {
    command_with_status(bin, root, args, true);
}

fn benchmark_repo(symbols: usize) -> TempDir {
    let repo = tempfile::tempdir().expect("create benchmark repo");
    let mut source = String::new();
    let mut docs = String::from("# API\n\n");

    for i in 0..symbols {
        source.push_str(&format!(
            "pub fn item_{i}(value: usize) -> bool {{\n    value == {i}\n}}\n\n"
        ));
        docs.push_str(&format!("- `src/lib.rs#item_{i}`\n"));
    }

    write(repo.path(), "src/lib.rs", &source);
    write(repo.path(), "README.md", &docs);

    let bin = driftless_bin();
    command(&bin, repo.path(), &["update"]);
    repo
}

fn benchmark_mixed_repo(files: usize) -> TempDir {
    let repo = tempfile::tempdir().expect("create mixed benchmark repo");
    let mut docs = String::from("# API\n\n");

    for i in 0..files {
        write(
            repo.path(),
            &format!("src/rust/item_{i}.rs"),
            &format!("pub fn item_{i}(value: usize) -> bool {{\n    value == {i}\n}}\n"),
        );
        write(
            repo.path(),
            &format!("src/go/item_{i}.go"),
            &format!(
                "package item{i}\n\ntype User{i} struct{{}}\n\nfunc (u *User{i}) Login() bool {{\n    return true\n}}\n"
            ),
        );
        docs.push_str(&format!(
            "- `src/rust/item_{i}.rs#item_{i}`\n- `src/go/item_{i}.go#User{i}`\n- `src/go/item_{i}.go#User{i}.Login`\n"
        ));
    }

    write(repo.path(), "README.md", &docs);
    let bin = driftless_bin();
    command(&bin, repo.path(), &["update"]);
    repo
}

fn benchmark_cached_source_repo(refs: usize) -> TempDir {
    let repo = tempfile::tempdir().expect("create cached-source benchmark repo");
    write(
        repo.path(),
        "src/lib.rs",
        "pub fn login(user: &str) -> bool {\n    user == \"admin\"\n}\n",
    );
    let mut docs = String::from("# API\n\n");
    for i in 0..refs {
        docs.push_str(&format!("- repeated ref {i}: `src/lib.rs#login`\n"));
    }
    write(repo.path(), "README.md", &docs);

    let bin = driftless_bin();
    command(&bin, repo.path(), &["update"]);
    repo
}

fn benchmark_drift_repo(symbols: usize) -> TempDir {
    let repo = benchmark_repo(symbols);
    let mut source = String::new();
    for i in 0..symbols {
        source.push_str(&format!(
            "pub fn item_{i}(value: usize) -> bool {{\n    value != {i}\n}}\n\n"
        ));
    }
    write(repo.path(), "src/lib.rs", &source);
    repo
}

fn bench_cli(c: &mut Criterion) {
    let bin = driftless_bin();
    let repo = benchmark_repo(200);
    let mixed_repo = benchmark_mixed_repo(40);
    let cached_repo = benchmark_cached_source_repo(1_000);
    let drift_repo = benchmark_drift_repo(80);

    let mut group = c.benchmark_group("cli");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(250));
    group.measurement_time(Duration::from_secs(2));

    group.bench_function("check_200_refs", |b| {
        b.iter(|| command(&bin, repo.path(), &["check"]));
    });
    group.bench_function("check_json_200_refs", |b| {
        b.iter(|| command(&bin, repo.path(), &["check", "--json"]));
    });
    group.bench_function("coverage_200_public_symbols", |b| {
        b.iter(|| command(&bin, repo.path(), &["coverage", "--include", "src/"]));
    });
    group.bench_function("check_mixed_120_refs_80_files", |b| {
        b.iter(|| command(&bin, mixed_repo.path(), &["check"]));
    });
    group.bench_function("check_cached_source_1000_refs", |b| {
        b.iter(|| command(&bin, cached_repo.path(), &["check"]));
    });
    group.bench_function("check_json_80_body_drifts", |b| {
        b.iter(|| command_with_status(&bin, drift_repo.path(), &["check", "--json"], false));
    });
    group.bench_function("coverage_mixed_120_public_symbols", |b| {
        b.iter(|| command(&bin, mixed_repo.path(), &["coverage", "--include", "src/"]));
    });

    group.finish();
}

criterion_group!(benches, bench_cli);
criterion_main!(benches);
