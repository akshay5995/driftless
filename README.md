# driftless

Driftless keeps Markdown docs linked to code so drift is caught while you work.

Package name: `driftless`. Binary name: `driftless`.

It is built for developer and agent loops:

- write docs that point at real symbols
- run a fast local check
- get exact stale sections as JSON when an agent needs to repair docs
- let CI be the backstop, not the first time drift is noticed

## Ref syntax

```markdown
Login handled by `src/auth.py#AuthService.login`.

​```python ref=src/auth.py#AuthService.login
def login(self, user, password): ...
​```
```

Plain Markdown links work too. Link refs are resolved relative to the doc, so they stay clickable on GitHub and in editors:

```markdown
See [login](../src/auth.py#AuthService.login).
```

Languages: Go, Java, Kotlin, Python, Ruby, Rust, TS/TSX/JS. Symbols resolved via tree-sitter (dotted paths: `Class.method`, Go `Receiver.Method`, `mod.fn`, rust `impl` types included).

## Local loop

```sh
cargo install driftless --locked
driftless --version
driftless init --prompt
driftless init
driftless check
driftless check --json
driftless coverage --include src/
driftless coverage --include src/ --json
```

Give `driftless init --prompt` to an agent when you want it to set up a project. The same setup path is configurable: use `driftless init --prompt --ci gitlab` for a GitLab-ready prompt, `driftless init --ci gitlab` to write `.gitlab-ci.yml`, or `driftless init --ci none` when CI is managed elsewhere. By default, `driftless init` writes an `AGENTS.md` guide and GitHub Actions workflow; `driftless init --print --ci <github|gitlab|none>` prints copyable snippets instead of writing files.

When docs have been reviewed, update the lockfile:

```sh
driftless update
```

`update` writes `.driftless.lock` only when every ref resolves. When code changes later, `check` fails until the docs are reviewed and the lockfile is refreshed.

Other useful commands:

```sh
driftless check --warn-body  # body-only drift = warning; signature drift still fails
driftless lsp                # LSP over stdio
```

Signature and body are hashed separately. Coverage counts a symbol documented if it or an ancestor is referenced; constructors, private names (`_name`), Rust crate-visible items (`pub(crate)`), and non-exported symbols are exempt.

## Dogfooding this repo

This repo uses Driftless against its own docs and tests:

- `docs/architecture.md` links to the core implementation.
- `src/init.rs#run_init` is the project setup path for new users and agents.
- `src/init.rs#print_prompt` is the copyable prompt path for agent-led setup.
- `tests/common/language.rs#assert_language_roundtrip_refs` proves refs, lockfile updates, checks, and coverage agree end to end.
- `tests/languages.rs#typescript_exported_class_and_function_refs_roundtrip` and neighboring tests cover the supported language matrix.

Before handing off changes here, run:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
./target/release/driftless check
./target/release/driftless check --json
./target/release/driftless coverage --include src/
```

The benchmark suite in `benches/cli.rs#bench_cli` uses the real release binary:

| Benchmark | What it protects |
| --- | --- |
| `check_200_refs` | Basic locked-ref check path on one documented Rust source file. |
| `check_json_200_refs` | Agent JSON output overhead when refs are valid. |
| `coverage_200_public_symbols` | Public-symbol inventory and docs coverage on a dense Rust file. |
| `check_mixed_120_refs_80_files` | Multi-language, multi-file source parsing across Rust and Go refs. |
| `check_cached_source_1000_refs` | Per-run source parse cache when many docs point at one file. |
| `check_json_80_body_drifts` | Agent JSON output when many refs need repair context. |
| `coverage_mixed_120_public_symbols` | Coverage over mixed Rust and Go public APIs. |

Run full measurements with `cargo bench --bench cli`; use `cargo bench --bench cli -- --test` for a quick smoke check.

## CI Backstop

```yaml
- run: driftless check
```

This repo also includes `.github/workflows/ci.yml` with format, clippy, tests, release build, `driftless check`, and `driftless coverage` checks.

## Release Binaries

Push a version tag to publish binaries:

```sh
git tag v0.1.0
git push origin v0.1.0
```

`.github/workflows/release.yml` builds release archives for Linux x64/arm64, macOS Intel/Apple Silicon, and Windows x64. The workflow verifies the tag matches `Cargo.toml`, checks the crates.io package, publishes archives with `SHA256SUMS`, and creates GitHub artifact attestations for release assets.

Install a prebuilt Unix binary by choosing one target from `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, or `aarch64-apple-darwin`:

```sh
version=0.1.0
target=aarch64-apple-darwin
repo=akshay5995/driftless
base="https://github.com/${repo}/releases/download/v${version}"

curl -fsSLO "${base}/driftless-${version}-${target}.tar.gz"
curl -fsSLO "${base}/SHA256SUMS"
shasum -a 256 -c SHA256SUMS --ignore-missing
gh attestation verify "driftless-${version}-${target}.tar.gz" --repo "${repo}"
tar -xzf "driftless-${version}-${target}.tar.gz"
install -m 0755 "driftless-${version}-${target}/driftless" /usr/local/bin/driftless
driftless --version
```

For Windows, download `driftless-0.1.0-x86_64-pc-windows-msvc.zip`, verify it against `SHA256SUMS`, then add the extracted `driftless.exe` to `PATH`.

## Editor

Neovim (native LSP):
```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "markdown",
  callback = function()
    vim.lsp.start({ name = "driftless", cmd = { "driftless", "lsp" },
      root_dir = vim.fs.root(0, { ".driftless.lock", ".git" }) })
  end,
})
```

VS Code: any generic LSP client works, e.g. the "LSP Proxy"/"Generic LSP Client" extensions pointed at `driftless lsp` for `markdown`. (A dedicated small extension is the polished path.)

The server dynamically registers source-file and `.driftless.lock` watchers when the client supports `workspace/didChangeWatchedFiles`. Clients without dynamic file watching still get diagnostics on Markdown edits and saves; configure them to send file-change notifications for `**/*.{go,java,js,jsx,kt,kts,py,rb,rs,ts,tsx}` and `**/.driftless.lock` if you want diagnostics to refresh immediately after source or lockfile changes.

## Design

- No LSP servers spawned for target languages, no SCIP index. Driftless uses tree-sitter and parses only referenced or included files.
- Drift = sha256 of the symbol's byte range vs `.driftless.lock`. Rename/delete = resolution failure.
- Adding a language = one crate + node naming in `def_name()`/`def_path()` plus public-symbol rules for coverage.
- Coverage is intentionally conservative: it tracks common public classes, functions, methods, types, and modules, not a full language-server-grade symbol graph.

See `docs/architecture.md` for the module map. That doc uses live `driftless` references into this codebase and is locked in `.driftless.lock`.

## Agent loop (`--json`)

`driftless check --json` prints a JSON array. Each record includes `schema_version: 1`, `status` (`sig_drift`|`body_drift`|`file_missing`|`symbol_missing`|`unlocked`), `severity` (`error`|`warning`), `blocks_exit`, `ref`, `source_file`, `symbol`, `expected_hash`, `actual_hash`, `doc {file, line, heading, section}` (full enclosing markdown section), and `symbol_source` (current code, null if unresolvable). `driftless check --warn-body --json` still emits `body_drift` records, but marks them as non-blocking warnings and exits 0 if no blocking records exist.

`driftless coverage --json --include src/` also prints a JSON array. Each record includes `schema_version: 1`, `status: "undocumented"`, `severity: "error"`, `blocks_exit: true`, `kind`, `source_file`, `symbol`, and `ref`.

Old code comes from git, not driftless: `git show $(git log -1 --format=%H -- .driftless.lock):$source_file`. Judge gets {old code, new code, doc section} -> binary still_accurate + patch. Accurate -> `driftless update`. Stale -> apply patch, review, `driftless update`.
