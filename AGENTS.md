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
- Do not carry unrelated completed work across independent user goals. After a
  package-driven goal, request-file update, AGENTS.md policy edit, or coherent
  refactor has passed its appropriate validation, describe it, commit it, and
  push it before starting the next independent package or design request unless
  the user explicitly asks to hold the changes locally.
- If validation is partially blocked but the user asks to continue with another
  independent task, either commit and push with the blocked validation clearly
  documented in `docs/implementation/` and the final response, or stop and ask
  for direction. Do not let multiple unrelated completed slices accumulate in
  one working copy.
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
- When an Arcweft-owned enum or boundary type is missing behavior needed by
  another crate, add the behavior as an inherent method on the original type
  whenever the dependency direction allows it. Do not scatter ad hoc `match`
  helpers, local extension traits, or stringly wrappers around the workspace to
  compensate for an incomplete enum API.
- Do not solve language, parser, type-checker, lowering, or runtime-plan
  behavior by hard-coding one-off names when the rule can be represented
  generically. For example, expected-type enum shorthand, builtin namespaces,
  and variant resolution should be implemented through shared typed registries
  or grammar/type rules, not by special-casing a single enum such as
  `DataFormat.Json` / `.Json`.
- Prefer deterministic runtime behavior.
- Do not use `unsafe` unless isolated in a clearly named crate/module with an explanation.
- Do not implement speculative full features before creating minimal stable interfaces.
- Treat unreleased, unused Arcweft APIs and serialized formats as freely
  replaceable implementation contracts. Move directly to the best final model;
  do not preserve an inferior shape merely because it existed in the repository.
- Compatibility work requires evidence of a released artifact, persisted user
  data, an external consumer, or an explicit compatibility requirement. Without
  that evidence, do not add deprecated fields, dual readers, aliases, migration
  shims, or version bumps that only memorialize a discarded provisional format.
  A corrected unpublished format should become the initial supported version.
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

## Helper and conversion discipline

Prefer owned, discoverable conversion APIs over free-standing
`{SourceType}_to_{TargetType}` helper functions.

When converting one Arcweft-owned boundary type into another and the dependency
direction allows it:

- prefer `impl From<Source> for Target` for infallible, context-free
  conversions;
- prefer `impl TryFrom<Source> for Target` or `impl TryFrom<&Source> for
  Target` for fallible, context-free conversions;
- prefer an inherent method on the owning boundary type when the conversion is
  part of that type's domain behavior;
- prefer a method on a dedicated lowering, inventory, verifier, or adapter
  context when conversion needs interning, diagnostics, source spans,
  allocation, policy, or shared state.

Avoid public or widely reused helpers named like `runtime_policy_to_awbc_policy`,
`foo_to_bar`, `convert_foo_bar`, or similar when the same behavior can live on
an owned type or a named context object. Such helpers make ownership boundaries
harder to audit and encourage scattered field-by-field projections.

Small private helpers are acceptable when they are truly local implementation
details, but name them by domain role rather than by both endpoint type names
where possible. If a helper grows, is reused across modules, or mirrors a
boundary contract, move it onto the responsible type or context object. Do not
add extension traits, compatibility shims, or ad hoc wrapper APIs merely to
avoid placing behavior on the owning type.

Do not create private helper functions merely to hide a small `map_err`,
`ok_or_else`, `match`, `if let`, or one-off error conversion. Prefer inline
error handling when the logic is used once, the mapping is one or two
expressions, and a helper name would mostly restate the operation, such as
`convert_error`, `map_parse_error`, `missing_foo_error`, or `foo_or_error`.

A private helper is appropriate when it names a real domain rule, invariant, or
policy; is reused in multiple call sites; keeps a public function below a
meaningful complexity threshold without hiding important error context;
centralizes structured diagnostic construction that must remain consistent; or
isolates boundary adaptation between crates or runtime tiers.

When error construction needs shared formatting, stable diagnostic codes,
source ranges, or repeated structured fields, prefer an inherent constructor on
the error or diagnostic type over scattered local private helper functions.
Avoid helper names whose only information is control mechanics, such as
`handle_error`, `map_error`, `to_error`, `ensure_valid`, or `check_foo`, unless
they encode a specific domain invariant in their name and body.

## Structural audit and decomposition gate

Repository structure is part of the implementation result. Do not treat
compilation and test success alone as sufficient evidence that a change has an
appropriate ownership boundary.

