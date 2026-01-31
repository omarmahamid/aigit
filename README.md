# aigit

`aigit` is a git-like CLI that blocks commits unless the committer can pass a deterministic “Proof-of-Understanding” (PoU) exam and an audit transcript is attached to the resulting commit.

Product requirements live in `docs/aigit.adoc`.

## Quickstart (day-to-day)

- Make a change (bugfix/refactor/etc.)
- Stage it: `git add -A`
- Commit via aigit: `aigit commit -m "your message"`
- Answer the prompts (end each answer with a line containing just `.`)
- (Optional) Verify later: `aigit verify HEAD`

## MVP commands

- `aigit exam` (default: staged diff; `--format tui|json`)
- `aigit commit` (runs exam, then delegates to `git commit` on pass, and stores transcript in git notes)
- `aigit verify <commit-ish>` (validates transcript presence + diff fingerprint + thresholds)
- `aigit policy validate` / `aigit config set <key> <value>` (minimal policy support via `.aigit.toml`)

## Install

### From GitHub Releases (recommended)

Download the binary from the repo’s GitHub Releases page and put it on your `PATH` (e.g. `~/.local/bin`).

Or use the installer script (edit `AIGIT_REPO` if your repo slug differs):

```sh
curl -fsSL https://raw.githubusercontent.com/omarmahamid/aigit/main/scripts/install.sh | sh
```

### From source (Rust)

```sh
cargo install --path .
```

## Using Codex CLI as the grader

- Install Codex CLI (`codex`) and login (so `codex exec "hello"` works).
- In your repo, create `.aigit.toml`:

```toml
provider = "codex-cli"

[codex_cli]
command = "codex" # or: "npx -y @openai/codex@0.75.0"
sandbox = "read-only"
timeout_secs = 120
```

- Use it normally:
  - `git add -A`
  - `aigit commit -m "message"`

## Hook (optional)

Install a `pre-commit` hook that blocks `git commit` unless it was invoked through `aigit commit`:

```sh
aigit install-hook --mode pre-commit
```
