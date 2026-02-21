# AGENTS.md — AI Developer Onboarding Guide

**Purpose**: This document helps AI agents understand Silo's architecture and contribute effectively to the codebase.

---

## 1. Quick Reference Card

| **Attribute** | **Value** |
|---------------|-----------|
| **Language** | Rust 2024 edition (minimum 1.93.0) |
| **Architecture** | Layered: Infrastructure → Services → CLI |
| **Core Pattern** | Trait-based abstractions with generics |
| **Error Handling** | Custom error enums per domain with `From` implementations |
| **Testing** | Hand-written mocks, unit tests in-file (`#[cfg(test)]`) |
| **Quality Gates** | Pre-commit hooks: fmt, clippy, check, test |

### Common Commands

| **Command** | **Purpose** |
|-------------|-------------|
| `cargo build` | Compile the project |
| `cargo test` | Run all tests |
| `cargo fmt` | Format code |
| `cargo clippy` | Lint code |
| `cargo check` | Fast compile check |

---

## 2. Project Mental Model

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
- **Custom errors** for each domain (GitError, ProcessError, LaunchError, ListError)
- **Composition** over inheritance (services compose infrastructure traits)

**Key Constraint**: macOS-focused (uses `osascript`, iTerm2)

---

## 3. Architecture Layers

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

**Service Pattern**: Services accept **trait bounds**, not concrete types (except in `main.rs`).

---

## 4. Critical Coding Patterns

### Pattern 1: Trait-Based Infrastructure

**DO** — Define trait in infrastructure layer:

```rust
// src/infra/git.rs:12-18
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
// ❌ WRONG
pub struct MyService {
    git: Git,  // Concrete type - not mockable!
}
```

### Pattern 2: Generic Services with Trait Bounds

**DO** — Use generic type parameters with trait bounds:

```rust
// src/services/agent_list.rs:45-53
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

**DON'T** — Use trait objects or concrete types:

```rust
// ❌ WRONG
pub struct AgentListService {
    git: Box<dyn GitOperations>,  // Harder to test, runtime overhead
}
```

### Pattern 3: Custom Error Types with From Implementations

**DO** — Create domain-specific errors with conversion traits:

```rust
// src/services/agent_list.rs:16-43
#[derive(Debug)]
pub enum ListError {
    Git(GitError),
    Process(ProcessError),
}

impl std::fmt::Display for ListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ListError::Git(e) => write!(f, "Git error: {}", e),
            ListError::Process(e) => write!(f, "Process error: {}", e),
        }
    }
}

impl std::error::Error for ListError {}

impl From<GitError> for ListError {
    fn from(error: GitError) -> Self {
        ListError::Git(error)
    }
}

impl From<ProcessError> for ListError {
    fn from(error: ProcessError) -> Self {
        ListError::Process(error)
    }
}
```

This enables **`?` operator** across error boundaries.

### Pattern 4: Hand-Written Mocks for Testing

**DO** — Create simple struct mocks implementing traits:

```rust
// src/services/agent_list.rs:120-168
#[cfg(test)]
mod tests {
    use super::*;

    // Mock GitOperations
    struct MockGit {
        worktrees: Vec<WorktreeInfo>,
    }

    impl GitOperations for MockGit {
        fn get_repo_root(&self) -> Result<PathBuf, GitError> {
            Ok(PathBuf::from("/repo"))
        }

        fn get_project_name(&self) -> Result<String, GitError> {
            Ok("test-project".to_string())
        }

        fn create_worktree(&self, _path: &Path, _branch: &str) -> Result<(), GitError> {
            Ok(())
        }

        fn list_worktrees(&self) -> Result<Vec<WorktreeInfo>, GitError> {
            Ok(self.worktrees.clone())
        }

        fn remove_worktree(&self, _path: &Path) -> Result<(), GitError> {
            Ok(())
        }
    }

    // Mock ProcessOperations
    struct MockProcess {
        processes: Vec<(u32, String)>,
        cwds: Vec<(u32, PathBuf)>,
    }

    impl ProcessOperations for MockProcess {
        fn find_processes_by_names(
            &self,
            _names: &[&str],
        ) -> Result<Vec<(u32, String)>, ProcessError> {
            Ok(self.processes.clone())
        }

