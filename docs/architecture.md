# Architecture

Driftless has a small deterministic core with several fast feedback surfaces: the CLI, the JSON agent stream, editor diagnostics, and CI gates. The core extracts Markdown refs, resolves them to source symbols with tree-sitter, hashes the reviewed source span, and compares the result with `.driftless.lock`.

## Reference Extraction

Markdown refs are represented by `src/refs.rs#MdRef`. `src/refs.rs#extract_refs` recognizes inline code refs, Markdown links, and fenced code `ref=` attributes. `src/refs.rs#md_files` walks Markdown files while skipping heavy generated folders, and `src/refs.rs#line_of` maps byte offsets back to user-facing line numbers.

Refs are deliberately simple. File paths must stay inside the project root, and links are resolved relative to the Markdown file so docs remain clickable outside Driftless.

## Symbol Resolution

Language dispatch lives in `src/resolve.rs#language_for`. Tree-sitter parsing is isolated in `src/resolve.rs#parse_tree`, naming rules live in `src/resolve.rs#def_name`, and qualified paths such as Rust impl methods or Go receiver methods are normalized by `src/resolve.rs#def_path`.

`src/resolve.rs#resolve_symbol_in_tree` maps a dotted symbol path to a source byte range. `src/resolve.rs#symbol_hash` hashes signatures separately from bodies when the grammar exposes a body node, which lets `driftless check --warn-body` demote body-only churn without ignoring signature drift. `src/resolve.rs#symbol_source` returns current code for JSON agent records.

## Check And Update

`src/check.rs#run_check` drives both `driftless check` and `driftless update`. Each ref produces a `src/check.rs#RefStatus`; failures become terminal diagnostics or versioned JSON records with severity, blocking status, hashes, the stale Markdown section, and current source.

`src/check.rs#CheckContext` owns the per-run parse cache. The CLI uses it with files from disk. The LSP uses `src/check.rs#CheckContext.with_source_overrides` so unsaved source buffers can be checked before they hit disk.

Reviewed state is stored by `src/lockfile.rs#Lockfile`. `src/lockfile.rs#load_lock` treats a missing lockfile as empty state, but malformed lockfiles are hard errors. `src/lockfile.rs#save_lock` writes stable pretty JSON so lockfile diffs stay reviewable.

## Coverage

Coverage is the inverse check: source first, docs second. `src/coverage.rs#run_coverage` walks included source files, collects public symbols, and fails if neither a symbol nor one of its ancestors appears in docs. Human output stays compact, while `driftless coverage --json` emits versioned missing-symbol records for agents. That ancestor rule lets a class-level or type-level doc cover methods when the doc is intentionally about the whole API surface.

The public-symbol rules are intentionally conservative. Coverage should catch obvious missing public docs without pretending to be a full language server. The regression test near `tests/cli_core.rs#coverage_reports_rust_public_impl_methods` protects Rust inherent methods because they sit behind `impl` nodes rather than standalone public items.

## Consumers

The CLI command map lives in `src/main.rs`. `src/init.rs#print_prompt` is the entire project setup surface: it prints a copyable agent prompt and does not write CI or repository files. `src/init.rs#run_init` keeps that command side-effect-free.

The editor consumer is `src/lsp.rs#run`. It publishes diagnostics over stdio, tracks open Markdown documents, and rechecks them when source buffers or `.driftless.lock` change. It dynamically registers source-file and lockfile watchers with capable clients, while open source buffers flow through `src/check.rs#CheckContext.with_source_overrides` so the editor path stays on the same deterministic core as the CLI.

## Dogfooding

This repo keeps user docs and developer docs locked with Driftless. The shared language fixture `tests/common/language.rs#assert_language_roundtrip_refs` proves that refs, lockfile updates, checks, and coverage agree end to end. The supported language matrix starts around `tests/languages.rs#typescript_exported_class_and_function_refs_roundtrip`.
