# silo

Automated environment isolation for multi-agent development

Current LLM agents are powerful, but managing their environments is a chore. silo automates the "boring stuff"—cloning repos, setting up worktrees, and partitioning environments—so you can deploy a fleet of agents to solve tasks in parallel.

How it works:
1. Define Tasks: Feed a list of objectives via CLI or the Web UI.
1. Provision: The system automatically spins up isolated environments (Local Worktrees or Remote Containers).
1. Execute: Agents (Claude, Gemini, Codex) work independently without file conflicts.
1. Review: Merge the successful outputs back into your main branch.

## Usage

### Initialize silo

Optionally create a dedicated directory for all your worktrees:

```bash
silo init
```

This creates a `~/.silo/` directory in your home folder. Once initialized, all future worktrees will be created here by default instead of in the parent directory of each repository.

### Launch an agent

Launch an agent in an isolated git worktree:

```bash
silo launch
```

This creates a new worktree with a unique branch and starts a Claude session inside it.

The worktree location is determined by this priority:
1. Explicit `--worktree-base` argument (highest priority)
2. `~/.silo/` directory (if it exists from running `silo init`)
3. Parent directory of the repo (fallback)

#### Options

- `--worktree-base <path>` — Base directory for the worktree (overrides default)
- `--branch <name>` — Custom branch name (default: auto-generated from project name)
- `--agent <name>` — Agent to launch: `claude` or `opencode` (default: `claude`)
- `--tab` — Launch the agent in a new terminal tab instead of replacing the current process
- `--split-pane` — Launch the agent in a vertical split pane (iTerm2 only)

### List running agents

View all active agents running in worktrees of the current repository:

```bash
silo ps
```

### Show worktree status

View uncommitted changes and commits ahead/behind for each worktree:

```bash
silo status
```

Use `--all` to include clean worktrees.

### Clean up worktrees

Remove inactive worktrees where no agents are currently running:

```bash
silo cleanup
```

### Shell completions

Zsh
```bash
mkdir -p ~/.zsh/completions
silo completions zsh > ~/.zsh/completions/_silo
printf '\n# The following lines have been added by silo to enable CLI completions.\nfpath=(~/.zsh/completions $fpath)\nautoload -Uz compinit\ncompinit\n# End of silo completions' >> ~/.zshrc
```

Bash
```bash
silo completions bash > ~/.local/share/bash-completion/completions/silo
```

Fish
```bash
silo completions fish > ~/.config/fish/completions/silo.fish
```

## Development

### Prerequisites

- Rust 1.93.0 or later

### Pre-commit hooks

Install [pre-commit](https://pre-commit.com/) and set up the git hooks:

```bash
pip install pre-commit
pre-commit install
```

The hooks run `cargo fmt`, `clippy`, `check`, and `test` automatically on each commit.

### Build

```bash
cargo build
```

For a release build:

```bash
cargo build --release
```

### Test

```bash
cargo test
```
