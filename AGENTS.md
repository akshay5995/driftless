# Agent Guide

Driftless exists to reduce documentation drift while code is being written. Treat CI as the backstop, not the main loop: prefer fast local checks, editor diagnostics, and `driftless check --json` for agentic repair.

## Working Rules

- Keep the hot path fast. Before adding parsing, filesystem walking, allocation-heavy code, or broad dependency changes, consider the cost in `driftless check`.
- Do not add unsafe code. This crate uses `#![forbid(unsafe_code)]`; generated or dependency internals are outside this repo, but Driftless source should stay safe Rust.
- Preserve the deterministic core: Markdown refs are extracted by `src/refs.rs#extract_refs`, resolved by `src/resolve.rs#resolve_symbol_in_tree`, hashed by `src/resolve/hash.rs#symbol_hash`, and checked by `src/check.rs#run_check`.
- A dotted symbol path must resolve to exactly one definition. `src/resolve.rs#resolve_symbol_in_tree` returns `ResolveError::Ambiguous` rather than guessing when a path matches more than one distinct definition; a bare type name matching both its own declaration and its impl block is not treated as a conflict.
- Lock state is keyed per `(doc, ref)` pair via `src/check.rs#doc_lock_key`, not per ref alone, so reviewing one doc's mention of a symbol never marks a different doc's mention as reviewed.
- Keep setup dead simple: `src/init.rs#print_prompt` should give agents a copyable setup prompt, and `src/init.rs#run_init` should not write CI or repository files.
- Update docs and `.driftless.lock` together when changing referenced code or architecture docs. Run `./target/release/driftless update` only after the docs have been reviewed; scope it to specific paths (`driftless update <path>...`) when only part of a change has been reviewed.
- Add behavioral tests for user-visible behavior. Prefer real integration tests under `tests/` for agent/editor/CI workflows.
- Dogfood the tool: docs should link to the implementation or tests they describe. Shared fixtures live in `tests/common/language.rs#assert_language_roundtrip_refs`, and the language matrix lives around `tests/languages.rs#typescript_exported_class_and_function_refs_roundtrip`.
- Use unit tests for local edge cases: parser normalization, symbol resolution, lockfile behavior, and JSON sections.

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

## Releases

Release binaries are pushed by `.github/workflows/release.yml` when a `v*` tag is pushed. The workflow checks that the tag matches the Cargo package version, verifies `cargo package`, builds packaged release binaries for Linux x64/arm64, macOS Intel/Apple Silicon, and Windows x64, publishes a GitHub Release with `SHA256SUMS`, and creates artifact attestations.

When a change may affect performance, also run:

```sh
cargo bench --bench cli
```

For quick CI-style validation of the benchmark target without collecting full measurements:

```sh
cargo bench --bench cli -- --test
```

## Performance Notes

- Benchmarks live in `benches/cli.rs#bench_cli` and measure the real `driftless` binary on synthetic repos with many documented refs.
- Keep the per-run source cache in `src/check.rs#CheckContext` healthy; repeated refs to the same file should not repeatedly parse that file.
- Parse only files referenced by docs.
- Avoid introducing project-wide indexes until a benchmark proves the current on-demand model is insufficient.
- Keep JSON useful but compact; it is an agent interface, so include enough context to fix docs without dumping unrelated code.