### Audit triggers

Run a structural audit when any of the following applies:

- a task explicitly requests an architecture, dependency, test-structure,
  duplication, naming, or maintainability review;
- a production Rust file exceeds 1,200 physical LOC or grows by more than
  300 physical LOC in one coherent change;
- a `lib.rs` or `main.rs` exceeds 1,000 physical LOC;
- an integration-test file exceeds 2,500 physical LOC;
- a production file above 1,200 LOC contains an embedded `#[cfg(test)]` module;
- a workspace dependency, public contract, root re-export, Cargo feature, or
  crate boundary is added or materially changed;
- one change touches orchestration together with transport, persistence,
  rendering, protocol conversion, pixel processing, or platform I/O;
- the same boundary type, identifier, payload shape, or conversion logic
  appears in multiple crates;
- a manual field-by-field projection, statistics delta, descriptor inventory,
  or schema mapping is added or extended.

At reviewable push cut points, run the audit even when only warnings are
expected. Do not wait until a file crosses an error threshold before examining
its responsibilities.

### Required measurement

Use exact data from the current checkout. Do not use diff additions as a
substitute for current file size.

For changed Rust files and the largest workspace Rust files, record:

- repository revision or Jujutsu change identifier;
- path and owning crate;
- exact byte size;
- physical LOC;
- whether the file is production, unit test, integration test, generated data,
  benchmark, or facade;
- embedded test LOC when `#[cfg(test)]` is present;
- major responsibilities present in the file;
- workspace dependency fan-in and fan-out when relevant.

Exclude `target/`, VCS internals, vendored upstream source, generated artifacts,
and historical documentation unless the task explicitly audits them. Generated
source must be marked as generated rather than silently mixed into production
hotspot rankings.

Use these values as review triggers:

- production Rust file:
  - warning above 1,200 physical LOC;
  - error above 2,500 physical LOC;
- `lib.rs` / `main.rs`:
  - warning above 1,000 physical LOC;
  - post-split facade target of at most 250 LOC;
- integration-test file:
  - warning above 2,500 physical LOC;
  - error above 8,000 physical LOC;
- ordinary responsibility module:
  - preferred target range of 300-800 LOC.

These are ownership-review thresholds, not a license to split cohesive code
arbitrarily. Generated lookup tables and genuinely cohesive algorithms may be
exempted, but the reason must be documented in a module-level comment and in
the relevant implementation audit. An error-level exception requires an
explicit repository-visible rationale.

### Automated audit command

The checked-in structural audit script is the canonical first pass:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

## Source-gate prohibition

Do not add source gates. A source gate is a test, script, CI check, or audit
rule that opens checked-in implementation or documentation files and passes or
fails by searching for source text, symbol spellings, snippets, module paths,
file locations, or the presence/absence of named implementation details.
Examples include `include_str!`/`read_to_string` followed by `contains`, token
deny lists over the repository, and assertions that a particular type or call
still appears in a particular `.rs` file.

- Do not repair a source gate after a refactor by updating its expected token,
  symbol, or file list. Replace it with direct evidence or delete it.
- Do not rename a source gate or move the same spelling/path checks into the
  structural audit. Structural audits measure ownership, size, dependencies,
  and typed boundaries; they do not freeze implementation text.
- Test behavior, serialization, and security invariants through public or
  crate-owned APIs, including positive, negative, round-trip, and tampered-input
  cases where relevant.
- Test language removal through parser/compiler rejection and structured
  diagnostics. Test API constraints with visibility, type checking, or
  compile-fail cases. Test architecture and crate layering from Cargo metadata
  or another structured dependency graph.
- Deterministic generated-artifact comparison is acceptable when the generated
  artifact itself is the contract. It must regenerate and compare the artifact,
  not search unrelated implementation source for expected words.
- Existing source gates should be removed after preserving any real invariant
  with direct tests. If a gate does not correspond to an observable invariant,
  remove it without replacement.
- This repository-wide rule supersedes older design requests and implementation
  notes that ask for source gates. Translate those historical acceptance items
  into direct behavior, codec, compile, lint, or dependency evidence instead of
  reintroducing their prescribed source scans.

This prohibition does not prevent normal code review or one-off source
inspection. It prevents source spelling and file placement from becoming
automated correctness evidence.

## Rust conventions

