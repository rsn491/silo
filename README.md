# silo

A CLI tool for managing isolated Git workspaces for multi-agent development. Silo lets multiple AI agents (Claude Code, OpenCode, Codex) work simultaneously on the same repository without interfering with each other, by creating separate Git worktrees or clones for each agent.

## Usage

### Initialize

Set up the `~/.silo/` directory where workspaces will be stored:

```sh
silo init
```

### Launch an agent

Create an isolated workspace and launch an AI agent inside it:

```sh
silo launch                            # Launch Claude Code (default)
silo launch --agent opencode           # Launch OpenCode
silo launch --agent codex              # Launch Codex
silo launch --branch my-feature        # Use a specific branch name
silo launch --tab                      # Open in a new iTerm2 tab
silo launch --split-pane               # Open in a split iTerm2 pane
silo launch --checkout                 # Use git clone instead of worktree
```

### List running agents

Show all AI agents currently running in silo workspaces:

```sh
silo ps
```

### Show workspace status

Display the status of all worktrees with commit information:

```sh
silo status
```

### Clean up workspaces

Remove inactive workspaces. By default, workspaces with uncommitted changes are skipped:

```sh
silo cleanup                           # Remove inactive workspaces (prompts for confirmation)
silo cleanup --all                     # Remove all workspaces
silo cleanup --force                   # Remove even workspaces with uncommitted changes
silo cleanup -y                        # Skip confirmation prompt
```

### Shell completions

Generate shell completion scripts:

```sh
silo completions --shell bash          # Bash completions
silo completions --shell zsh           # Zsh completions
silo completions --shell fish          # Fish completions
```

## Dependencies

Silo requires the following to be installed:

- **Git** — for worktree and clone operations
- At least one AI agent CLI:
  - [Claude Code](https://claude.ai/code)
  - [OpenCode](https://opencode.ai)
  - [Codex](https://openai.com/index/openai-codex/)

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

### Project structure

```
src/
├── main.rs                 # CLI entry point, dependency injection
├── commands/               # Command handlers (launch, ps, cleanup, ...)
├── services/               # Business logic (generic, trait-bounded)
└── infra/                  # System interactions (git, process, terminal)
```

The codebase follows a three-layer architecture where dependencies flow strictly downward: CLI → Services → Infrastructure. Services use generic type parameters with trait bounds to keep business logic testable and decoupled from system calls.
