# AGENTS.md

For setup, testing, and project structure details, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Quick Reference

| Attribute | Value |
|-----------|-------|
| Language | Rust 2024 edition (minimum 1.93.0) |
| Architecture | Layered: Infrastructure -> Services -> CLI |
| Core Pattern | Trait-based abstractions with generics |
| Error Handling | `thiserror` enums per domain with `From` implementations |
| Testing | `mockall` for trait mocking, unit tests inline (`#[cfg(test)]`) |
| Quality Gates | Pre-commit hooks: fmt, clippy, check, test |

## Architecture

```
CLI (src/cli/)  -->  Services (src/services/)  -->  Infrastructure (src/infra/)
```

- **Only the CLI layer prints.** Services and infrastructure never produce output directly.
- Services use **generic trait bounds**, not concrete types.
- `src/main.rs` is the composition root that wires concrete types into services.

## Critical Patterns

### Trait-Based Services

```rust
// DO: generic trait bounds
pub struct MyService<G: GitOperations> { git: G }

// DON'T: concrete types (not mockable)
pub struct MyService { git: Git }
```

### Error Propagation

Define domain errors with `From` impls to enable the `?` operator across error boundaries.

### Testing with Mocks

Place tests at the bottom of each file in `#[cfg(test)]` modules. Use `mockall` to mock
infrastructure traits and test service logic in isolation.

## Where to Add Code

```
New system interaction (git, process, terminal)?
  -> src/infra/ (define trait + concrete impl)

New business logic?
  -> src/services/ (generic service with trait bounds)

New CLI command?
  -> src/cli/ (add handler)
  -> src/main.rs (add enum variant + routing)
```

After adding a new file, export it in the corresponding `mod.rs`.

## Common Pitfalls

1. **Using concrete types in services** - Always use generic trait bounds.
2. **Printing outside CLI layer** - Only `src/cli/` files should use `println!`/`eprintln!`.
3. **Forgetting `mod.rs` exports** - New files must be declared and re-exported.
4. **Calling `Command::new` in services** - Use trait abstractions so logic is testable.

## Development Workflow

1. Read relevant code first
2. Follow existing patterns
3. Add tests with mocks
4. Run quality checks: `cargo fmt && cargo clippy && cargo test`
5. Commit (pre-commit hooks run automatically)

## Key Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, dependency injection |
| `src/cli/` | Command handlers (all user-facing output) |
| `src/services/agent_launcher.rs` | Workspace creation + agent spawning |
| `src/services/agent_list_service.rs` | Running agent discovery |
| `src/infra/git.rs` | Git operations trait + impl |
| `src/infra/system_process.rs` | Process operations trait + impl |
| `src/infra/terminal/` | Terminal abstraction (iTerm2) |
| `src/infra/agent/` | Agent definitions (Claude, Opencode, etc.) |
| `Cargo.toml` | Package manifest |
| `.pre-commit-config.yaml` | Pre-commit hook configuration |
