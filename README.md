# silo

Multi-Agent Orchestration with Absolute Isolation.

Current LLM agents are powerful, but managing their environments is a chore. silo automates the "boring stuff"—cloning repos, setting up worktrees, and partitioning environments—so you can deploy a fleet of agents to solve tasks in parallel.

How it works:
1. Define Tasks: Feed a list of objectives via CLI or the Web UI.
1. Provision: The system automatically spins up isolated environments (Local Worktrees or Remote Containers).
1. Execute: Agents (Claude, Gemini, Codex) work independently without file conflicts.
1. Review: Merge the successful outputs back into your main branch.

## Development

### Prerequisites

- Rust 1.93.0 or later

### Build

```bash
cargo build
```

For a release build:

```bash
cargo build --release
```

### Run

```bash
cargo run
```

### Test

```bash
cargo test
```