        fn get_process_cwd(&self, pid: u32) -> Result<PathBuf, ProcessError> {
            self.cwds
                .iter()
                .find(|(p, _)| *p == pid)
                .map(|(_, path)| path.clone())
                .ok_or_else(|| ProcessError::CommandFailed("Process not found".to_string()))
        }
    }
}
```

**DON'T** — Use mocking frameworks (not needed, adds complexity).

---

## 5. File Organization Map

### Decision Tree: Where to Add Code

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

### Module Pattern Guidelines

**Infrastructure Layer** (`src/infra/`):
- Each module defines one or more related traits
- Concrete implementations in same file as trait
- Error types in separate files (e.g., `git_error.rs`)
- `mod.rs` re-exports public items

**Service Layer** (`src/services/`):
- Each service in its own file
- Generic type parameters with trait bounds
- Service-specific error enums in same file
- `mod.rs` re-exports public items

**Tests**:
- Place at bottom of implementation file
- Use `#[cfg(test)]` module
- Mock implementations defined inline

---

## 6. Development Workflow

### Making Changes Checklist

1. **Read** relevant code first (understand before modifying)
2. **Code** following existing patterns
3. **Test** with hand-written mocks
4. **Run quality checks**:
   ```bash
   cargo fmt      # Format code
   cargo clippy   # Check for issues
   cargo check    # Fast compile check
   cargo test     # Run tests
   ```
5. **Commit** (pre-commit hooks run automatically)

### Adding Features Checklist

1. **Identify layer** (infrastructure, service, or CLI)
2. **Define traits** (if adding infrastructure)
3. **Implement** following patterns above
4. **Wire dependencies** in `main.rs`
5. **Add tests** with mocks
6. **Update mod.rs** to export new items

---

## 7. Testing Guidelines

### Test Location

**Always** place tests at the bottom of the implementation file:

```rust
// Implementation code above...

#[cfg(test)]
mod tests {
    use super::*;
    // Test code here
}
```

### Mock Pattern

**Service Tests** — Mock all trait dependencies:

```rust
// src/services/agent_list.rs:170-206
#[test]
fn test_list_running_agents_in_worktrees() {
    let mock_git = MockGit {
        worktrees: vec![
            WorktreeInfo {
                path: PathBuf::from("/repo/worktree1"),
                branch: Some("feature-1".to_string()),
            },
            WorktreeInfo {
                path: PathBuf::from("/repo/worktree2"),
                branch: Some("feature-2".to_string()),
            },
        ],
    };

    let mock_process = MockProcess {
        processes: vec![
            (123, "/usr/bin/claude --args".to_string()),
            (456, "/usr/bin/claude --other".to_string()),
        ],
        cwds: vec![
            (123, PathBuf::from("/repo/worktree1")),
            (456, PathBuf::from("/repo/worktree2")),
        ],
    };

    let service = AgentListService::new(mock_git, mock_process);
    let agents = service.list_running_agents().unwrap();

    assert_eq!(agents.len(), 2);
    assert_eq!(agents[0].pid, 123);
    assert_eq!(agents[0].worktree_path, PathBuf::from("/repo/worktree1"));
    assert_eq!(agents[0].branch, Some("feature-1".to_string()));
}
```

**Parser Tests** — Test pure functions with sample input:

```rust
// src/infra/git.rs:193-210
#[test]
fn test_parse_worktree_list_multiple_worktrees() {
    let output = "\
worktree /path/to/main
HEAD abc123def456
branch refs/heads/main

worktree /path/to/feature
HEAD 789ghi012jkl
branch refs/heads/feature-branch

";
    let worktrees = parse_worktree_list(output);
    assert_eq!(worktrees.len(), 2);
    assert_eq!(worktrees[0].path, PathBuf::from("/path/to/main"));
    assert_eq!(worktrees[0].branch, Some("main".to_string()));
}
```

### Test Coverage Expectations

- **All public service methods** should have tests
- **Parser functions** should test happy path + edge cases
- **Error paths** should be tested (e.g., command failures)
- **Empty/null cases** should be covered

---

## 8. Common Pitfalls & Solutions

### Pitfall 1: Using Concrete Types in Services

**❌ WRONG**:
```rust
pub struct MyService {
    git: Git,  // Concrete type
}
```

**✅ CORRECT**:
```rust
pub struct MyService<G: GitOperations> {
    git: G,  // Generic with trait bound
}
```

### Pitfall 2: Missing Clone Bound for Stored Generics

**❌ WRONG**:
```rust
pub struct MyService<G: GitOperations> {
    git: G,  // Error if trying to .clone() later
}
```

**✅ CORRECT**:
```rust
pub struct MyService<G: GitOperations + Clone> {
    git: G,  // Can be cloned
}
```

### Pitfall 3: Forgetting From Implementations

**❌ WRONG**:
```rust
// Manually convert every error
let worktrees = match self.git.list_worktrees() {
    Ok(w) => w,
    Err(e) => return Err(ListError::Git(e)),  // Verbose!
};
```

