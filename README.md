# silo

Automated environment isolation for multi-agent development

Current LLM agents are powerful, but managing their environments is a chore. silo automates the "boring stuff"—cloning repos, setting up worktrees, and partitioning environments—so you can deploy a fleet of agents to solve tasks in parallel.

How it works:
1. Define Tasks: Feed a list of objectives via CLI or the Web UI.
1. Provision: The system automatically spins up isolated environments (Local Worktrees or Remote Containers).
1. Execute: Agents (Claude, Gemini, Codex) work independently without file conflicts.
1. Review: Merge the successful outputs back into your main branch.

## Usage

Launch an agent in an isolated git worktree:

```bash
silo launch
```

This creates a new worktree (in the parent directory of the repo by default) with a unique branch and starts a Claude session inside it.

### Options

- `--worktree-base <path>` — Base directory for the worktree (default: parent of repo)
- `--branch <name>` — Custom branch name (default: auto-generated from project name)
- `--tab` — Launch the agent in a new terminal tab instead of replacing the current process
- `--split-pane` — Launch the agent in a vertical split pane (iTerm2 only)

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
