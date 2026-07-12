# Development

Driftless is built for the local loop first. CI should catch missed drift, but the preferred repair path is a fast local check, editor diagnostics, or `driftless check --json` while the code change is still active.

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

When a change may affect hot-path performance, also run:

```sh
cargo bench --bench cli
```

Use the benchmark smoke check for quick CI-style validation:

```sh
cargo bench --bench cli -- --test
```

## Dogfooding Rules

When docs describe implementation behavior, link the docs to real source refs and update `.driftless.lock` with the docs. The deterministic core is `src/check.rs#run_check`; Markdown parsing starts at `src/refs.rs#extract_refs`; symbol resolution starts at `src/resolve.rs#resolve_symbol_in_tree`; hashing lives in `src/resolve/hash.rs#symbol_hash`.

Do not run `driftless update` just to silence failures. Review the prose first, then refresh the lockfile.

## Product Boundaries

[setup-prompt.txt](setup-prompt.txt) is the source printed by `src/init.rs#print_prompt`. `driftless init` should not guess the user's CI provider, overwrite repository policy, or install workflow files. `src/init.rs#run_init` should stay side-effect-free.

Keep `src/lsp.rs#run` on the same checking path as the CLI. Editor diagnostics should use `src/check.rs#CheckContext` so source-buffer overrides, lockfile behavior, and resolver behavior stay aligned with `driftless check`.

Keep JSON records compact and repair-oriented. The agent interface should include enough context to fix stale docs without dumping unrelated source. Treat JSON fields as append-only within a version series; adding fields is fine, but removing or renaming fields should wait for an explicit format version.

Keep resolver growth test-led. `src/resolve/range.rs#symbol_range` should stay a small semantic-envelope rule, with language-specific behavior added only after a fixture proves the generic rule misses real user-facing behavior. Put adjacent statement patterns in `src/resolve/attachments.rs#is_attached_statement` rather than growing the resolver facade. The positive attachment boundary is covered by `tests/languages.rs#tsx_static_assignment_drift_fails_check`, `tests/languages.rs#tsx_default_wrapper_drift_fails_check`, `tests/languages.rs#javascript_direct_commonjs_export_drift_fails_check`, and `tests/languages.rs#javascript_commonjs_arrow_wrapper_drift_fails_check`; unrelated-code exclusions are covered by `tests/languages.rs#tsx_adjacent_member_call_change_does_not_drift_symbol`, `tests/languages.rs#rust_inner_doc_change_does_not_drift_following_function`, `tests/languages.rs#javascript_commonjs_string_literal_does_not_attach_symbol`, `tests/languages.rs#javascript_commonjs_function_binding_does_not_attach_symbol`, and `tests/languages.rs#javascript_commonjs_parameter_binding_does_not_attach_symbol`.

## Release Flow

Release binaries are published by `.github/workflows/release.yml` when a `v*` tag is pushed. Before building artifacts, the workflow runs formatting, Clippy, tests, benchmark smoke, and both human and JSON Driftless checks. It then verifies that the tag matches `Cargo.toml`, runs `cargo package --locked` as a packaging sanity check, builds release binaries for Linux x64/arm64, macOS Intel/Apple Silicon, and Windows x64, publishes a GitHub Release with `SHA256SUMS`, and creates artifact attestations.

The release workflow follows the repository's binary distribution rules: third-party actions are pinned to full commit SHAs, workflow permissions are scoped per job, release assets are checksummed before publication, and every published archive plus `SHA256SUMS` receives provenance attestation. After a public release is finalized, prefer a new patch tag over moving an existing tag; enable GitHub immutable releases or equivalent tag protections where the repository settings allow it.

Before tagging, run the required checks above from a clean worktree and confirm the README install instructions match the actual distribution state.
