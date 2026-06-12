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

Arcweft source files use the extension `.arcw`.

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
- Push autonomously to `main` at reasonable, reviewable cut points after validation instead of waiting for an explicit push request every time. A cut point should be a coherent implementation slice, not every small edit and not a large bag of unrelated work.
- Do not create new branches or Jujutsu bookmarks unless the user explicitly asks for one. Keep routine work on `main`, avoid pushing speculative WIP refs, and remove or ask to remove merged/obsolete remote branches when cleanup is available and safe.
- Keep crate boundaries aligned with architecture layers. Lower-level crates must not depend on higher-level crates.
- Keep `arcweft-core` Sans I/O.
- `arcweft-core` is runtime/data core only. Do not make it depend on dialogue, presentation, syntax, verifier, CLI, LSP, GPU, filesystem, network, Servo, Wasmtime, CPAL, camera, USB, MCP, or OS adapters.
- Keep `arcweft-lang-syntax` parser/syntax-only. It may own CST, parser, surface AST, expression/type/pattern parsing, text tokenization, and syntax lints; HIR lowering, semantic checks, runtime-plan lowering, verifier logic, and tooling belong in separate crates.
- Keep HIR, semantic analysis, runtime-plan lowering, verification, solver adapters, CLI, and LSP as separate layers. Preferred direction is `syntax -> hir -> sema -> runtime-plan/verify -> tooling`.
- Keep data-format crates Sans I/O. Manifests, schemas, bytecode, bundles, save snapshots, and debug traces should expose typed data plus deterministic bytes/string codecs; path reads/writes, network, clocks, embedding, signing, and platform storage belong in CLI/build/player adapters.
- Put backend-specific dependencies behind feature flags and adapter crates.
- Use a facade crate for broad application-facing preludes. Do not place broad convenience preludes in low-level crates.
- Prefer responsibility modules as public boundaries (`pub mod`) when a crate has multiple stable subsystems. Avoid flattening every type through root-level `pub use`; root re-exports should be deliberate facade API, not compatibility shims.
- Do not leave workspace-external directories that look like active `crates/`, `tests/`, or fixtures. Archive true historical material under docs only when explicitly requested; otherwise remove obsolete migration scratch directories.
- Prefer typed APIs over stringly typed APIs.
- Prefer deterministic runtime behavior.
- Do not use `unsafe` unless isolated in a clearly named crate/module with an explanation.
- Do not implement speculative full features before creating minimal stable interfaces.
- When parser, compiler, or language-surface work requires broad reshaping, move directly toward the final model instead of preserving temporary compatibility layers.
- Do not preserve backward compatibility during internal parser/compiler/language-surface refactors. Replace the old model directly and let breakage expose every call site that must be updated.
- Do not use `deprecated` APIs, compatibility aliases, compatibility modules, wrapper APIs, migration shims, or compatibility shims inside unfinished compiler/parser code.
- Do not add parser/tooling branches that silently accept removed syntax. Removed syntax should fail through structured parser recovery/diagnostics unless the task explicitly targets an external one-shot migration tool.
- Prefer root-cause edits over transitional layers: remove obsolete variants/functions/types, run `cargo check` and `cargo clippy`, and fix all resulting call sites.
- During internal refactors, existing API compatibility may be dropped when it
  conflicts with the target architecture. Prefer explicit `pub mod` namespaces
  over broad pass-through `pub use` exports in non-facade crates/modules.
- Parser implementation should follow the grammar docs as the source of truth and should prefer explicit AST/CST nodes, source spans, and structured errors over stringly typed parsing.
- Parser tests should cover complete documented syntax families, including success cases, malformed input, recovery spans, and ambiguity rules.
- Public parser and AST types should have concise documentation comments suitable for generated Rust documentation.
- Comments in parser code should explain grammar decisions, ambiguity handling, and recovery strategy; avoid restating obvious control flow.

## Rust conventions

- Use `cargo fmt`.
- Use `cargo clippy --workspace --all-targets --all-features` when feasible.
- Follow `docs/implementation/test-execution-policy.md` for test scope. Prefer
  focused changed-crate tests during tight loops, run workspace check/clippy at
  reviewable cut points, and reserve the ignored Tier 2 MCP stdio / exact visual
  golden suite for changes that touch that risk area or milestone validation.
- Do not run full workspace tests after every small edit. Use `just test-fast`
  for the short core/render-text/text-layout/native-player smoke route,
  `just test-rich-text` or `just test-cli-native` for native rich-text/Agent
  observe work, `just test-workspace` for the full ignored-Tier-2 workspace
  pass at main push cut points, `just test-cli-check` for CLI-heavy cut points,
  and `just test-tier2` only for explicit slow validation.
- Keep public API intentional.
- Prefer `pub mod` boundaries for subsystem APIs and keep item visibility narrow inside those modules. Use root `pub use` only for small, deliberate facade surfaces.
- Split large `lib.rs` / `main.rs` files by responsibility before they become architectural boundaries in practice. Prefer `module.rs` plus subdirectories over `mod.rs`.
- Add tests for each new crate's core behavior.
- Add snapshot/golden tests only when deterministic.
- Use `thiserror` for Rust error types across the workspace unless there is a clear reason to hand-write `Display` / `std::error::Error`; preserve structured fields such as `kind`, `range`, `anchor`, and `message`.

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
5. Minimal parser or parser stubs for `.arcw`.
6. Tests for ID generation, dialogue line construction, and `Need` state transitions.

Do not attempt to implement wgpu rendering, Servo, audio, camera, USB, Cranelift JIT, or MCP until the core model compiles and tests pass.

## Documentation update rule

When implementation decisions change the design, update the relevant markdown files under `docs/`.

Keep implementation-state documentation separate from design documentation:

- `docs/implementation/` records current Rust workspace status, crate completion state, verification results, and known TODOs.
- `docs/00-overview/` through `docs/05-build-and-security/` remain design/specification documents.
- Do not mix transient implementation progress notes into stable design chapters unless the design itself changes.

Use code fences consistently:

- Arcweft DSL: `arcw`
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

