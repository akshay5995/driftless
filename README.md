# driftless

Driftless keeps Markdown docs tied to the code they describe. It catches stale docs during local, editor, and agent workflows so CI is the backstop, not the first warning.

## Why it exists

Docs drift when code changes faster than prose. Driftless makes important Markdown statements point at real source symbols, records the reviewed symbol hashes in `.driftless.lock`, and tells you exactly which doc section needs another look when the code changes.

For agents, `driftless check --json` returns repair-ready records with the stale doc section and current source. For people, plain `driftless check` gives fast terminal feedback.

## Install

Install the latest GitHub Release:

```sh
curl -fsSL https://raw.githubusercontent.com/akshay5995/driftless/main/install.sh | sh
```

Install a specific version or directory:

```sh
curl -fsSL https://raw.githubusercontent.com/akshay5995/driftless/main/install.sh \
  | DRIFTLESS_VERSION=v0.1.0 DRIFTLESS_INSTALL_DIR=/usr/local/bin sh
```

The installer downloads the matching release archive for your OS and CPU, verifies it with `SHA256SUMS`, and installs `driftless`.

Manual install:

```sh
# Download the archive for your platform from GitHub Releases, then:
tar -xzf driftless-0.1.0-x86_64-unknown-linux-gnu.tar.gz
install -m 755 driftless-0.1.0-x86_64-unknown-linux-gnu/driftless ~/.local/bin/driftless
driftless --version
```

## Add refs

Inline refs:

```markdown
Login is handled by `src/auth.py#AuthService.login`.
```

Markdown links stay clickable in editors and on GitHub:

```markdown
See [login](../src/auth.py#AuthService.login).
```

Fenced code refs work when the prose is attached to a snippet:

````markdown
```python ref=src/auth.py#AuthService.login
def login(self, user, password): ...
```
````

Supported source files: Go, Java, Kotlin, Python, Ruby, Rust, TypeScript, TSX, JavaScript, and JSX.

## Local loop

```sh
driftless init
driftless update
driftless check
driftless check --json
driftless coverage --include src/
driftless coverage --include src/ --json
```

`driftless init` prints a setup prompt for an agent. It does not write CI or project files.

Run `driftless update` only after the docs have been reviewed. It writes `.driftless.lock` when every ref resolves. Later, `driftless check` fails if a referenced symbol changes, disappears, or is not yet locked.

`driftless coverage --include src/` inverts the check: it reports public symbols under `src/` that are not covered by docs. Constructors, private names, crate-visible Rust items, and non-exported symbols are ignored.

## Output examples

`driftless init` prints a prompt and does not write files:

```text
Set up Driftless in this repository.

Goal: keep Markdown docs linked to source symbols so documentation drift is caught while code is being written.

Steps:
1. Inspect this repository's docs and source layout.
2. Add Markdown refs to important docs using inline refs like `src/lib.rs#symbol`, links like `[symbol](src/lib.rs#symbol)`, or fenced-code info strings like `rust ref=src/lib.rs#symbol`.
3. Run `driftless update` after the docs have been reviewed.
...
```

First lock after adding refs:

```text
locked   README.md:42 src/lib.rs#login
wrote /path/to/project/.driftless.lock
```

Clean check:

```text
driftless: ok
```

Signature drift:

```text
error    README.md:42 src/lib.rs#login signature changed (40d3a77b2e5d5eb7:9de37b2f0b2cbb6f -> 2a5f1a4c3bb0a212:9de37b2f0b2cbb6f); update docs, then `driftless update`
driftless: 1 error(s)
```

JSON output for an agent:

```json
[
  {
    "schema_version": 1,
    "status": "body_drift",
    "severity": "error",
    "blocks_exit": true,
    "ref": "src/lib.rs#login",
    "source_file": "src/lib.rs",
    "symbol": "login",
    "expected_hash": "40d3a77b2e5d5eb7:9de37b2f0b2cbb6f",
    "actual_hash": "40d3a77b2e5d5eb7:2711c50286f30f24",
    "doc": {
      "file": "README.md",
      "line": 42,
      "heading": "Authentication",
      "section": "## Authentication\n\nLogin is handled by `src/lib.rs#login`."
    },
    "symbol_source": "pub fn login(user: &str) -> bool {\n    user == \"root\"\n}\n"
  }
]
```

With `--warn-body --json`, body-only drift records keep the same shape but use `"severity": "warning"` and `"blocks_exit": false`.

Missing coverage:

```text
undocumented fn        src/lib.rs#logout
driftless: 1 undocumented public symbol(s)
```

Coverage JSON:

```json
[
  {
    "schema_version": 1,
    "status": "undocumented",
    "severity": "error",
    "blocks_exit": true,
    "kind": "fn",
    "source_file": "src/lib.rs",
    "symbol": "logout",
    "ref": "src/lib.rs#logout"
  }
]
```

## Agent repair

Use JSON output for automated repair loops:

```sh
driftless check --json
```

Each check record includes `schema_version`, `status`, `severity`, `blocks_exit`, `ref`, `source_file`, `symbol`, hash fields when relevant, the enclosing Markdown `doc` section, and current `symbol_source`. The deterministic core is `src/check.rs#run_check`; agent setup is intentionally just a prompt from `src/init.rs#print_prompt`.

## Editor

Driftless includes an LSP server over stdio:

```sh
driftless lsp
```

Neovim example:

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "markdown",
  callback = function()
    vim.lsp.start({
      name = "driftless",
      cmd = { "driftless", "lsp" },
      root_dir = vim.fs.root(0, { ".driftless.lock", ".git" }),
    })
  end,
})
```

The LSP uses the same checking path as the CLI. It can recheck open Markdown docs against unsaved source buffers, and capable clients get dynamic watchers for source files and `.driftless.lock`.

## More

- `docs/architecture.md` explains the implementation.
- `docs/development.md` explains repo checks, release flow, and dogfooding expectations.
- `docs/benchmarking.md` explains benchmark fixtures, limits, and local baselines.
- `CONTRIBUTING.md` is the short contributor entry point.
