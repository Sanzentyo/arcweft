# Deletion-driven compile-clean implementation sequence

Every public cut must compile and execute using only its final owners. The
sequence is normative. Production work starts from `17b384a36e1412cc7e7d9f13073d8dd33dcb5cbc` or a later main
after reconciling any new change.

## Cut 1 — Generic Match only

### Add/change

- exhaustive checked expression semantic encoder in `arcweft-lang-sema`;
- exhaustive checked pattern semantic encoder;
- stable pattern binding coordinates;
- private bounded `MatchCoverageAnalyzer`;
- `CheckedMatchCoverage`, `CheckedMatch`, `CheckedMatchRef`;
- `CheckedMatchSemanticDigest`;
- exact Boolean-literal guard classification;
- final semantic analysis/catalog generation for generic Match.

### Explicitly do not touch

- ownership/persistence classification;
- View catalogs/products/runtime;
- task/Need identity or carriers;
- AWBC task plans;
- bundle/save/replay;
- host adapters.

### Compile-clean condition

Every HIR Match has exactly one generic checked fact or a deterministic checked
error. Existing consumers that do not yet consume `CheckedMatch` continue
through final sema only; no empty View catalog or fake product is introduced.

### Gates

```text
cargo check -p arcweft-lang-sema
cargo test -p arcweft-lang-sema checked_match
cargo test -p arcweft-lang-sema match_coverage
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

## Cut 2 — Ownership evidence chain

### Change atomically

- add `RuntimeOpaqueValueClass` and `RuntimeOpaquePersistence` to
  `AcceptedNominalInventoryInput`;
- update `AcceptedNominalInventoryInput::new`;
- update registrar validation;
- extend original `AcceptedNominalSemantics::Opaque`;
- update `AcceptedNominalRecord::try_new_opaque`;
- update accepted catalog digest;
- update standard/domain/environment/adapter/test constructors and fixtures;
- project both fields through runtime normalized types;
- add total inherent ownership classifier;
- add exact value-level certificates and ownership evidence digest;
- add producer-argument admission certificate.

### Explicitly do not add

- defaults;
- producer/name inference;
- ResourceTypeRegistry context;
- extension traits or copied enum matches;
- View admission/product rows.

### Compile-clean condition

There is no constructor path that can publish an opaque nominal without both
fields. Every current `TypeKind` is covered exhaustively. Generic Match remains
independent.

### Gates

```text
cargo check -p arcweft-lang-sema -p arcweft-runtime-plan
cargo test -p arcweft-lang-sema accepted_nominal
cargo test -p arcweft-lang-sema checked_ownership
cargo test --workspace opaque
cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets -- -D warnings
```

## Cut 3 — View admission and product/runtime join

### Add/change together

- `CheckedViewMatchAdmission` and digest;
- stable checked child-role path and `ViewMatchSiteId`;
- exact `CheckedViewMatchCoordinate`;
- checked View Match catalog;
- compiler projection and private core/bundle join projections;
- strict bundle row/codec/validation/merge/source-map/digest behavior;
- runtime-driver View Match evaluator/subscription projection;
- save/replay fields for View admission coordinates;
- replacement mapping/equality validation;
- static certification contaminant wiring;
- deletion of any copied View coverage/ownership facts.

### Preserve

- current `ViewProgramId`;
- current `AcceptedViewProgramRevision([u8;32])`;
- `arcweft-view` independence from core;
- accepted Variant/Tuple selector and explicit guard Branch lowering.

### Compile-clean condition

View product construction either receives one complete admission or fails
closed. Revision is absent from task-plan/producer identity. No
`ViewProgramSemanticDigest` exists.

### Gates

```text
cargo check -p arcweft-view -p arcweft-compiler -p arcweft-bundle -p arcweft-runtime-driver
cargo test -p arcweft-compiler checked_view_match
cargo test -p arcweft-bundle view_match
cargo test -p arcweft-runtime-driver view_match
cargo clippy -p arcweft-view -p arcweft-compiler -p arcweft-bundle -p arcweft-runtime-driver --all-targets -- -D warnings
```

## Cut 4 — Private identity preparation

### Add privately

- core `GenerationId`;
- fixed `NeedProducerInstanceKey`, `NeedId`, `TaskKey`, `TaskId`,
  `TaskLaunchOrdinal`;
- producer contract/plan/runtime type digest owners;
- exact transcript inherent methods;
- zero-hash typed errors;
- sink-parametric canonical RuntimeValue visitor;
- private `RuntimeValue::NeedHandle` implementation and focused canonical tests;
- private final task/correlation schemas behind the protected cut boundary.

### Migrate privately

- runtime-driver internals may compile against core `GenerationId`, but no
  public task/save/bundle schema is switched independently;
- plan owners gain recomputable semantic digest methods; no stored self-digest.

### Explicitly do not publish

- old/new public task schema together;
- compatibility conversions;
- String reader;
- typed handle on only one consumer;
- delayed save/replay changes.

### Gates

```text
cargo check -p arcweft-core -p arcweft-runtime-plan -p arcweft-runtime-driver
cargo test -p arcweft-core runtime_value_digest
cargo test -p arcweft-core task_identity
cargo test -p arcweft-core awbc_task_plan_digest
cargo clippy -p arcweft-core -p arcweft-runtime-plan -p arcweft-runtime-driver --all-targets -- -D warnings
```

## Cut 5 — Atomic public task/Need carrier and deletion cut

This cut is intentionally indivisible.

### Switch in the same protected commit

- final `TaskSpec`, `TaskHandle`, `TaskCorrelation`, `TaskEvent`,
  `RuntimeNeedState`, `RuntimeNeedOutcome`;
- `TaskHost::ensure_task` transaction;
- scheduler and engine;
- runtime-plan task interning;
- AWBC verifier/VM/product-step and task plan;
- `MakeNeedHandle`;
- direct Await;
- AwaitMany base/child/in-flight;
- timeout;
- View, line, structured, and host producers;
- native/Web/headless/Agent adapters;
- journal, save, snapshot, restore, replay, replacement;
- bundle and private codecs;
- generated artifacts and every fixture/test.

### Delete in the same commit

- String `NeedId`, `TaskKey`, `TaskId`;
- caller-supplied IDs/ordinal;
- `AwbcTaskPlan.need_id`;
- NeedHandle-as-String type/value admission;
- `await_target` String conversion;
- direct-Await surrogate;
- indexed String/suffix child identities;
- old snapshot fields/readers;
- task-plan self digest field;
- every compatibility/fallback/dual schema;
- stale copied View coverage/admission rows;
- the retained parent old View Await authority where still present.

### Protected-branch admission

No merge or public branch state may expose a midpoint of this cut. Local
implementation commits may be squashed or kept private, but every published
commit must compile with one final schema.

### Gates

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

Plus the focused/tamper/property/differential/restore/replacement/structural
and Tier-2 gates in `TEST_MATRIX.md`.

## External prerequisites/consumers

The maintained semantic AWBC opcode/function-kind/function-flag allocation,
canonical varint, final encoder buffer, and direct borrowed reader are external
prerequisites. This sequence does not redesign or reorder them.

The later CopyValue/Need/timeout/line/Stream feature cuts consume this
nonnumeric contract but do not permit reopening its identity or View admission.

## Forbidden intermediate states

- empty/dummy Match or View catalog;
- `coverage: bool` supplied by caller;
- generic Match gated by ownership;
- opaque default evidence;
- ResourceTypeRegistry without typed resource key;
- both driver-local and core GenerationId;
- both String and fixed Need/task identities;
- typed TaskSpec with old save/replay;
- typed RuntimeNeedHandle with String Await;
- new TaskEvent without journal/host correlation;
- plan digest stored in its own input;
- revision in producer identity;
- public compatibility reader or alias.

Any such state is contract-incomplete even if a narrow crate compiles.
