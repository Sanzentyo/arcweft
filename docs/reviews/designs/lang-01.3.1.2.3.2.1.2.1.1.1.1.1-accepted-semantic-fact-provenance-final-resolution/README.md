# Accepted semantic-fact/provenance final resolution

Date: 2026-08-15

Status: direct Sol max resolution; implementation authority for the residual
conflicts in Lang-01.3.1.2.3.2.1.2.1.1.1.1.1.

Baseline:
`35d42efdd89fef8fde73f62be2a3e38fd5e81e52`

Resolves:
`docs/reviews/requests/2026-08-14-lang-01.3.1.2.3.2.1.2.1.1.1.1.1-accepted-semantic-fact-provenance-and-compile-clean-admission-order-correction.md`

Invalid retry intake:
`docs/implementation/2026-08-15-accepted-semantic-fact-provenance-retry-return-invalid.md`

## 1. Selected authority

The retry package is not implementation authority. This resolution preserves
the already-landed runtime semantic owner inventory, normalized
expression/pattern/local facts, local and type tables, checked/operational
classification, pattern binding coordinates, normalized variant cases,
catalog digests, and root identities.

The final flow is:

```text
accepted semantic facts
  -> one RuntimePlanBuilder
  -> recursively typed RuntimePlan
  -> independent generation admission
  -> AWBC v1 builder and admission
  -> same-parent admitted product
  -> compiler/bundle evidence
  -> runtime-driver publication
  -> host-selected core backend factory
```

Raw plan or AWBC data remains structural quarantine. It cannot issue or amend
independent generation facts.

## 2. Recursive typed expression and pattern authority

Do not add a raw-tree-plus-flat-type-sidecar wrapper. The final recursive node
is itself typed:

```rust
pub struct RuntimeExpr {
    ty: RuntimePlanTypeId,
    kind: RuntimeExprKind,
}

pub struct RuntimePattern {
    ty: RuntimePlanTypeId,
    kind: RuntimePatternKind,
}
```

All expression children are `RuntimeExpr`. All pattern children are
`RuntimePattern`. `IfLet`, `Match`, flow/stream/source match arms, and every
other pattern-bearing carrier retain the complete typed pattern rather than a
raw pattern or detached side table.

```rust
pub struct RuntimeExprMatchArm {
    pattern: RuntimePattern,
    guard: Option<RuntimeExpr>,
    value: RuntimeExpr,
}

pub enum RuntimeExprKind {
    // Existing expression families with recursive fields replaced by
    // RuntimeExpr/RuntimePattern and string locals replaced by plan-local IDs.
    IfLet {
        pattern: RuntimePattern,
        scrutinee: Box<RuntimeExpr>,
        guard: Option<Box<RuntimeExpr>>,
        then_expr: Box<RuntimeExpr>,
        else_expr: Box<RuntimeExpr>,
    },
    Match {
        scrutinee: Box<RuntimeExpr>,
        arms: Box<[RuntimeExprMatchArm]>,
    },
    Agent(RuntimeAgentExpr),
}
```

`RuntimeExpr::Local`, let/function parameters, map/filter parameters, captures,
and assignment destinations use the existing plan-local runtime IDs. They do
not retain source names as execution identity.

Pattern binding fields retain the existing
`RuntimePatternBindingCoordinate`. The public lowering seed supplies a
`RuntimeLocalDeclarationId`; only `RuntimePlanBuilder` derives the final path
from actual traversal. Whole and rest paths retain the landed schema-1 grammar
and depth limit.

