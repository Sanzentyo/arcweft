# Implementation order and validation gates

The implementation is one architecture change delivered in eight coherent,
compiling cuts. Each cut MUST be committed and pushed to `main` after its gate.
No cut may leave both old and new successful readers/resolvers.

## Global constraints

- Work on current `main`; no compatibility branch is required.
- Read current `AGENTS.md` and the complete Rust skill before implementation.
- Use `jj status`/`jj diff` where available.
- Keep `arcweft-core` and all codec/model crates Sans I/O.
- Do not add source gates.
- Do not add CSS or Takumi paths.
- Do not retain old Speaker/preset API aliases.
- Do not redesign ordinary functions/currying or typed Stream.
- Run the structure audit at every reviewable push cut that changes a public
  contract, dependency, codec, or large owner.

## Cut 1 — core nominal value and dialogue domain model

### Goal

Create the final runtime/domain types without changing source syntax.

### Changes

`arcweft-core`:

- add `RuntimeNominalRecordValue`;
- add `RuntimeValue::NominalRecord`;
- add inherent accessors/validation helpers on the owning types;
- update value labels, exact canonical encoder/digest, nested traversal,
  sequence/record utilities, root payloads, and exhaustive matches;
- preserve anonymous `Record` behavior.

`arcweft-dialogue`:

- add dependencies on character/core/view/serde as fixed by the contract;
- replace its public model with `CharacterDialogue`,
  `CharacterDialogueConfig`, patch/config value types, contract identity,
  locale/custom IDs, and content application;
- move inline-failure policy types directly from render-text;
- delete `SpeakerRef`, `SpeakerPreset`, `SayOptions`, `.say()` builder APIs, and
  related tests in the same cut;
- update direct Rust consumers.

### No compatibility interval

Do not re-export deleted names. Compilation failures identify every caller.

### Focused tests

- CharacterId direct ownership;
- immutable patch and clear behavior for domain fields;
- nominal vs anonymous record distinction;
- canonical value encoding/equality/hash;
- wrong type/layout/field count;
- Rust compile-fail/API tests for deleted dialogue crate APIs.

### Gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-core -p arcweft-dialogue -p arcweft-render-text --all-targets --all-features
cargo test -p arcweft-core -p arcweft-dialogue -p arcweft-render-text --all-features
cargo clippy -p arcweft-core -p arcweft-dialogue -p arcweft-render-text --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-4-cut1
```

## Cut 2 — syntax, HIR, and source-site line identity

### Goal

Atomically replace string speaker/content HIR with the final structured content
application and source-site IDs.

### Changes

`arcweft-lang-syntax`:

- add `DialogueContentApplicationExpr` and exact surface/ranges;
- route colon and bracket dialogue content to the same node;
- retain generic postfix bracket ambiguity substrate;
- delete `SpeakerLine`, `SpeakerLineSurface`, string `ContentCall` shapes;
- keep ordinary `.say` selected-call parsing only.

`arcweft-lang-hir`:

- add `HirDialogueContentApplication`;
- retain the full target expression and source map;
- implement source-owner-based line ID builder for flow/callable owners;
- implement project-wide collision inventory;
- derive text keys;
- delete `HirDialogue.callee`, `DialogueSpeakerSlug`, `.say` stripping, and
  character-name-derived IDs.

### Focused tests

- colon/bracket AST equivalence with distinct surface ranges;
- missing delimiter recovery;
- index/collection/record/call ambiguities;
- flow/function generated IDs;
- relative/absolute IDs and collisions;
- dynamic target generated ID independence from CharacterId;
- no `.say`-specific parser diagnostic.

### Gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-lang-syntax -p arcweft-lang-hir --all-targets --all-features
cargo test -p arcweft-lang-syntax -p arcweft-lang-hir --all-features
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-4-cut2
```

## Cut 3 — sema and shared callable resolver atomic switch

### Goal

Publish and consume the final CharacterDialogue types/callable facts.

### Changes

`arcweft-lang-sema`:

- add `CharacterDialogueType::{Exact,Any}` and
  `TypeKind::CharacterDialogue`;