- Use `cargo fmt`.
- Use `cargo clippy --workspace --all-targets --all-features` when feasible.
- Keep Cargo feature sets stable to avoid unnecessary `target/` growth. Prefer
  the checked-in `just` entrypoints and repeat the same feature combination for
  a validation slice instead of alternating between default features,
  `--all-features`, and ad hoc `--features ...` forms. Use extra feature
  combinations only when they directly match the code path under test, and
  record that reason in the final validation summary.
- Remove wildcard imports when they are reported by the active clippy lint
  configuration, including `clippy::wildcard_imports`. Do not treat every
  private `use super::*` as a blanket structural violation unless clippy or the
  local task calls it out.
- Follow `docs/implementation/test-execution-policy.md` for test scope. Prefer
  focused changed-crate tests during tight loops, run workspace check/clippy at
  reviewable cut points, and reserve the ignored Tier 2 MCP stdio / exact visual
  golden suite for changes that touch that risk area or milestone validation.
- Do not run full workspace tests after every small edit. Use `just test-fast`
  for the short core/render-text/text-layout/native-player smoke route,
  `just test-rich-text` or `just test-cli-native` for native rich-text/Agent
  observe smoke work, `just test-workspace` for the normal workspace fast path
  at main push cut points, `just test-cli-check` or focused exact `check.rs`
  tests for routine CLI-heavy smoke, `just test-cli-check-full` only when a full
  `check.rs` pass is explicitly warranted,
  `just test-doc` for explicit doc-test validation, and `just test-tier2` only
  for explicit slow validation.
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

## Scope and completion boundaries

- When a task is driven by an attached package or design brief, derive concrete
  acceptance criteria from that source and compare them with current
  implementation evidence before marking the goal complete. Do not redefine
  success around the subset already implemented.
- Keep task-specific completion audits, non-goals, and follow-up boundaries in
  `docs/implementation/` rather than hard-coding them into this file. Use
  AGENTS.md only for durable repository-wide workflow and architecture rules.
- If a requirement is intentionally out of scope for the current cut, document
  that as an explicit non-goal in the relevant implementation note or keep the
  goal open. Avoid silently treating a missing feature as complete because
  nearby functionality exists.

## Zip design package workflow

When a user provides a zip design package and asks to set or pursue an
implementation goal, handle the package as the goal's source of truth before
writing production code.

Before setting the goal:

- Confirm the package path exists and inspect its README, request markdown,
  implementation notes, overlay/patch manifest, and design documents.
- Derive the goal from the package's explicit acceptance criteria, not from the
  subset that is easiest to implement.
- Name the goal so it distinguishes:
  - items that are sufficiently designed and implementation-ready;
  - items intentionally excluded because the package says they are not
    implemented;
  - items excluded because the package leaves design, direction, API shape,
    migration order, or verification expectations underspecified.

While implementing a package-driven goal:

- Implement the sufficiently designed items end to end.
- Do not count a package's broad sequence request as sufficient design by
  itself. If the package includes a topic in a sequence request but does not
  give enough concrete implementation direction, remove that topic from the
  active goal and create a follow-up design request.
- If the package marks an item as future work, non-goal, intentionally
  unimplemented, migration-gated, or specified only at a high level, exclude it
  from the current implementation completion criteria unless another package
  document provides concrete implementation-ready design for that item.
- Record excluded implementation items and verification gaps in
  `docs/implementation/`, not in stable design chapters.

When adding follow-up request markdown under `docs/reviews/requests/`:

- Use sequence-preserving branch numbers for items split out of an existing
  request, for example `2026-06-24-seq-01.1-...md`,
  `2026-06-24-seq-01.2-...md`.
- Make each request independently throwable to another designer or agent:
  include sequence position, why the split is needed, required decisions,
  implementation order, tests to specify, constraints, and expected output.
- Group topics into one request only when they must be designed together to
  avoid incompatible contracts. Otherwise prefer smaller sequential requests.
- State that the follow-up request must not redesign already implemented and
  verified substrate unless the design package or current implementation
  evidence shows a concrete flaw.
- Link the follow-up requests from the relevant implementation note so the
  active goal's non-goals are visible from repository documentation.

## Acceptance criteria for each task

Each task should end with:

- changed files summary
- tests run
- remaining TODOs
- design deviations, if any

Do not hide failures. If a command fails, report the failure and either fix it or explain why it is out of scope.

