# Contributing

This guide is for people changing Driftless itself. User-facing usage lives in [README.md](README.md).

## Start Here

- Repo checks, release flow, and product boundaries: [docs/development.md](docs/development.md)
- Benchmark details and fixture limits: [docs/benchmarking.md](docs/benchmarking.md)
- Implementation map: [docs/architecture.md](docs/architecture.md)

## Required Checks

Run these before handing off changes:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
./target/release/driftless check
./target/release/driftless check --json
```

When release packaging changes, also run:

```sh
cargo package --locked
```

When a change may affect performance, also run:

```sh
cargo bench --bench cli
```

For quick CI-style validation of the benchmark target without collecting full measurements:

```sh
cargo bench --bench cli -- --test
```

## Release Tags

Release binaries are pushed by `.github/workflows/release.yml` when a `v*` tag is pushed. The peeled tag ref, `refs/tags/v0.2.0^{}`, should resolve to the release commit.

To publish `v0.2.0` from the current commit:

```sh
git status --short --branch
git tag -a v0.2.0 -m "driftless 0.2.0" HEAD
git push origin main
git push origin refs/tags/v0.2.0
git ls-remote origin refs/heads/main refs/tags/v0.2.0 refs/tags/v0.2.0^{}
```