```rust
pub enum RuntimePatternKind {
    Bind {
        mutable: bool,
        binding: RuntimePatternBindingCoordinate,
    },
    Discard,
    Literal(RuntimeValue),
    Tuple(Box<[RuntimePattern]>),
    Record {
        fields: Box<[RuntimeRecordPatternField]>,
        rest: RuntimePatternRest,
    },
    Sequence {
        items: Box<[RuntimePattern]>,
        rest: RuntimePatternRest,
    },
    Variant {
        ordinal: u32,
        payload: Option<Box<RuntimePattern>>,
    },
    Whole {
        binding: RuntimePatternBindingCoordinate,
        pattern: Box<RuntimePattern>,
    },
    Typed {
        binding: RuntimePatternBindingCoordinate,
    },
}

pub enum RuntimePatternRest {
    Exact,
    Ignore,
    Bind(RuntimePatternBindingCoordinate),
}
```

The retry types `RuntimeTypedExpr`, `RuntimeTypedPattern`,
`RuntimeExprTypeFact`, and `RuntimePatternTypeFact` are not introduced.

## 3. Builder-only construction

The external checked lowerer passes recursive seeds containing semantic type
declarations, source-accepted children, and plan-local identities. A seed is
not executable, serializable, or an admission token.

```rust
pub struct RuntimeExprSeed {
    declaration: RuntimePlanTypeDeclaration,
    kind: RuntimeExprSeedKind,
}

pub struct RuntimePatternSeed {
    declaration: RuntimePlanTypeDeclaration,
    kind: RuntimePatternSeedKind,
}

impl RuntimePlanBuilder {
    pub fn try_build_expr(
        &mut self,
        seed: RuntimeExprSeed,
    ) -> Result<RuntimeExpr, RuntimePlanBuildError>;

    pub fn try_build_pattern(
        &mut self,
        seed: RuntimePatternSeed,
    ) -> Result<RuntimePattern, RuntimePlanBuildError>;
}
```

For one root the builder:

1. preflights the complete recursive node set, declarations, locals, depth,
   pattern traversal, and bindings;
2. calls the landed atomic type-table batch interner;
3. rewrites the seed to final private nodes in the same traversal;
4. derives binding coordinates from that traversal; and
5. commits no node or type-table mutation on failure.

All lowerers borrow one builder. No unchecked type/local/node ID constructor or
second type interner exists.

## 4. Agent semantic and expression closure

Deterministic Agent constructors are runtime values, not untyped carriers.
Effectful Agent host operations remain non-value carriers. The landed runtime
expression inventory is changed accordingly:

- deterministic Agent constructor and probe-comparison calls: `Retain`;
- calls for which `host_operation().is_some()`: existing non-value call
  carrier behavior.

Add a closed normalized Agent family and a corresponding plan operational
family. Exact semantic identity remains the existing
`RuntimeSemanticTypeId`.

```rust
pub enum RuntimeTypeShape {
    // existing variants
    Agent(RuntimeAgentTypeShape),
}

pub enum RuntimeAgentTypeShape {
    DebugStatePath,
    ObservationFieldPath,
    Probe(Box<RuntimeNormalizedType>),
    Predicate,
    Observation,
    ObservedObject,
    BoundingBox,
    ActionName,
    ActionTarget,
    ActionResult,
    AgentValue,
    DataFormat,
    DataShape,
    EntityMetadata,
    SourceAnchor,
    ProjectGraphNeighborhood,
    ProjectGraphSymbol,
    ProjectGraphEdge,
    CaptureTarget,
    CaptureReference,
    Resource,
    ResourceBody,
    RagContextPack,
    Diagnostics,
    ViewportPoint,
}

pub enum RuntimeOperationalType {
    // existing variants
    Agent(RuntimeAgentOperationalType),
}
```

`RuntimeAgentOperationalType` has the same top-level closed families. A
normalized `Probe<T>` retains `T`; the operational plan family is `Probe`, and
the full generic identity remains in the semantic ID.

Standard Agent types currently represented by built-in stringly `Named`
spellings, including `Diagnostics` and `ViewportPoint`, move to a closed
sema-owned `AgentBuiltinType`. Compiler projection must not recognize them by
string spelling. Accepted external nominal/opaque types remain on their
existing authority.

The current anonymous record/string/tuple scaffold is deleted. Use one closed
expression algebra:

```rust
pub enum RuntimeAgentExpr {
    ChoiceAction { choice: RuntimeCommandTargetId },
    Target(RuntimeAgentTargetExpr),
    Path(RuntimeAgentPathExpr),
    Probe(RuntimeAgentProbeExpr),
    Predicate(RuntimeAgentPredicateExpr),
    ViewportPoint { x: Box<RuntimeExpr>, y: Box<RuntimeExpr> },
}
```

The target/path/probe/predicate sub-enums retain typed authored operands and
closed metadata such as comparison operators. Metadata like `kind` and `op`
is not a synthetic expression. `All` and `Any` retain a boxed expression list,
not a synthetic tuple.

The builder verifies the exact root-family mapping:

- ChoiceAction -> ActionTarget;
- Viewport/Layer/Object -> CaptureTarget;
- StatePath/ObservationPath -> their path families;
- Signal/Metric/State/Observation -> Probe;
- Diagnostics -> Diagnostics;
- Exists/ActionEnabled/All/Any/Not/Compare -> Predicate;
- ViewportPoint -> ViewportPoint.

Thus retry rows SYN-023 through SYN-061 are deleted. Every Agent root uses the
source call's accepted semantic type; authored operands use their own accepted
facts. No anonymous record type or value-shape inference is introduced.

Native evaluation produces a core-owned typed Agent runtime value. Conversion
to a host protocol record is owned by one Agent adapter. AWBC uses a closed
Agent runtime type and typed Agent construction instructions; it never lowers
these nodes through generic `MakeRecord` or reconstructs meaning from field
names.

## 5. RuntimePlan version-1 codec

`RuntimePlan` is compiler/developer IR, not an AWFB product section. Its final
derives are `Clone`, `Debug`, and `PartialEq`. Remove `Default`, public fields,
derived `Serialize`/`Deserialize`, struct literals, and `with_*` mutation.

Core owns one canonical version-1 codec and one semantic rebuild path:

```rust
impl RuntimePlan {
    pub fn encode_v1(&self) -> Result<Vec<u8>, RuntimePlanEncodeError>;
    pub fn decode_v1(
        bytes: &[u8],
        limits: RuntimePlanDecodeLimits,
    ) -> Result<Self, RuntimePlanDecodeError>;
}
```

If generic Serde remains necessary for a maintained compiler artifact, custom
Serde delegates to this same wire model and builder. It is not a second
reader.

Required wire field order is:

1. `schema_version = 1`;
2. local declaration count/table;
3. type declarations, where one-based ID equals row ordinal plus one;
4. typed sites in strict site order;
5. entries;
6. callable executables;
7. flow executables;
8. flows;
9. pure helpers;
10. trait methods;
11. line-task groups;
12. stream plans; and
13. source plans.

Every recursive expression/pattern wire node carries its encoded type ID.
Decode resolves that ID to a declaration, creates a private recursive seed,
and passes the complete root through the same builder. Builder-issued IDs must
equal the encoded canonical IDs. Decoded tables are never directly installed.

All fields are required; unknown/duplicate fields and non-1 versions fail.
Limits cover input bytes, every table, total and per-root expression/pattern
nodes, collection items, path depth 64, and semantic nesting 64. Validation is
byte budget -> wire/schema -> counts/nesting -> local table -> type table ->
recursive builder -> typed sites -> aggregate verification -> canonical
re-encode equality.

## 6. BytecodeProgram deletion

Delete `BytecodeProgram`, `BytecodeFlow`, `BytecodeInstruction`, their
parallel verifier/Serde/ABI, and RuntimePlan round-trip conversions. They lose
the final type/local/binding authority and would duplicate RuntimePlan.

- compiler/developer structured paths consume verified `RuntimePlan`;
- product AWFB contains canonical AWBC v1 only;
- Agent/product compilation emits AWBC;
- no structured product fallback or synthesized default remains.

## 7. AWBC version-1 aggregate