**✅ CORRECT**:
```rust
// Implement From trait once
impl From<GitError> for ListError {
    fn from(error: GitError) -> Self {
        ListError::Git(error)
    }
}

// Then use ? operator
let worktrees = self.git.list_worktrees()?;  // Clean!
```

### Pitfall 4: Forgetting to Update mod.rs

After creating a new file, **always** export it in `mod.rs`:

```rust
// src/services/mod.rs
mod agent_launcher;
mod agent_list;
mod my_new_service;  // ← Add this!

pub use agent_launcher::*;
pub use agent_list::*;
pub use my_new_service::*;  // ← And this!
```

### Pitfall 5: Using Command::new Directly in Services

**❌ WRONG**:
```rust
// In a service file
use std::process::Command;

pub fn my_method(&self) {
    let output = Command::new("git")...  // Not mockable!
}
```

**✅ CORRECT**:
```rust
// Use trait abstraction
pub fn my_method<G: GitOperations>(&self, git: &G) {
    let result = git.get_repo_root()?;  // Mockable!
}
```

---

## 9. Code Navigation Guide

### Key Implementation Examples

| **Pattern** | **File Location** |
|-------------|-------------------|
| Generic service with full tests | `/Users/ricardo/code_repos/silo/src/services/agent_list.rs:45-281` |
| Trait definition + implementation | `/Users/ricardo/code_repos/silo/src/infra/git.rs:12-102` |
| Parser with comprehensive tests | `/Users/ricardo/code_repos/silo/src/infra/git.rs:121-243` |
| Dependency injection in CLI | `/Users/ricardo/code_repos/silo/src/main.rs:74-146` |
| Simple generic service | `/Users/ricardo/code_repos/silo/src/services/git_worktree_workspace.rs:10-56` |
| Process operations trait | `/Users/ricardo/code_repos/silo/src/infra/process.rs:21-60` |
| Terminal trait + detection | `/Users/ricardo/code_repos/silo/src/infra/terminal/mod.rs:15-49` |
| Hand-written mocks | `/Users/ricardo/code_repos/silo/src/services/agent_list.rs:120-168` |
| Error type with From impls | `/Users/ricardo/code_repos/silo/src/services/agent_list.rs:16-43` |

---

## 10. Extending the Codebase

### Adding a New Command

**Template**:

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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::MyCommand(args) => {
            // Instantiate concrete dependencies
            let git = Git;
            let service = MyService::new(git);

            // Call service method
            service.do_something()?;
        }
        // ... other commands
    }

    Ok(())
}
```

### Adding a New Agent Type

**Template**:

```rust
// src/infra/agent.rs
use strum::{Display, EnumString};

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

**Template**:

```rust
// 1. Add to TerminalKind enum in src/infra/terminal/mod.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalKind {
    ITerm2,
    MyTerminal,  // ← Add this
}

// 2. Create implementation file src/infra/terminal/my_terminal.rs
use super::Terminal;
use crate::infra::agent::Agent;
use crate::services::agent_launcher::LaunchError;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct MyTerminal;

impl Terminal for MyTerminal {
    fn open_tab(&self, worktree_path: &Path, agent: &Agent) -> Result<(), LaunchError> {
        // Implementation
    }

    fn split_pane(&self, worktree_path: &Path, agent: &Agent) -> Result<(), LaunchError> {
        // Implementation
    }
}

// 3. Update detect_terminal() function to recognize new terminal
pub fn detect_terminal() -> Result<Box<dyn Terminal>, LaunchError> {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    let kind = match term_program.as_deref() {
        Some("iterm2") => TerminalKind::ITerm2,
        Some("myterminal") => TerminalKind::MyTerminal,  // ← Add detection
        // ...
    };
    Ok(create_terminal(&kind))
}

// 4. Update create_terminal() to construct new type
pub fn create_terminal(kind: &TerminalKind) -> Box<dyn Terminal> {
    match kind {
        TerminalKind::ITerm2 => Box::new(ITerm2),
        TerminalKind::MyTerminal => Box::new(MyTerminal),  // ← Add creation
    }
}
```

### Adding Infrastructure (General Pattern)

**Template**:

```rust
// 1. Create src/infra/my_infrastructure.rs

// Define trait
pub trait MyOperations {
    fn operation_one(&self) -> Result<String, MyError>;
    fn operation_two(&self, arg: &str) -> Result<(), MyError>;
}

// Define error type
#[derive(Debug)]
pub enum MyError {
    OperationFailed(String),
}

impl std::fmt::Display for MyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MyError::OperationFailed(msg) => write!(f, "Operation failed: {}", msg),
        }
    }
}

impl std::error::Error for MyError {}

// Implement trait for concrete type
#[derive(Default, Clone)]
pub struct MyInfra;

impl MyOperations for MyInfra {
    fn operation_one(&self) -> Result<String, MyError> {
        // Use std::process::Command or other system calls
        Ok("result".to_string())
    }

    fn operation_two(&self, arg: &str) -> Result<(), MyError> {
        // Implementation
        Ok(())
    }
}

// Add tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_one() {
        // Test implementation
    }
}

// 2. Export in src/infra/mod.rs
mod my_infrastructure;
pub use my_infrastructure::*;
```

---

## 11. Dependencies & Crates

### Core Dependencies

| **Crate** | **Version** | **Purpose** |
|-----------|-------------|-------------|
| `clap` | 4.5 | CLI argument parsing (with `derive` feature) |
| `uuid` | 1.11 | Generate unique worktree identifiers (with `v4` feature) |
| `dirs` | 5.0 | Cross-platform directory paths (home dir) |
| `strum` | 0.26 | Enum utilities (string conversion, iteration) |

### Standard Library Patterns

**Common imports**:
```rust
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fmt;
use std::error::Error;
```

**System commands**:
- Use `std::process::Command` for git, ps, lsof
- Always check `output.status.success()`
- Parse stdout/stderr with `String::from_utf8_lossy()`

### When to Add Dependencies

**Prefer standard library** unless:
- Complex CLI parsing → `clap`
- UUID generation → `uuid`
- Cross-platform paths → `dirs`
- Enum utilities → `strum`

**Avoid** adding dependencies for:
- Simple string parsing (use stdlib)
- HTTP clients (out of scope)
- Async runtime (not needed)

---

## 12. Key Files Reference

### Core Implementation Files

| **File** | **Purpose** |
|----------|-------------|
| `/Users/ricardo/code_repos/silo/src/main.rs` | CLI entry point, dependency injection |
| `/Users/ricardo/code_repos/silo/src/infra/git.rs` | Git operations trait + implementation |
| `/Users/ricardo/code_repos/silo/src/infra/process.rs` | Process operations trait + implementation |
| `/Users/ricardo/code_repos/silo/src/infra/terminal/mod.rs` | Terminal trait, detection, creation |
| `/Users/ricardo/code_repos/silo/src/infra/terminal/iterm2.rs` | iTerm2-specific terminal implementation |
| `/Users/ricardo/code_repos/silo/src/infra/agent.rs` | Agent enum (Claude, Opencode) |
| `/Users/ricardo/code_repos/silo/src/services/agent_list.rs` | List running agents in worktrees |
| `/Users/ricardo/code_repos/silo/src/services/agent_launcher.rs` | Launch agent in worktree |
| `/Users/ricardo/code_repos/silo/src/services/git_worktree_workspace.rs` | Create git worktree workspace |
| `/Users/ricardo/code_repos/silo/src/services/worktree_cleanup.rs` | Remove unused worktrees |
| `/Users/ricardo/code_repos/silo/src/services/silo_config.rs` | Configuration and path resolution |

### Configuration Files

| **File** | **Purpose** |
|----------|-------------|
| `/Users/ricardo/code_repos/silo/Cargo.toml` | Rust package manifest |
| `/Users/ricardo/code_repos/silo/.pre-commit-config.yaml` | Pre-commit hooks (fmt, clippy, test) |
| `/Users/ricardo/code_repos/silo/README.md` | User-facing documentation |

### Module Re-export Files

| **File** | **Purpose** |
|----------|-------------|
| `/Users/ricardo/code_repos/silo/src/infra/mod.rs` | Infrastructure layer exports |
| `/Users/ricardo/code_repos/silo/src/services/mod.rs` | Service layer exports |

---

## Quick Start for AI Agents

**To understand the project** (2-minute scan):
1. Read **Section 2** (Project Mental Model) for purpose
2. Read **Section 3** (Architecture Layers) for structure
3. Scan **Section 4** (Critical Coding Patterns) for examples

**To add a feature**:
1. Use **Section 5** (File Organization Map) to identify layer
2. Follow template in **Section 10** (Extending the Codebase)
3. Check **Section 8** (Common Pitfalls) to avoid mistakes
4. Reference **Section 9** (Code Navigation Guide) for examples

**To fix a bug**:
1. Use **Section 12** (Key Files Reference) to find relevant file
2. Read the file using absolute paths
3. Follow **Section 6** (Development Workflow) for testing

**When stuck**:
- Check **Section 8** (Common Pitfalls & Solutions)
- Reference similar code in **Section 9** (Code Navigation Guide)
- Verify dependencies in **Section 11** (Dependencies & Crates)
