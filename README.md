# aigit

`aigit` is a git-like CLI that blocks commits unless the committer can pass a deterministic “Proof-of-Understanding” (PoU) exam and an audit transcript is attached to the resulting commit.

Product requirements live in `docs/aigit.adoc`.

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

Set `provider = "codex-cli"` in `.aigit.toml` to grade answers via `codex exec` (non-interactive).

## Hook (optional)

Install a `pre-commit` hook that blocks `git commit` unless it was invoked through `aigit commit`:

```sh
aigit install-hook --mode pre-commit
```
