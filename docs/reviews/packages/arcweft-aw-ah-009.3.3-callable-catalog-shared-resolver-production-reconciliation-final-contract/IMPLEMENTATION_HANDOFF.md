# Implementation handoff

## 1. Preconditions and branch basis

Implement on the then-current accepted descendant of
`9fd6ee8fb2814ff04dc7a3e4ef413b86b7f4ac4d`. Before editing:

1. read the current root `AGENTS.md` in full and any more-specific instructions
   for changed paths;
2. read the complete Rust skill;
3. verify that the upstream AW-AH-009.3 contract identity remains
   `cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5`;
4. re-run the current production family audit against public behavior;
5. do not connect call ranges before AW-AH-009.3.1 and do not connect accepted
   HIR request leasing before AW-AH-009.3.2;
6. do not rebase this design by changing result behavior. Any new current-main
   family is added to the same candidate/schema/resolver model before migration.

Use small compiling cuts. Never leave two successful resolvers at the end of a
cut.

## 2. Cut 1 — typed substrate and invariant tests

### 2.1 Files

Create:

```text
crates/arcweft-lang-sema/src/callable.rs
crates/arcweft-lang-sema/src/callable/identity.rs
crates/arcweft-lang-sema/src/callable/schema.rs
crates/arcweft-lang-sema/src/callable/catalog.rs
crates/arcweft-lang-sema/src/callable/publication.rs
crates/arcweft-lang-sema/src/callable/resolver.rs
crates/arcweft-lang-sema/src/callable/arguments.rs
crates/arcweft-lang-sema/src/callable/facts.rs
crates/arcweft-lang-sema/src/callable/presentation.rs
crates/arcweft-lang-sema/src/callable/dialogue.rs
crates/arcweft-lang-sema/src/callable/limits.rs
crates/arcweft-lang-sema/src/callable/error.rs
```

Do not create `callable/mod.rs`. Re-export the intentional public surface from
`callable.rs` and `lib.rs`.

### 2.2 Content

Implement all exact declarations and inherent APIs in `FINAL_CONTRACT.md`:

- scalar/path/provider/candidate IDs;
- schema, docs, provenance, source records;
- immutable catalog record/set types;
- resolved targets/function values;
- target facts, semantic results, diagnostics;
- limits/work/error enums.

Owned enums receive behavior in their own inherent `impl`; do not add extension
traits or duplicate match helpers. Add no Serde, unsafe, unstable feature,
macro-generated public API, raw ID constructor, or compatibility alias.

### 2.3 Direct gates

- constructor success/failure for every scalar and index;
- every family ID resolves exact typed segments and rejects near misses;
- schema contiguous-index/name/rest/default/source invariants;
- non-empty candidate/result invariants;
- active signature/parameter/source identity invariants;
- exact and one-over test limits;
- `TypeKind` method keys use equality/hash without string formatting.

Compile gate:

```text
cargo fmt --all -- --check
cargo check -p arcweft-lang-sema --all-targets
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
cargo test -p arcweft-lang-sema --all-targets callable
```

## 3. Cut 2 — HIR and atomic catalog publication

### 3.1 HIR source publication

Update the smallest existing HIR model/project/lowering owners to add
`HirCallableSignatureSource`, parameter source rows, effect rows, and
`HirProject` ordered accessors. Reuse `CallableDeclarationId`, `FnSignature`,
docs, canonical package/module identity, and exact `SourceSpan`.

Add module rows for every module, including no-callable modules. Do not publish
source `impl` methods and do not render/parse a signature string.

### 3.2 Adapter publication

In `crates/arcweft-adapter-context/src/manifest.rs` and its existing typed Rust
metadata owner:

- replace callable string fields directly with `AdapterCallableName`,
  `AdapterCallablePath`, typed overload/group/parameter indices, multi-group
  `AdapterFunctionSignature`, passing/presence enums, and typed tooling subjects;
- migrate every standard/desktop/project-loader manifest constructor to typed
  segments; do not leave a dotted-string compatibility constructor;
- implement inherent `AdapterManifest::try_callable_publication`;
- form `AdapterPackageId` only from manifest `id`;
- classify the six standard manifest IDs through a typed inherent mapping;
- preserve the complete Rust package identity, Rust path provenance, exported typed path,
  purity, effects, tooling docs, parameter docs, defaults, rest, groups, and
  declaration order;
- remove only the callable-writing portion of the infallible env application
  after every caller moves; leave non-callable symbol registration on its
  current route.

### 3.3 Registration transaction

