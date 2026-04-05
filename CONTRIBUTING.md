# Contributing to Silo

This guide is for both human contributors and AI agents. It covers architecture, coding patterns, testing, and how to extend the codebase.

---

## Table of Contents

1. [Project Mental Model](#1-project-mental-model)
2. [Architecture Layers](#2-architecture-layers)
3. [Coding Patterns](#3-coding-patterns)
   - [Trait-Based Infrastructure](#pattern-1-trait-based-infrastructure)
   - [Generic Services with Trait Bounds](#pattern-2-generic-services-with-trait-bounds)
   - [Custom Error Types with From Implementations](#pattern-3-custom-error-types-with-from-implementations)
   - [Hand-Written Mocks for Testing](#pattern-4-hand-written-mocks-for-testing)
4. [File Organization](#4-file-organization)
5. [Development Workflow](#5-development-workflow)
6. [Testing Guidelines](#6-testing-guidelines)
7. [Common Pitfalls](#7-common-pitfalls)
8. [Extending the Codebase](#8-extending-the-codebase)
   - [Adding a New Command](#adding-a-new-command)
   - [Adding a New Agent Type](#adding-a-new-agent-type)
   - [Adding a New Terminal](#adding-a-new-terminal)
   - [Adding New Infrastructure](#adding-new-infrastructure)
9. [Dependencies & Crates](#9-dependencies--crates)
10. [Code Navigation Guide](#10-code-navigation-guide)

---

## 1. Project Mental Model

**Purpose**: Silo manages isolated git worktrees for multi-agent development.

**Core Workflow**:
```
init → launch → work → cleanup
  ↓       ↓       ↓       ↓
Create  Create  Agent   Remove
.silo/  worktree  runs   unused
dir     + branch  here   worktrees
```

**Design Philosophy**:
- **Trait abstractions** for testability (mock any system interaction)
- **Generic types** for flexibility (services work with any trait implementation)
- **Custom errors** for each domain (`GitError`, `ProcessError`, `LaunchError`, `ListError`)
- **Composition** over inheritance (services compose infrastructure traits)

**Key Constraint**: macOS-focused (uses `osascript`, iTerm2)

---

## 2. Architecture Layers

```
┌─────────────────────────────────────────┐
│  CLI Layer (main.rs)                    │
│  - Command parsing (clap)               │
│  - Dependency injection                 │
│  - Concrete type instantiation          │
└─────────────────┬───────────────────────┘
                  │ calls
┌─────────────────▼───────────────────────┐
│  Service Layer (src/services/)          │
│  - Generic business logic               │
│  - Trait-bounded type parameters        │
│  - Domain errors                        │
└─────────────────┬───────────────────────┘
                  │ depends on
┌─────────────────▼───────────────────────┐
│  Infrastructure Layer (src/infra/)      │
│  - Trait definitions                    │
│  - Concrete implementations             │
│  - System interactions (git, ps, lsof)  │
└─────────────────────────────────────────┘
```

**CRITICAL RULE**: Dependencies flow **downward only**:
- CLI → Services → Infrastructure ✅
- Services → CLI ❌ (NEVER)
- Infrastructure → Services ❌ (NEVER)

Services accept **trait bounds**, not concrete types (except in `main.rs`).

---

## 3. Coding Patterns

### Pattern 1: Trait-Based Infrastructure

**DO** — Define trait in infrastructure layer:

```rust
// src/infra/git.rs
pub trait GitOperations {
    fn get_repo_root(&self) -> Result<PathBuf, GitError>;
    fn get_project_name(&self) -> Result<String, GitError>;
    fn create_worktree(&self, path: &Path, branch: &str) -> Result<(), GitError>;
    fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>, GitError>;
    fn remove_worktree(&self, path: &Path) -> Result<(), GitError>;
}
```

**DON'T** — Use concrete types directly in services:

```rust
// ❌ WRONG — not mockable
pub struct MyService {
    git: Git,
}
```

### Pattern 2: Generic Services with Trait Bounds

**DO** — Use generic type parameters with trait bounds:

```rust
// src/services/agent_list.rs
pub struct AgentListService<G: GitOperations, P: ProcessOperations> {
    git: G,
    process: P,
}

impl<G: GitOperations, P: ProcessOperations> AgentListService<G, P> {
    pub fn new(git: G, process: P) -> Self {
        Self { git, process }
    }
}
```

**DON'T** — Use trait objects or concrete types in service structs:

```rust
// ❌ WRONG — runtime overhead, harder to test
pub struct AgentListService {
    git: Box<dyn GitOperations>,
}
```

If the service needs to clone its dependencies, add `+ Clone` to the bound:

```rust
pub struct MyService<G: GitOperations + Clone> { git: G }
```

### Pattern 3: Custom Error Types with From Implementations

**DO** — Create domain-specific errors with conversion traits:

```rust
// src/services/agent_list.rs
#[derive(Debug)]
pub enum ListError {
    Git(GitError),
    Process(ProcessError),
}

impl std::fmt::Display for ListError { /* ... */ }
impl std::error::Error for ListError {}

impl From<GitError> for ListError {
    fn from(error: GitError) -> Self { ListError::Git(error) }
}
impl From<ProcessError> for ListError {
    fn from(error: ProcessError) -> Self { ListError::Process(error) }
}
```

This enables the **`?` operator** across error boundaries, replacing verbose `match` blocks.

### Pattern 4: Hand-Written Mocks for Testing

**DO** — Create simple struct mocks implementing traits:

```rust
// Place inline in the #[cfg(test)] module of the file under test
#[cfg(test)]
mod tests {
    use super::*;

    struct MockGit {
        worktrees: Vec<WorktreeInfo>,
    }

    impl GitOperations for MockGit {
        fn get_repo_root(&self) -> Result<PathBuf, GitError> {
            Ok(PathBuf::from("/repo"))
        }
        fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>, GitError> {
            Ok(self.worktrees.clone())
        }
        // implement remaining methods with Ok(()) / Ok(default)
    }
}
```

**DON'T** — Use mocking frameworks (not needed, adds complexity).

---

## 4. File Organization

### Where to Add Code

```
Adding new functionality?
├─ System interaction (git, process, terminal)?
│  └─ src/infra/
│     ├─ Define trait
│     ├─ Implement for concrete type
│     ├─ Add error type if needed
│     └─ Export in mod.rs
│
├─ Business logic (orchestration, algorithms)?
│  └─ src/services/
│     ├─ Create service struct with generic trait bounds
│     ├─ Implement methods using traits
│     ├─ Define service-specific error enum
│     └─ Export in mod.rs
│
└─ CLI command or user interface?
   └─ src/main.rs
      ├─ Add enum variant to Commands
      ├─ Add args struct if needed
      ├─ Wire dependencies with concrete types
      └─ Call service methods
```

### Module Conventions

**Infrastructure Layer** (`src/infra/`):
- Each module defines one or more related traits with a concrete implementation in the same file
- Error types in separate files (e.g., `git_error.rs`)
- `mod.rs` re-exports all public items

**Service Layer** (`src/services/`):
- Each service in its own file
- Service-specific error enums defined in the same file
- `mod.rs` re-exports all public items

**Tests**: always at the bottom of the implementation file inside `#[cfg(test)]`.

---

## 5. Development Workflow

### Making Changes

1. **Read** relevant code first (understand before modifying)
2. **Code** following the patterns in [Section 3](#3-coding-patterns)
3. **Test** with hand-written mocks (see [Section 6](#6-testing-guidelines))
4. **Run quality checks**:
   ```bash
   cargo fmt      # Format code
   cargo clippy   # Lint
   cargo check    # Fast compile check
   cargo test     # Run tests
   ```
5. **Commit** — pre-commit hooks run the above automatically

### Adding a Feature

1. Identify the layer (infra / service / CLI) using [Section 4](#4-file-organization)
2. Define traits if adding infrastructure
3. Implement following the patterns in [Section 3](#3-coding-patterns)
4. Wire concrete dependencies in `main.rs`
5. Add tests with mocks
6. Update `mod.rs` to export new items

---

## 6. Testing Guidelines

### Test Location

Always place tests at the bottom of the implementation file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    // ...
}
```

### Service Tests

Mock all trait dependencies and test through the public service API:

```rust
#[test]
fn test_list_running_agents_in_worktrees() {
    let mock_git = MockGit {
        worktrees: vec![
            WorktreeInfo { path: PathBuf::from("/repo/wt1"), branch: Some("feat-1".to_string()) },
        ],
    };
    let mock_process = MockProcess {
        processes: vec![(123, "/usr/bin/claude".to_string())],
        cwds: vec![(123, PathBuf::from("/repo/wt1"))],
    };

    let service = AgentListService::new(mock_git, mock_process);
    let agents = service.list_running_agents().unwrap();

    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0].pid, 123);
}
```

### Parser Tests

Test pure functions with sample input covering happy path and edge cases:

```rust
#[test]
fn test_parse_worktree_list_multiple_worktrees() {
    let output = "worktree /path/to/main\nHEAD abc123\nbranch refs/heads/main\n\n";
    let worktrees = parse_worktree_list(output);
    assert_eq!(worktrees.len(), 1);
    assert_eq!(worktrees[0].branch, Some("main".to_string()));
}
```

### Coverage Expectations

- All public service methods
- Parser functions: happy path + edge cases
- Error paths (e.g., command failures)
- Empty / null cases

---

## 7. Common Pitfalls

### Using Concrete Types in Services

```rust
// ❌ WRONG
pub struct MyService { git: Git }

// ✅ CORRECT
pub struct MyService<G: GitOperations> { git: G }
```

### Missing Clone Bound

```rust
// ❌ Compile error if you try to .clone() later
pub struct MyService<G: GitOperations> { git: G }

// ✅ Add Clone when the service needs to clone its dependency
pub struct MyService<G: GitOperations + Clone> { git: G }
```

### Forgetting From Implementations

```rust
// ❌ Verbose manual conversion
let worktrees = match self.git.list_worktrees() {
    Ok(w) => w,
    Err(e) => return Err(ListError::Git(e)),
};

// ✅ Implement From once, then use ?
let worktrees = self.git.list_worktrees()?;
```

### Forgetting to Update mod.rs

After creating a new file, export it:

```rust
// src/services/mod.rs
mod my_new_service;
pub use my_new_service::*;
```

### Using Command::new Directly in Services

```rust
// ❌ WRONG — not mockable
use std::process::Command;
fn my_method(&self) { Command::new("git")... }

// ✅ CORRECT — use the trait abstraction
fn my_method<G: GitOperations>(&self, git: &G) { git.get_repo_root()?; }
```

---

## 8. Extending the Codebase

### Adding a New Command

```rust
// 1. Add enum variant to Commands in main.rs
#[derive(Subcommand)]
pub enum Commands {
    Launch(LaunchArgs),
    Ps,
    MyCommand(MyCommandArgs),  // ← Add this
}

// 2. Define args struct
#[derive(Parser, Debug)]
pub struct MyCommandArgs {
    #[arg(long)]
    pub some_option: Option<String>,
}

// 3. Add match arm in main()
Commands::MyCommand(args) => {
    let git = Git;
    let service = MyService::new(git);
    service.do_something()?;
}
```

### Adding a New Agent Type

```rust
// src/infra/agent.rs
#[derive(Debug, Clone, PartialEq, Eq, Display, EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum Agent {
    Claude,
    Opencode,
    MyNewAgent,  // ← Add variant
}

impl Agent {
    pub fn command(&self) -> &str {
        match self {
            Agent::Claude => "claude",
            Agent::Opencode => "opencode",
            Agent::MyNewAgent => "my-new-agent",  // ← Add command
        }
    }
}
```

### Adding a New Terminal

```rust
// 1. Add variant to TerminalKind in src/infra/terminal/mod.rs
pub enum TerminalKind { ITerm2, MyTerminal }

// 2. Create src/infra/terminal/my_terminal.rs
pub struct MyTerminal;

impl Terminal for MyTerminal {
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), LaunchError> { todo!() }
    fn split_pane(&self, worktree_path: &Path, agent: &Agent) -> Result<(), LaunchError> { todo!() }
}

// 3. Update detect_terminal() to recognize the new terminal
Some("myterminal") => TerminalKind::MyTerminal,

// 4. Update create_terminal() to construct the new type
TerminalKind::MyTerminal => Box::new(MyTerminal),
```

### Adding New Infrastructure

```rust
// 1. Create src/infra/my_infrastructure.rs

pub trait MyOperations {
    fn operation_one(&self) -> Result<String, MyError>;
    fn operation_two(&self, arg: &str) -> Result<(), MyError>;
}

#[derive(Debug)]
pub enum MyError { OperationFailed(String) }
impl std::fmt::Display for MyError { /* ... */ }
impl std::error::Error for MyError {}

#[derive(Default, Clone)]
pub struct MyInfra;

impl MyOperations for MyInfra {
    fn operation_one(&self) -> Result<String, MyError> { Ok("result".to_string()) }
    fn operation_two(&self, _arg: &str) -> Result<(), MyError> { Ok(()) }
}

#[cfg(test)]
mod tests { /* ... */ }

// 2. Export in src/infra/mod.rs
mod my_infrastructure;
pub use my_infrastructure::*;
```

---

## 9. Dependencies & Crates

### Core Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `clap` | 4.5 | CLI argument parsing (with `derive` feature) |
| `uuid` | 1.11 | Unique worktree identifiers (with `v4` feature) |
| `dirs` | 5.0 | Cross-platform home directory |
| `strum` | 0.26 | Enum utilities (string conversion, iteration) |

### Standard Library Patterns

```rust
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fmt;
use std::error::Error;
```

- Use `std::process::Command` for git, ps, lsof — always in infra, never in services
- Always check `output.status.success()`
- Parse output with `String::from_utf8_lossy()`

### When to Add Dependencies

Prefer the standard library. Only reach for external crates for the use cases covered by the existing four above. Avoid adding HTTP clients, async runtimes, or mocking frameworks.

---

## 10. Code Navigation Guide

| Pattern | File |
|---------|------|
| Generic service with full tests | `src/services/agent_list.rs` |
| Trait definition + implementation | `src/infra/git.rs` |
| Parser with comprehensive tests | `src/infra/git.rs` |
| Dependency injection in CLI | `src/main.rs` |
| Simple generic service | `src/services/git_worktree_workspace.rs` |
| Process operations trait | `src/infra/process.rs` |
| Terminal trait + detection | `src/infra/terminal/mod.rs` |
| Hand-written mocks | `src/services/agent_list.rs` |
| Error type with From impls | `src/services/agent_list.rs` |
