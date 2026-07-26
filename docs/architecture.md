# Architecture

Driftless has a small deterministic core with several fast feedback surfaces: the CLI, the JSON agent stream, editor diagnostics, and CI gates. The core extracts Markdown refs, resolves them to source symbols with tree-sitter, hashes the reviewed source span, and compares the result with `.driftless.lock`.

## Reference Extraction

Markdown refs are represented by `src/refs.rs#MdRef`. `src/refs.rs#extract_refs` recognizes inline code refs, Markdown links, and fenced code `ref=` attributes. `src/refs.rs#md_files` walks Markdown files while skipping heavy generated folders, and `src/refs.rs#line_of` maps byte offsets back to user-facing line numbers.

Refs are deliberately simple. File paths must stay inside the project root, and links are resolved relative to the Markdown file so docs remain clickable outside Driftless.

## Symbol Resolution

`src/resolve.rs` is a small facade over focused resolver modules. Language dispatch lives in `src/resolve.rs#language_for`, and tree-sitter parsing is isolated in `src/resolve.rs#parse_tree`. Naming rules live in `src/resolve/names.rs#def_name`, and qualified paths such as Rust impl methods or Go receiver methods are normalized by `src/resolve/names.rs#def_path`.

`src/resolve.rs#resolve_symbol_in_tree` maps a dotted symbol path to a source byte range, returning `ResolveError::Ambiguous` when a path matches more than one distinct definition (for example the same method name defined by two different trait implementations) instead of silently picking the first match. A bare type name matching both its own declaration and an `impl` block for that type is not treated as ambiguous — the declaration wins, since an `impl` block's own def path is just the type name too. `src/resolve/range.rs#symbol_range` builds a small semantic envelope around the matched item: parser-owned attributes, decorators, annotations, item doc comments, export or single-variable declaration wrappers, and adjacent trailing statements that explicitly attach to the symbol through `src/resolve/attachments.rs#is_attached_statement`. Attached statements include static property assignments, default exports, direct and property-style CommonJS exports, and `Object.assign`; ordinary member calls, string-literal or new-binding lookalikes, and enclosing Rust `//!` docs stay outside the symbol range. Using syntax-tree relationships keeps unrelated declarations out of the symbol hash even when they are adjacent.

`src/resolve/hash.rs#symbol_hash` hashes signatures separately from bodies when the grammar exposes a body node, which lets `driftless check --warn-body` demote body-only churn without ignoring signature drift. Before hashing, both the signature and body text have comment nodes stripped and remaining whitespace collapsed, so comment-only edits and reformatting don't register as drift; comments are collected over the definition's whole span rather than just its body field, since some grammars (Python) place a comment that opens a body as a sibling of the body node rather than nesting it inside. `src/resolve/hash.rs#symbol_source` returns current code (including comments, unstripped) for JSON agent records.

## Check And Update

`src/check.rs#run_check` drives both `driftless check` and `driftless update`. Each ref produces a `src/check.rs#RefStatus`; failures become terminal diagnostics or versioned JSON records with severity, blocking status, hashes, the stale Markdown section, and current source.

`src/check.rs#CheckContext` owns the per-run parse cache. The CLI uses it with files from disk. The LSP uses `src/check.rs#CheckContext.with_source_overrides` so unsaved source buffers can be checked before they hit disk.

Reviewed state is stored by `src/lockfile.rs#Lockfile`, keyed per `(doc, ref)` pair via `src/check.rs#doc_lock_key` rather than by ref alone, so two docs mentioning the same symbol are tracked independently: relocking one doc's mention never marks another doc's mention as reviewed. Each `src/lockfile.rs#LockEntry` stores the code hash alongside a hash of the enclosing Markdown section at lock time; `src/check.rs#run_check` compares that against the section's current hash to tell whether the doc has already been touched since the code drifted. Body drift where the doc has already changed is demoted to a non-blocking warning; signature drift always blocks regardless, since a public contract change deserves an explicit look. `src/lockfile.rs#load_lock` treats a missing lockfile as empty state, but malformed lockfiles are hard errors. `src/lockfile.rs#save_lock` writes stable pretty JSON so lockfile diffs stay reviewable.

`driftless update` accepts optional path arguments to scope relocking to specific docs; refs in docs outside that scope keep their existing lock entries untouched, so a partially reviewed change doesn't relock mentions nobody looked at. Excluding paths entirely from both `check` and `update` is done with an optional `.driftlessignore` file, loaded by `src/config.rs#load_config` and matched by `src/config.rs#is_excluded` (a small glob matcher supporting `*` and `**`, with no external dependency).

## Consumers

The CLI command map lives in `src/main.rs#Cmd`, and `src/main.rs#Cli` owns the short help path agents see first. `src/init.rs#SETUP_PROMPT` embeds the canonical setup text from [setup-prompt.txt](setup-prompt.txt); `src/init.rs#print_prompt` prints it, and `src/init.rs#run_init` keeps that command side-effect-free instead of writing CI or repository files.

The editor consumer is `src/lsp.rs#run`. It publishes diagnostics over stdio, tracks open Markdown documents, and rechecks them when source buffers or `.driftless.lock` change. It dynamically registers source-file and lockfile watchers with capable clients, while open source buffers flow through `src/check.rs#CheckContext.with_source_overrides` so the editor path stays on the same deterministic core as the CLI.

## Dogfooding

This repo keeps user docs and developer docs locked with Driftless. The shared language fixture `tests/common/language.rs#assert_language_roundtrip_refs` proves that refs, lockfile updates, and checks agree end to end. The supported language matrix starts around `tests/languages.rs#typescript_exported_class_and_function_refs_roundtrip`, with resolver boundary cases covered by `tests/languages.rs#tsx_static_assignment_drift_fails_check`, `tests/languages.rs#tsx_adjacent_member_call_change_does_not_drift_symbol`, `tests/languages.rs#rust_inner_doc_change_does_not_drift_following_function`, `tests/languages.rs#javascript_direct_commonjs_export_drift_fails_check`, `tests/languages.rs#javascript_commonjs_arrow_wrapper_drift_fails_check`, `tests/languages.rs#javascript_commonjs_string_literal_does_not_attach_symbol`, `tests/languages.rs#javascript_commonjs_function_binding_does_not_attach_symbol`, and `tests/languages.rs#javascript_commonjs_parameter_binding_does_not_attach_symbol`.