Update `arcweft-lang-sema/src/registration`, `env/registered.rs`, and their
existing constructors to:

1. collect HIR project callables and all project bindings;
2. add core standard publication;
3. add standard adapter publications;
4. add selected adapter publications;
5. finish/validate the complete catalog;
6. construct `RegisteredTypeCheckEnv` with `Arc<RegisteredCallableCatalog>`;
7. publish the complete registered world only after success.

Do not expose a partial publication API. Add tests that failed catalog creation
preserves the prior accepted world pointer and every prior component.

Compile gate:

```text
cargo check -p arcweft-lang-hir --all-targets
cargo test -p arcweft-lang-hir --all-targets callable_signature
cargo clippy -p arcweft-lang-hir --all-targets -- -D warnings
cargo check -p arcweft-adapter-context --all-targets
cargo test -p arcweft-adapter-context --all-targets callable_publication
cargo clippy -p arcweft-adapter-context --all-targets -- -D warnings
cargo test -p arcweft-lang-sema --all-targets registration
```

## 4. Cut 3 — migrate free-call families one at a time

For each numbered subcut, add resolver probe, schema construction, validator
routing, parity tests, then delete the old successful checker branch before the
subcut ends.

### 4.1 FX

- move `Fx.<member>` identity/schema into `FxCallableSignatureId` inherent impl;
- retain current `FxCatalog` user-definition validation as non-resolving facts;
- route property/stack/conditional/shader checks through the selected FX
  validator;
- prove unknown FX member cannot fall through;
- delete `check_fx_constructor_call` name resolution after routing.

### 4.2 enum, Result, Option

- construct candidate IDs without expected type;
- put expected owner/type in instantiation;
- preserve placeholder/poison recovery and exact payload diagnostics;
- delete checker-local successful constructor resolver.

### 4.3 builtin/capability

- implement typed path resolution in `BuiltinCallableId` inherent impl;
- preserve every operation and exact positional/named/spread diagnostic;
- normalize `event.emit` capability behavior;
- delete builtin name matches from checker.

### 4.4 Agent

- add inherent `AgentIntrinsicSignatureId::resolve/signature_schema`;
- route every listed intrinsic through selected validator;
- retain effect/entity/resource/path behavior;
- delete Agent name dispatch after all 30 IDs pass parity tests.

### 4.5 presentation

- add inherent presentation schema and typed owner acquisition;
- keep current open/closed named policies and state changes at commit;
- make `show.look` structural;
- delete presentation name dispatch from checker after every call passes.

### 4.6 lexical/project/environment/virtual/speaker/function values

- route exact lexical binding, project binding, environment catalog, virtual
  path validator, speakers, local functions, curried values, ordinary function
  values, and higher-order effects through one target product;
- make project non-callable binding terminal before environment lookup;
- delete successful `TypeCheckEnv` function map lookup and checker path-call
  resolver branches after normalized catalog/function-value tests pass.

Per-subcut gate:

```text
cargo test -p arcweft-lang-sema --all-targets <family_filter>
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

Use a crate-owned typed test counter or injected resolver fixture to establish
that only the shared resolver selected the target. Do not add tests that grep
source for old function names.

## 5. Cut 4 — migrate every selected/method family

Perform these subcuts in production precedence order:

1. drop;
2. `traverse` and `parallel`;
3. environment methods, including legacy-untyped normalized records;
4. collection methods;
5. presentation-handle methods;
6. integer methods;
7. remaining domain methods;
8. capacity methods;
9. trait outcomes;
10. data-last fallback;
11. unknown method recovery.

For each subcut:

- construct exactly one typed candidate ID and instantiated schema;
- route arguments through the shared engine;
- retain current result/effects/diagnostics and closed-name consumption;
- add direct collision tests against the next lower-priority family;
- preserve one data-last shadow warning where applicable;
- delete the old successful branch before proceeding.

After environment method migration, remove old overwrite semantics and final
`method_type` success. After capacity migration, put schema construction in the
owning table/enum inherent impl rather than a duplicate helper. After trait
migration, the existing trait catalog remains the selection authority; only its
outcome is normalized. After data-last migration, delete the old independent
fallback checker.

Gate:

```text
cargo test -p arcweft-lang-sema --all-targets method
cargo test -p arcweft-lang-sema --all-targets trait
cargo test -p arcweft-lang-sema --all-targets data_last
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

## 6. Cut 5 — dialogue and final structural character expectations

