# driftless

Driftless tells you when Markdown docs may be stale.

You point a doc at the code it describes. Driftless saves the reviewed source hash in `.driftless.lock`. Later, when that source changes, `driftless check` points back to the exact doc section that needs review.

It does not decide whether prose is correct. It creates a fast review tripwire so docs are checked while code is still being changed.

## Install

Install the latest GitHub Release:

```sh
curl -fsSL https://raw.githubusercontent.com/akshay5995/driftless/main/install.sh | sh
```

For pinned versions or a custom install directory, see the [installer options](install.sh). Prebuilt archives and checksums are available on [GitHub Releases](https://github.com/akshay5995/driftless/releases).

## Basic Loop

Print the setup prompt for an agent:

```sh
driftless init
```

You can also point an agent at [docs/setup-prompt.txt](docs/setup-prompt.txt).
The prompt checks whether Driftless is installed, installs the latest GitHub Release when needed, and verifies the CLI before configuring the repository.

After adding useful refs and reviewing the docs:

```sh
driftless update
```

Restrict the relock to specific docs when only some have been reviewed (for example in a large PR); refs in every other doc keep their prior locked state and still fail `check` if they've drifted:

```sh
driftless update README.md docs/
```

While working:

```sh
driftless check
driftless check --json
```

Run `driftless update` only after the docs and source have been reviewed. Do not use it just to silence failures.

Exclude paths from scanning with a `.driftlessignore` file (one glob per line, `#` for comments):

```text
vendor/**
docs/legacy/*.md
```

## Add Refs

Inline refs:

```markdown
Login is handled by `src/auth.py#AuthService.login`.
```

Markdown links stay clickable:

```markdown
See [login](../src/auth.py#AuthService.login).
```

Fenced code refs work when prose is attached to a snippet:

````markdown
```python ref=src/auth.py#AuthService.login
def login(self, user, password): ...
```
````

Supported source files: Go, Java, Kotlin, Python, Ruby, Rust, TypeScript, TSX, JavaScript, and JSX.

## What Fails

A clean check:

```text
driftless: ok
```

If a referenced symbol changes:

```text
error    README.md:42 src/lib.rs#login signature changed (... -> ...); update docs, then `driftless update`
driftless: 1 error(s)
```

For agents, JSON output includes the stale doc section and current source:

```json
[
  {
    "status": "body_drift",
    "ref": "src/lib.rs#login",
    "doc_reviewed": false,
    "doc": { "file": "README.md", "line": 42 },
    "symbol_source": "pub fn login(...) { ... }"
  }
]
```

`doc_reviewed` is true when the Markdown section covering the ref has already changed since it was locked; body drift in that case is a non-blocking `warning` instead of an `error`, since something has already edited that prose. Signature drift always blocks, since a public contract change deserves an explicit look regardless. The same symbol referenced from two different docs is tracked independently, so reviewing (and relocking) one doc's mention never marks another doc's mention as reviewed.

A ref can also come back `ambiguous_symbol` when it names multiple distinct definitions (for example two different trait implementations of the same method) rather than one — the tool won't silently guess which one you mean.

## Editor

Driftless includes an LSP server over stdio:

```sh
driftless lsp
```

The editor path uses the same checker as the CLI and can report diagnostics while source buffers are still unsaved.

## Documentation

- [Agent setup prompt](docs/setup-prompt.txt) — self-contained instructions that install Driftless when needed and add it to a repository.
- [Architecture](docs/architecture.md) — how Markdown refs are extracted, resolved, hashed, and checked.
- [Development](docs/development.md) — local verification, dogfooding, and the release process.
- [Benchmarking](docs/benchmarking.md) — performance scenarios, baselines, and measurement guidance.
- [Contributing](CONTRIBUTING.md) — how to propose and validate changes.