- add non-escaping `DialogueLine<R>` compiler type;
- add typed custom-field registry;
- add checked factory/reconfiguration/application facts;
- add dependent look parameter validation;
- implement joins, compatibility, expected types, generics, aliases, branches,
  returns, captures, and indirect target checks;
- publish CharacterFactory, CharacterReconfigure, and ContentApplication
  schemas through the shared callable catalog;
- split reusable and immediate-application patch contexts;
- migrate checker, signature results, hover facts, canonicalization inventory;
- delete `Speaker`, `SpeakerPreset`, `SpeakerLineType`,
  `CheckedSpeakerLine`, old dialogue callable variants, and all special checker
  branches in the same cut.

### Focused tests

- exact/any type joins;
- dynamic branches/returns/parameters/closures/collections;
- exact and dynamic look checks;
- application-only id/text-key context;
- custom registry collisions/types/View compatibility;
- shared resolver and signature schemas;
- source-ranged mismatch diagnostics;
- ordinary missing-method rejection for `.say`.

### Gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-lang-sema -p arcweft-verify-lsp -p arcweft-lsp --all-targets --all-features
cargo test -p arcweft-lang-sema -p arcweft-verify-lsp -p arcweft-lsp --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-verify-lsp -p arcweft-lsp --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-4-cut3
```

## Cut 4 — runtime-plan, AWBC ABI/codec, VM, and verifier

### Goal

Make CharacterDialogue fully executable and persistent at the core VM layer.

### Changes

`arcweft-runtime-plan`:

- add Make/Patch runtime expressions and typed patch plans;
- lower checked sema facts only;
- add runtime content application with CharacterDialogue expression;
- remove `DialogueSpeakerPreset`, `speaker_preset_from_let`,
  `speaker_preset_chain`, callee/default string maps, and suffix logic;
- split static content specs from runtime config.

`arcweft-core::awbc`:

- switch to ABI 2 / codec 8;
- add CharacterDialogue default/custom tables;
- add Make/Patch opcodes and patch-field table;
- add Dialogue register to terminator/suspension;
- decode nominal records without losing type identity;
- add exact verifier and VM transaction;
- update AOT/parity/runtime type handling;
- reject ABI 1/codec 7; no old reader.

`arcweft-verify`:

- validate typed runtime-plan/AWBC obligations and line result behavior.

### Focused tests

- runtime construction/patch/content after dynamic control flow;
- immutability and rollback;
- exact one-over budgets/limits;
- nominal record codec/type/layout/tamper;
- opcode/terminator verifier;
- VM suspension/resume;
- function captures/collections containing CharacterDialogue;
- no RuntimeCallTarget string fallback.

### Gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-verify --all-targets --all-features
cargo test -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-verify --all-features
cargo clippy -p arcweft-core -p arcweft-runtime-plan -p arcweft-compiler -p arcweft-verify --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-4-cut4
```

## Cut 5 — bundle, runtime-driver, display, save/replay, and hot reload

### Goal

Connect the typed value to every actual product and session boundary.

### Changes

`arcweft-render-text`:

- replace `LineDisplaySpec` mixed identity/config with static
  `DialogueContentSpec`;
- update cascade layer to `CharacterDialogueConfig`;
- consume typed effective config;
- remove `callee` and `speaker_label`.

`arcweft-bundle` / compiler:

- add/cross-validate CharacterDialogue AWBC/default/custom/content resources;
- require typed View IDs and source revisions;
- reject old bundle fixtures rather than dual reading.

`arcweft-runtime-driver`:

- decode CharacterDialogue at Dialogue suspension;
- resolve required character display name;
- create runtime `LineDisplayFrame` with typed Character payload;
- update state transition inputs without grammar logic;
- switch save schema to 2;
- retain root replay schema 1 and extend only generic `RuntimePayload` nominal-value validation;
- update debug presentation trace and Agent observation to typed Character fields;
- implement exact hot-reload/rebind digest rules;
- reject old save and old dialogue-callee/debug-observation shapes.

### Focused tests

