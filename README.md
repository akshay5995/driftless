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

After adding useful refs and reviewing the docs:

```sh
driftless update
```

While working:

```sh
driftless check
driftless check --json
```

Run `driftless update` only after the docs and source have been reviewed. Do not use it just to silence failures.

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
    "doc": { "file": "README.md", "line": 42 },
    "symbol_source": "pub fn login(...) { ... }"
  }
]
```

## Editor

Driftless includes an LSP server over stdio:

```sh
driftless lsp
```

The editor path uses the same checker as the CLI and can report diagnostics while source buffers are still unsaved.

## More

- [docs/architecture.md](docs/architecture.md) maps the implementation.
- [docs/development.md](docs/development.md) covers checks, release flow, and dogfooding.
- [docs/benchmarking.md](docs/benchmarking.md) explains benchmark fixtures and limits.
- [CONTRIBUTING.md](CONTRIBUTING.md) is the contributor entry point.
