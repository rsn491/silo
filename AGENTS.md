# Overview

**Purpose**: Silo manages isolated workspaces for multi-agent development

**Core workflow**: `silo launch` → agent works in isolated workspace

**Workspace kinds**: `Worktree` (default) or `Clone` (full repo copy)

**Configuration file**: `~/.silo/settings.json`

| **Attribute** | **Value** |
|---------------|-----------|
| **Language** | Rust 2024 edition (minimum 1.93.0) |
| **Error Handling** | `thiserror` derive macros + `From` implementations |
| **Testing** | Unit tests in-file (`#[cfg(test)]`) |
| **Quality Gates** | Pre-commit hooks: fmt, clippy, test |

# Architecture

- **CLI** (`src/main.rs` + `src/cli/`): command parsing (clap), wires concrete types
- **Services** (`src/services/`): generic business logic, workspace orchestration
- **Infra** (`src/infra/`): trait definitions + concrete impls (git, process, terminal, agent, osascript)

**CRITICAL**: Dependencies flow **downward only**. Services → CLI or Infrastructure → Services is **NEVER** allowed.

**CLI layer structure**: `main.rs` matches on `Commands` and wires concrete types. Each command has its own file in `src/cli/` (e.g. `cli/launch.rs` holds `LaunchArgs` and `LaunchCommand`).

# Where to Add Code

- **System interaction** (git, process, terminal) → `src/infra/`: trait + concrete impl + thiserror error type 
- **Business logic** → `src/services/`: generic struct with trait bounds + domain error enum
- **New CLI command** → `src/cli/<command>.rs` + `src/main.rs`: `XxxArgs` (clap derive), `XxxCommand` with `.run()`, add `Commands` variant + match arm in main.rs