`AwbcProgram` is produced only by `AwbcProgramBuilder` or the canonical v1
decoder. Its fields are private and it has no `Default` or derived Serde.
Inspection formats wrap canonical AWBC bytes rather than serializing internal
tables.

The accepted retry decisions for one nominal-record-domain table, canonical
handle remapping, record-construction operands, independent admission, and
same-parent product pairing are retained. Privacy and legacy deletion occur in
the same cut as migration of the lowerer, codec, verifier, VM, bundle, driver,
host, codegen, tests, and fixtures.

## 8. Generation, evidence, and publication

`AdmittedRuntimeGeneration::try_issue` is a public trusted-integrator
structural boundary, not an unforgeability proof. Operational publication
requires compiler/bundle evidence. The bundle verifier owns byte/container
and trust-policy verification, independently admits generation, RuntimePlan,
and AWBC, and pairs an exact same-parent product.

Runtime-driver accepts only the bundle-owned verified product. Restore and
replay verify and publish the generation/product before admitting decoded
values or events. Save remains a lower Sans-I/O codec.

## 9. Cycle-free backend boundary

Do not add runtime-driver dependencies to JIT, runtime-codegen, or accelerator,
and do not add backend dependencies to runtime-driver.

Core owns executor-neutral contracts over admitted core products:

```rust
pub trait RuntimeAwbcBackendFactory {
    fn capabilities(&self) -> RuntimeAwbcBackendCapabilities;
    fn prepare(
        &self,
        input: RuntimeAwbcBackendPrepareInput<'_>,
    ) -> Result<RuntimePreparedAwbcBackend, RuntimeBackendPrepareError>;
}
```

The prepare input contains `&AdmittedRuntimeProduct`, target, and limits. It
does not expose raw artifact issuance. Runtime-codegen implements the AWBC
factory; Cranelift remains a pure-helper factory until it implements that ABI;
accelerator implements the applicable pure factory; core owns the VM factory.

Runtime-host/CLI/player selects the factories and passes backend bindings to
runtime-driver publication. Driver compares the capability report with the
admitted product, prepares the selected backend, validates returned
program/layout/host digests, and atomically retains the prepared backend in the
published generation.

Final dependency direction:

```text
core <- bundle, runtime-plan, driver, codegen, jit, accelerator
driver -> bundle + core + save
codegen -> core
jit -> core
accelerator -> core (+ optional jit)
runtime-host -> driver + selected backends + bundle + core
CLI/players -> runtime-host/driver
```

## 10. Compile-clean implementation order

1. Add the Agent semantic/runtime type closure and dedicated Agent expression
   algebra. Migrate deterministic Agent constructors; retain effectful host
   call disposition.
2. Add recursive seeds and builder validation while existing runtime carriers
   still compile. These new types are not operational entry points yet.
3. In one workspace carrier cut, migrate RuntimeExpr/RuntimePattern fields,
   flow/match/helper/trait/effect/audio/task/stream/source/line-task carriers,
   evaluators, pattern matching, runtime-plan lowering, and all tests. Delete
   string execution locals, anonymous Agent scaffolds, and old raw insertion
   paths in the same cut.
4. In that cut or its immediately inseparable codec cut, migrate all
   RuntimePlan constructors and persisted compiler artifacts to builder decode;
   delete public fields, Default, derived readers, setters, BytecodeProgram,
   and structured product fallback.
5. Add AWBC builder/domain/decoder capabilities while old consumers compile;
   then migrate all producers/consumers and privatize/delete the raw aggregate
   in one workspace cut.
6. Add core backend contracts and backend implementations while current
   execution remains operational. Switch runtime-host composition and driver
   publication atomically, then delete raw backend preparation entry points.
7. Complete generation-first restore/replay and product-only checked-value/
   nominal-domain issuance, then run full deletion and workspace gates.

No phase privatizes an aggregate before all current consumers migrate. Every
Arcweft-owned version marker remains `1`; no legacy reader or compatibility
alias is retained.
