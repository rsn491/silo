# silo

Isolated workspace manager for parallel agentic development. Silo lets you launch multiple AI agents — like Claude Code, Codex, and OpenCode — to work simultaneously on the same repository, each in its own isolated Git worktree or clone.

<kbd><img title="Silo launch" src="docs/silo_launch.gif"></kbd><br/>

## Installation

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/rsn491/silo/releases/latest/download/silo-installer.sh | sh
```

## Usage

### Initialize

Set up the `~/.silo/` directory and configure default preferences:

```sh
silo init [--agent <agent>] [--workspace-type <worktree|clone>] [--exit-work <true|false>]
```

When run without arguments in an interactive terminal, `init` walks you through setup. Preferences are saved to `~/.silo/settings.json`.

### Launch an agent

Create an isolated workspace and launch an AI agent inside it:

```sh
silo launch [--agent <agent>] [--branch <branch>] [--worktree|--clone] [--reuse] [--tab|--split-pane]
```

| Flag | Description |
|------|-------------|
| `--agent <agent>` | Agent to launch: `claude` (default), `opencode`, `codex`, `gemini`, `droid` |
| `--branch <branch>` | Custom branch name (default: auto-generated) |
| `--worktree` | Use a Git worktree (default) |
| `--clone` | Use a full local Git clone instead |
| `--reuse` | Reuse an existing inactive workspace if available |
| `--tab` | Open in a new terminal tab (iTerm2, kitty, WezTerm; opens a new window on Ghostty) |
| `--split-pane` | Open in a vertical split pane (iTerm2, kitty, WezTerm; not supported on Ghostty) |

When the agent exits, silo detects uncommitted changes and offers to commit, suggest a branch name, and push.

### List running agents

Show all AI agents currently running in silo workspaces:

```sh
silo ps
```

### Show workspace status

Display branch, commit, and change status across all workspaces:

```sh
silo status
```

### Switch into a workspace

Open an interactive shell inside a workspace directory:

```sh
silo checkout [workspace_id|branch_name]
```

The argument can be a workspace ID (directory name) or a branch name — directory name takes precedence if both match. If no argument is given, an interactive selector is displayed. Type `exit` to return to your original session.

### Clean up workspaces

Remove inactive workspaces. Workspaces with uncommitted changes are skipped unless `--force` is passed:

```sh
silo cleanup [--all] [--force] [--yes]
```

| Flag | Description |
|------|-------------|
| `--all` | Clean all worktrees in the repo, including non-silo ones |
| `--force` | Remove workspaces even with uncommitted or unpushed work |
| `--yes` | Skip the confirmation prompt |

### Shell completions

Generate shell completion scripts:

```sh
silo completions --shell bash
silo completions --shell zsh
silo completions --shell fish
```

## Dependencies

Silo requires the following to be installed:

- **Git** — for worktree and clone operations
- At least one AI agent CLI:
  - [Claude Code](https://claude.ai/code)
  - [OpenCode](https://opencode.ai)
  - [Codex](https://openai.com/index/openai-codex/)
  - [Gemini CLI](https://geminicli.com)
  - [Droid](https://factory.ai)

## Development

**Requirements**: Rust 1.93.0+ (see `rust-toolchain.toml`)

```sh
# Build
cargo build

# Run
cargo run -- <command>

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

Pre-commit hooks run `fmt`, `clippy`, `check`, and `test` automatically. To install them:

```sh
pip install pre-commit
pre-commit install
```
