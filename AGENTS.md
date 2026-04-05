# AGENTS.md — AI Developer Onboarding Guide

Quick-reference index for AI agents contributing to Silo. For full details on patterns, workflows, and templates, see [CONTRIBUTING.md](CONTRIBUTING.md).

---

## Quick Reference

| Attribute | Value |
|-----------|-------|
| **Language** | Rust 2024 edition (minimum 1.93.0) |
| **Architecture** | Layered: Infrastructure → Services → CLI |
| **Core Pattern** | Trait-based abstractions with generics |
| **Error Handling** | Custom error enums per domain with `From` implementations |
| **Testing** | Hand-written mocks, unit tests in-file (`#[cfg(test)]`) |
| **Quality Gates** | Pre-commit hooks: fmt, clippy, check, test |

### Commands

| Command | Purpose |
|---------|---------|
| `cargo build` | Compile |
| `cargo test` | Run all tests |
| `cargo fmt` | Format code |
| `cargo clippy` | Lint |
| `cargo check` | Fast compile check |

---

## Architecture at a Glance

```
CLI (main.rs)  →  Services (src/services/)  →  Infrastructure (src/infra/)
```

Dependencies flow **downward only**. Services never import from CLI; infrastructure never imports from services.

See [Architecture Layers](CONTRIBUTING.md#2-architecture-layers) for the full diagram.

---

## Key Source Files

| File | Purpose |
|------|---------|
| `src/main.rs` | CLI entry point, dependency injection |
| `src/infra/git.rs` | Git operations trait + implementation |
| `src/infra/process.rs` | Process operations trait + implementation |
| `src/infra/terminal/mod.rs` | Terminal trait, detection, creation |
| `src/infra/terminal/iterm2.rs` | iTerm2-specific terminal implementation |
| `src/infra/agent.rs` | Agent enum (Claude, Opencode) |
| `src/services/agent_list.rs` | List running agents in worktrees |
| `src/services/agent_launcher.rs` | Launch agent in worktree |
| `src/services/git_worktree_workspace.rs` | Create git worktree workspace |
| `src/services/worktree_cleanup.rs` | Remove unused worktrees |
| `src/services/silo_config.rs` | Configuration and path resolution |
| `src/infra/mod.rs` | Infrastructure layer exports |
| `src/services/mod.rs` | Service layer exports |
| `Cargo.toml` | Package manifest |
| `.pre-commit-config.yaml` | Pre-commit hooks |

---

## Quick Start

**Understand the project** (2-minute scan):
1. [Project Mental Model](CONTRIBUTING.md#1-project-mental-model) — purpose and workflow
2. [Architecture Layers](CONTRIBUTING.md#2-architecture-layers) — layer structure and dependency rule
3. [Coding Patterns](CONTRIBUTING.md#3-coding-patterns) — trait, generics, error, mock patterns

**Add a feature**:
1. [File Organization](CONTRIBUTING.md#4-file-organization) — identify the right layer
2. [Extending the Codebase](CONTRIBUTING.md#8-extending-the-codebase) — templates for commands, agents, terminals, infra
3. [Common Pitfalls](CONTRIBUTING.md#7-common-pitfalls) — avoid known mistakes

**Fix a bug**:
1. Use the Key Source Files table above to locate the relevant file
2. [Development Workflow](CONTRIBUTING.md#5-development-workflow) — checklist for making and verifying changes

**Write tests**:
- [Testing Guidelines](CONTRIBUTING.md#6-testing-guidelines) — placement, mock patterns, coverage expectations

**Add a dependency**:
- [Dependencies & Crates](CONTRIBUTING.md#9-dependencies--crates) — what exists and when to add more