- product round trip and cross-section tamper;
- native runtime content display from dynamic CharacterDialogue;
- save/restore with values in registers/captures/containers;
- replay exactness;
- compatible/incompatible hot reload;
- stale manifest/default/View/content/custom schema;
- Agent observation field shape;
- runtime display contains CharacterId/display name and no callee identity.

### Gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-bundle -p arcweft-render-text -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-player-scene --all-targets --all-features
cargo test -p arcweft-bundle -p arcweft-render-text -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-player-scene --all-features
cargo clippy -p arcweft-bundle -p arcweft-render-text -p arcweft-runtime-driver -p arcweft-runtime-host -p arcweft-player-scene --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-4-cut5
```

## Cut 6 — formatter, canonicalization, LSP, CLI, and Agent adapters

### Goal

Complete `.say`-free tooling and semantic query consumption.

### Changes

- formatter: syntax-only CharacterDialogue/colon/bracket formatting;
- canonicalizer: CheckedCharacterDialogueApplication and bracket expansion;
- completion/hover/signature help/definition/rename/tokens/code actions;
- CLI project-aware canonicalization;
- Agent protocol/observation adapters;
- delete speaker canonicalization inventory/types and `.say` emission;
- no migration action unless separately requested as an external one-shot tool.

### Focused tests

- exact colon-to-bracket edits, Unicode/CRLF/comments;
- stale/unavailable sema no-edit;
- config vs content signature help;
- typed definitions/renames/custom fields;
- no `.say` completion/action/output;
- Agent JSON schema.

### Gate

```bash
cargo fmt --all -- --check
cargo check -p arcweft-tooling -p arcweft-lsp -p arcweft-cli -p arcweft-agent-protocol -p arcweft-agent-mcp --all-targets --all-features
cargo test -p arcweft-tooling -p arcweft-lsp -p arcweft-cli -p arcweft-agent-protocol -p arcweft-agent-mcp --all-features
cargo clippy -p arcweft-tooling -p arcweft-lsp -p arcweft-cli -p arcweft-agent-protocol -p arcweft-agent-mcp --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-4-cut6
```

## Cut 7 — canonical samples, docs, compile-fail deletion proof

### Goal

Remove every old source/API use and make the repository teach only the final
surface.

### Changes

- replace canonical `.say` samples/docs with direct CharacterDialogue calls;
- update narrator examples;
- update API docs and crate maps;
- delete old fixtures/goldens/digest tags;
- add compile-fail tests for deleted Rust/source APIs;
- update generated artifacts by regeneration, not source searching;
- record implementation note and exact validation results.

### Gate

```bash
cargo fmt --all -- --check
just test-fast
just test-rich-text
just test-cli-check
just test-workspace
just test-doc
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-4-cut7
git diff --check
```

## Cut 8 — runtime presentation and Tier 2 milestone validation

This change touches runtime presentation, save/replay, Agent observation, and
native/Web output, so the final milestone MUST run:

```bash
just test-tier2
```

In addition:

- native/Web/headless CharacterDialogue parity;
- Agent/MCP observe/action on active dialogue;
- exact save/restore/replay/hot reload suite;
- exact visual goldens for the affected dialogue sample;
- browser/wasm checks used by the current repository policy;
- final structure audit;
- final dependency graph review proving no cycle and no CSS/Takumi route.

Any environment-specific blocked test is recorded with exact command/output in
the implementation note. A blocked test does not silently become a pass.

## Final deletion gate

Before declaring implementation complete, direct API/type behavior tests must
prove the new model and compile-fail tests must prove deleted APIs. Do not add a
repository source scan. The compiler itself must have no executable old node
because all call sites compile only after the enum/type deletions.

## Commit/push order

Each cut is a coherent push point:

```text
Cut 1 -> commit/push
Cut 2 -> commit/push
Cut 3 -> commit/push
Cut 4 -> commit/push
Cut 5 -> commit/push
Cut 6 -> commit/push
Cut 7 -> commit/push
Cut 8 validation/audit note -> commit/push
```

Do not accumulate unrelated completed work in one checkout.
