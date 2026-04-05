# Contributing to Silo

## Setup

### Prerequisites

- Rust 1.93.0+ (specified in `rust-toolchain.toml`)
- [pre-commit](https://pre-commit.com/) for git hooks

### Install Pre-commit Hooks

```sh
pip install pre-commit
pre-commit install
```

The following hooks run automatically on every commit:

| Hook | Command | Purpose |
|------|---------|---------|
| cargo-fmt | `cargo fmt --all` | Enforce code formatting |
| cargo-clippy | `cargo clippy -- -D warnings` | Lint with warnings as errors |
| cargo-check | `cargo check` | Fast type checking |
| cargo-test | `cargo test --bins` | Run unit tests |

### Build and Run

```sh
cargo build          # Compile the project
cargo run -- <cmd>   # Run with a subcommand (e.g. launch, ps, cleanup)
```

---

## Testing

### Unit Tests

Unit tests live inline at the bottom of each source file inside `#[cfg(test)]` modules.

```sh
cargo test --bins    # Run unit tests only
cargo test           # Run all tests (unit + integration)
```

Tests use trait mocking (via `mockall`) to isolate business logic from system dependencies. Each service test creates mock implementations of infrastructure traits (`GitOperations`, `ProcessOperations`, etc.) and verifies behavior without touching the real filesystem or processes.

**Example**: `src/services/agent_list_service.rs` contains tests that mock git worktree listing and process lookup to verify agent discovery logic.

### Integration Tests

Integration tests live in the `tests/` directory:

- `tests/integration_test.rs` - End-to-end test that creates a temporary git repo, launches a workspace, and verifies worktree creation.

```sh
cargo test -- --ignored --test-threads=1   # Run integration tests (disabled by default)
```

Integration tests are marked `#[ignore]` because they require external tools (e.g. an AI agent binary) to be installed.

---

## Structure

Silo uses a three-layer architecture with strict downward-only dependencies:

```
CLI (src/cli/)  -->  Services (src/services/)  -->  Infrastructure (src/infra/)
```

### Layers

**CLI** (`src/cli/`) - Command handlers and user-facing output.
- Parses arguments (via `clap`), calls services, and formats output.
- **Only this layer prints to stdout/stderr.** Services and infrastructure never use `println!`, `eprintln!`, or any other direct output. All user-facing messages originate from CLI command handlers.

**Services** (`src/services/`) - Business logic and orchestration.
- Uses generic type parameters bounded by infrastructure traits (e.g. `<G: GitOperations>`).
- Contains domain-specific error types with `From` implementations for the `?` operator.
- Has no knowledge of how output is displayed.

**Infrastructure** (`src/infra/`) - System interactions and trait definitions.
- Defines traits (`GitOperations`, `ProcessOperations`, `Terminal`) and their concrete implementations.
- Wraps external commands (git, ps, lsof) and OS-specific APIs (osascript).

### Dependency Rules

| Direction | Allowed |
|-----------|---------|
| CLI -> Services -> Infrastructure | Yes |
| Services -> CLI | **Never** |
| Infrastructure -> Services | **Never** |
| Infrastructure -> CLI | **Never** |

`src/main.rs` is the composition root: it constructs concrete infrastructure types and injects them into services, which are then called by CLI handlers.

### Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, dependency injection, command routing |
| `src/cli/` | Command handlers (launch, ps, cleanup, checkout, init, status) |
| `src/services/agent_launcher.rs` | Workspace creation and agent spawning |
| `src/services/agent_list_service.rs` | Running agent discovery |
| `src/infra/git.rs` | Git operations trait + implementation |
| `src/infra/system_process.rs` | Process operations trait + implementation |
| `src/infra/terminal/` | Terminal abstraction and iTerm2 support |
