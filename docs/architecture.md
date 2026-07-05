# Architecture

Driftless has one deterministic core and several fast feedback surfaces. The core extracts references from Markdown, resolves each reference to a source symbol, hashes the reviewed symbol, and compares that hash to the lockfile. The feedback surfaces are local checks, editor diagnostics, CI gates, and the JSON record stream an agent can use for a fix loop.

## Reference Extraction

Markdown references are represented by `src/refs.rs#MdRef`. The extractor `src/refs.rs#extract_refs` finds inline code refs, fenced-code `ref=` attrs, and Markdown links. File discovery is kept in `src/refs.rs#md_files`, and diagnostics use `src/refs.rs#line_of` to report human-readable doc positions.

## Symbol Resolution

Language dispatch lives in `src/resolve.rs#language_for`. Tree-sitter node naming is centralized in `src/resolve.rs#def_name`, qualified node paths such as Go receiver methods are normalized by `src/resolve.rs#def_path`, and resolved source spans are carried by `src/resolve.rs#Resolved`. `src/resolve.rs#parse_tree` parses source once, `src/resolve.rs#resolve_symbol_in_tree` maps a dotted symbol path to source bytes, `src/resolve.rs#symbol_hash` splits signature/body hashes where possible, and `src/resolve.rs#symbol_source` returns current code for JSON agent records.

## Lockfile

Reviewed state is stored as `src/lockfile.rs#Lockfile`. `src/lockfile.rs#lock_path`, `src/lockfile.rs#load_lock`, and `src/lockfile.rs#save_lock` keep the `.driftless.lock` boundary small and explicit; malformed lockfiles are hard errors rather than empty state.

## Checks

Each resolved ref produces a `src/check.rs#RefStatus`. `src/check.rs#CheckContext` caches parsed source files during one check run, and `src/check.rs#CheckContext.check_ref` compares one Markdown reference against current source and the lockfile. `src/check.rs#run_check` drives both `driftless check` and `driftless update`, including versioned JSON records for agent loops with severity, blocking status, hashes, doc context, and current symbol source.

## Missing Docs

Coverage is the inverse check: source symbols first, docs second. `src/coverage.rs#run_coverage` walks public symbols and fails if neither the symbol nor an ancestor is referenced by docs. Human output stays compact, while `driftless coverage --json` emits versioned records for agents that need exact undocumented symbols.

The supported language set is declared by `src/main.rs#SOURCE_EXTENSIONS` and parsed by `src/resolve.rs#language_for`. Symbol naming is deliberately shared through `src/resolve.rs#def_name` and `src/resolve.rs#def_path`, while public API rules stay in coverage so drift checks can still resolve private symbols when docs explicitly reference them.

## Consumers

The CLI in `src/main.rs` dispatches commands into the modules above. `src/init.rs#print_prompt` gives agents a copyable setup prompt, and `src/init.rs#run_init` gives new projects an agent guide plus optional GitHub or GitLab CI workflow so setup is copyable instead of bespoke. The editor consumer is `src/lsp.rs#run`, which publishes diagnostics over stdio using the same check path as local and CI checks. It dynamically registers source-file and lockfile watchers with capable clients, then rechecks open Markdown docs when attached source files or `.driftless.lock` change. Agentic coding uses the same core through `driftless check --json`, so documentation repair can happen during the coding loop instead of after drift lands.

## Dogfood Tests

Developer-facing behavior should be proven through integration tests first. `tests/common/language.rs#assert_language_roundtrip_refs` is the shared fixture for language support: it writes docs, runs `driftless update`, checks the lockfile, runs `driftless check`, and verifies coverage. The tests near `tests/languages.rs#typescript_exported_class_and_function_refs_roundtrip` cover the supported language shapes so parser changes fail locally before docs drift.
