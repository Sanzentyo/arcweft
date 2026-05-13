# AGENTS.md — Arcweft Engine

## Project

This repository implements **Arcweft Engine**, a layered, verified, agent-native narrative engine written in Rust.

The main design documentation is in:

- `docs/README.md`
- `docs/00-overview/`
- `docs/01-language/`
- `docs/02-runtime/`
- `docs/03-presentation/`
- `docs/04-tooling/`
- `docs/05-build-and-security/`
- `docs/schemas/`
- `docs/examples/`

Arcweft source files use the extension `.awft`.

## Required skill usage

Before writing Rust code, read and follow the user's Rust skill instructions.

Look for Rust-related skill files in the local skills directory, such as:

- `~/.agents/skills/**/SKILL.md`
- `.agents/skills/**/SKILL.md`
- `skills/**/SKILL.md`

Use the Rust skill whenever editing Rust code, Cargo workspaces, build scripts, tests, benches, or Rust documentation.

If multiple Rust skills exist, read all relevant `SKILL.md` files and summarize the applicable rules before implementation.

## Implementation style

- Prefer small, compiling increments.
- Use Jujutsu (`jj`) for repository state when available; prefer `jj status`, `jj diff`, and `jj describe` over equivalent Git commands for local workflow reporting.
- Keep `arcweft-core` Sans I/O.
- Do not add GPU, filesystem, network, Servo, Wasmtime, CPAL, camera, or OS dependencies to `arcweft-core`.
- Put backend-specific dependencies behind feature flags and adapter crates.
- Prefer typed APIs over stringly typed APIs.
- Prefer deterministic runtime behavior.
- Do not use `unsafe` unless isolated in a clearly named crate/module with an explanation.
- Do not implement speculative full features before creating minimal stable interfaces.
- When parser, compiler, or language-surface work requires broad reshaping, move directly toward the final model instead of preserving temporary compatibility layers.
- Do not use `deprecated` APIs, compatibility aliases, or migration shims inside unfinished compiler/parser code. Let the Rust compiler expose every call site that must be updated.
- Parser implementation should follow the grammar docs as the source of truth and should prefer explicit AST/CST nodes, source spans, and structured errors over stringly typed parsing.
- Parser tests should cover complete documented syntax families, including success cases, malformed input, recovery spans, and ambiguity rules.
- Public parser and AST types should have concise documentation comments suitable for generated Rust documentation.
- Comments in parser code should explain grammar decisions, ambiguity handling, and recovery strategy; avoid restating obvious control flow.

## Rust conventions

- Use `cargo fmt`.
- Use `cargo clippy --workspace --all-targets --all-features` when feasible.
- Use `cargo test --workspace`.
- Keep public API minimal.
- Prefer private modules and explicit `pub use` exports.
- Add tests for each new crate's core behavior.
- Add snapshot/golden tests only when deterministic.

## Initial implementation order

Start with the smallest compiling Rust workspace.

Recommended first milestones:

1. Cargo workspace and crate skeleton.
2. Core identity types:
   - `EntityId`
   - `PublicId`
   - `TextKey`
   - `SourceAnchor`
3. Core result types:
   - `Need<T, E>`
   - `Progress`
   - `FrameInput`
   - `FrameOutput`
4. Dialogue surface model:
   - `DialogueLine`
   - `SpeakerRef`
   - `TextBoxRef`
   - `DialogueContent`
   - `DialogueTag`
   - `LinePlan`
   - `CancelScope`
5. Minimal parser or parser stubs for `.awft`.
6. Tests for ID generation, dialogue line construction, and `Need` state transitions.

Do not attempt to implement wgpu rendering, Servo, audio, camera, USB, Cranelift JIT, or MCP until the core model compiles and tests pass.

## Documentation update rule

When implementation decisions change the design, update the relevant markdown files under `docs/`.

Keep implementation-state documentation separate from design documentation:

- `docs/implementation/` records current Rust workspace status, crate completion state, verification results, and known TODOs.
- `docs/00-overview/` through `docs/05-build-and-security/` remain design/specification documents.
- Do not mix transient implementation progress notes into stable design chapters unless the design itself changes.

Use code fences consistently:

- Arcweft DSL: `awft`
- Rust implementation: `rust`
- JSON: `json`
- TOML: `toml`
- Shell: `bash`
- Plain diagrams: `text`

## Acceptance criteria for each task

Each task should end with:

- changed files summary
- tests run
- remaining TODOs
- design deviations, if any

Do not hide failures. If a command fails, report the failure and either fix it or explain why it is out of scope.
