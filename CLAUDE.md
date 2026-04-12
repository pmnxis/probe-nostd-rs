## Workflow Orchestration

### 1. Plan Mode by Default
- Enter plan mode for non-trivial tasks (3+ steps or architectural decisions)
- If things go sideways, STOP immediately and re-plan — don't keep pushing
- Use plan mode not just for builds but also for verification steps
- Write detailed specs upfront to reduce ambiguity

### 2. Sub-Agent Strategy
- Aggressively use sub-agents to keep the main context window clean
- Delegate research, exploration, and parallel analysis to sub-agents
- Throw more compute at complex problems via sub-agents
- Assign one task per sub-agent for focused execution

### 3. Self-Improvement Loop
- Update patterns in `.investigation/lessons.md` every time user corrects you
- Write rules that prevent the same mistake from recurring
- Ruthlessly iterate on these lessons until error rate drops
- Review relevant project lessons at session start

### 4. Verify Before Completion
- Never mark work as done without proving it works
- Compare behavior differences between main and changes when needed
- Ask yourself: "Would a senior engineer approve this?"
- Run `cargo fmt`, `clippy`, `check` before Rust builds
- Use debug builds unless the final build is for release

### 5. Pursue Elegance (Balanced)
- For non-trivial changes, ask "Is there a more elegant way?"
- If a fix feels like a hack: "Implement an elegant solution considering everything we know"
- Skip this for simple, clear fixes — no over-engineering
- Self-review your own work before presenting

### 6. Autonomous Bug Fixing
- When you get a bug report, just fix it. Don't ask for step-by-step instructions
- Proactively find and resolve logs, errors, and failing tests
- No context-switching needed from the user

## Task Management

1. **Plan first:** Write checkable items in `.investigation/todo.md`
2. **Validate plan:** Confirm before starting implementation
3. **Track progress:** Mark items complete as you go
4. **Explain changes:** Provide high-level summary at each step
5. **Document results:** Add a review section in `.investigation/todo.md`
6. **Capture lessons:** Update `.investigation/lessons.md` after corrections

## Core Principles

- **Simplicity first:** Keep every change as simple as possible. Minimal code impact.
- **No laziness:** Find the root cause. No temporary fixes. Senior developer standards.
- **Minimal blast radius:** Change only what's needed. No introducing new bugs.

## Code Patterns

- When `too_many_arguments` clippy warning occurs, prefer refactoring into a struct-based
  parameter pattern (builder or config struct) rather than `#[allow]`. Example:
  ```rust
  // Instead of: fn call(pc, r0, r1, r2, r3, init, max_polls) -> ...
  // Use a struct:
  struct FunctionCall { pc: u64, args: [u32; 4], init: bool, max_polls: u32 }
  fn call(&mut self, params: &FunctionCall) -> ...
  ```
  Reference: Rust's `Iteration`-style structs where configuration is bundled.
  Apply this pattern when adding new functions. Existing `#[allow]` instances are
  candidates for future refactoring.

## Project-Specific

- This is a `no_std` extraction of [probe-rs](https://github.com/probe-rs/probe-rs) for embedded targets (ESP32, STM32H7, RP2040, etc.)
- Reference project: [rusty-probe-firmware](https://github.com/probe-rs/rusty-probe) (RP2040 CMSIS-DAP probe, RTIC-based)
- Reference project: [billmock-app-rs](https://github.com/pmnxis/billmock-app-rs) and [billmock-plug-ed785] -- user's embedded optimization philosophy
- Architecture documentation lives in `.investigation/` -- read it before making structural changes
- Must support both Embassy and RTIC frameworks via `embedded-hal` traits
- All crates must be `#![no_std]` with optional `alloc` feature
- Use `defmt` for logging (behind feature flag), never `println!` or `log`
- Prefer `&'static str` and slices over `String`/`Vec` where possible

## Toolchain

- nostd crates (probe-rs-wire, probe-rs-adi, probe-rs-target-nostd): `nightly-2026-04-11`, `rust-version = "1.95"`
- Workspace default (probe-rs etc): `rust-toolchain.toml` = `1.94.0`
- Build nostd crates with `cargo +nightly` explicitly
- Features from 1.95/nightly to actively use: `cold_path`, `cfg_select!`, `if let` guards, `let_chains`

## Incremental Change Policy

- When modifying existing nostd crates (probe-rs-wire, probe-rs-adi, probe-rs-target-nostd):
  - Check `.investigation/RUST_FEATURES.md` for applicable Rust features
  - Maintain backward compatibility of public API unless explicitly asked to break
  - When adding new modules, update the crate's lib.rs re-exports
  - Run `cargo +nightly check/clippy/test` before marking done
- When upstream probe-rs updates (new targets, API changes):
  - Re-run build.rs codegen for probe-rs-target-nostd
  - Check if new probe-rs types need to be mirrored in nostd types
- When new Rust stable releases land:
  - Review `.investigation/RUST_FEATURES.md` for newly stable features
  - Update `rust-version` in nostd Cargo.toml files if beneficial
  - Update nightly pin date if needed for new nightly-only features
- Requirements may change mid-session -- track evolving decisions in the plan file and `.investigation/` docs, not just in conversation context