- construct `DialogueCalleeIdentity` from typed syntax/HIR judgment;
- route reserved `LineOptions` and open `LineArg` values through the common
  mapper;
- keep content-token, mark, FX, line-plan, wait/speed, rich-text, and inline
  failure validation outside callable identity;
- apply structural look expectation for speaker/speaker-preset owners;
- record typed owner-unavailable poison without guessing;
- prove same local spelling across character/look/part/variant owners remains
  isolated.

At this point both presentation and dialogue schemas, not checker-local word
logic, own look parameter expected types.

## 7. Cut 6 — checker target facts and public semantic results

- add `CallTargetFactMode` and private recorder to `TypeChecker`;
- record committed primary/equivalent/considered IDs, argument facts,
  function-value type, groups, effects, result, diagnostics, and poison;
- add `TypeCheckReport` accessors;
- construct `SemanticSignature` and `SemanticSignatureHelp` only from facts;
- compare checker and semantic result candidate IDs in direct tests;
- ensure `Disabled` allocates no fact vectors and `Focused` produces exactly one
  fact or typed error.

Do not connect an LSP word lookup. Remove the old word-only Rust metadata
fallback when the native semantic query is connected.

## 8. Cut 7 — connect AW-AH-009.3.1 and AW-AH-009.3.2

Only after both prerequisite cuts land:

- consume AW-AH-009.3.1's typed call/argument/range and cursor mapping to build
  ordered `CallArgumentRef` values and select the committed ordinary or
  fixed-spread slot fact used for the active parameter;
- consume AW-AH-009.3.2's accepted source/HIR/world lease and request
  cancellation owner to build `CallResolverRequest`;
- invoke one focused checker;
- project facts to semantic signature help;
- retain existing accepted-generation cache/stale policies from AW-AH-009.3/.2;
- delete any word-only, adapter-only, or second native successful resolver.

This contract does not authorize a temporary local parser or on-demand HIR
builder while waiting for those carriers.

## 9. Cut 8 — complete validation and documentation

### 9.1 Focused tests

```text
cargo test -p arcweft-lang-hir --all-targets callable_signature
cargo test -p arcweft-adapter-context --all-targets callable_publication
cargo test -p arcweft-lang-sema --all-targets callable
cargo test -p arcweft-lang-sema --all-targets registration
cargo test -p arcweft-lang-sema --all-targets presentation
cargo test -p arcweft-lang-sema --all-targets dialogue
cargo test -p arcweft-lang-sema --all-targets method
cargo test -p arcweft-lang-sema --all-targets signature
```

### 9.2 Workspace gates

Run from repository root:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Use the exact current root `AGENTS.md` command if it has changed by
implementation time. Record toolchain versions, command lines, exits, and audit
summary in the implementation note. Do not claim a pass without the raw result.

### 9.3 Dependency/visibility evidence

Use compiled public API tests/trybuild as already established in the workspace
and `cargo metadata --format-version 1` assertions to prove:

- HIR does not depend on sema;
- sema does not depend on adapter-context or LSP;
- adapter-context may depend on sema publication types;
- core/runtime-host do not gain syntax/HIR/sema normal dependencies;
- builder mutation is not publicly constructible;
- candidate/schema/result read APIs are reachable where required;
- no Serde/persisted callable catalog API was added.

These tests inspect typed API availability and Cargo metadata, not checked-in
source text.

## 10. Required deletion checklist

Delete by the end of migration, not deprecate:

- checker-local successful FX, builtin, Agent, presentation, constructor, free
  function, selected family, trait-normalization, data-last, and capacity
  dispatch branches;
- callable-mutating adapter manifest application;
- overwrite-based callable success from `TypeCheckEnv`;
- final untyped `method_type` successful fallback after normalization;
- signature-only/word-only/Rust-metadata successful resolver;
- hidden function-value/curried side channels replaced by explicit target
  facts, where they exist solely for call dispatch.

Retain only family validators that receive selected IDs and do not look up
names. Retain non-callable `TypeCheckEnv` responsibilities. Retain existing
trait catalog semantics. Retain existing presentation/dialogue content state
and validators behind committed candidates.

## 11. Stop conditions

Do not mark implementation complete when any of these remains:

- an omitted free or selected family;
- two successful resolvers;
- a successful old map lookup;
- a schema/result type the implementer had to invent;
- a same-rank collision deferred to query time;
- a display string used as identity;
- guessed character owner/part;
- partial accepted-world publication;
- missing exact/one-over tests;
- source-text gate;
- workspace/audit failure or unrecorded command result.
