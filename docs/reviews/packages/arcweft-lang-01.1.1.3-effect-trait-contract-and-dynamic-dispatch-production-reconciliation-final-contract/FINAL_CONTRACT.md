# Lang-01.1.1.3 final contract

## 0. Normative status and precedence

The key words **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.
This contract is an implementation-ready correction to the accepted
Lang-01.1.1 direct-style suspension/generator parent.

It supersedes the parent only where the parent left trait-method effects,
diagnostics, or dynamic-object dispatch incomplete. It does not alter:

- the sole authored ordinary function spelling `fn`;
- the accepted ordinary-function parser and HIR path;
- `DirectFrame`/`StreamFactory` classification;
- direct Await typing, static `control.suspend`, borrow diagnostics, runtime
  readiness, cancellation, or stack preservation;
- AWBC call, suspension, cancellation, or Stream wire/opcode contracts;
- project nominal identity; or
- the accepted call resolver's candidate precedence outside the trait-method
  identity/effect join described here.

A production implementation MUST NOT introduce compatibility aliases, shims,
dual readers, source gates, removed-syntax-specific diagnostics, source
reparsing, string-ID fallbacks, CSS, or Takumi paths.

## 1. Final semantic model

### 1.1 Sole checked identity

Every source callable participating in type/effect checking has exactly one
`CheckedCallableId`. The following are different declaration kinds but use the
same checked-ID type:

- existing ordinary/project callables;
- bodyless trait requirements;
- trait implementation methods;
- inherent methods; and
- standard-library trait methods.

Closures use `CheckedClosureId`, whose owner is a `CheckedCallableId` and whose
exact closure-expression span is revision-bound. The effect graph key is
`CheckedEffectCallableId::{Declaration, Closure}`. No source callable is keyed
by a display string, trait name string, local impl vector index alone, or a
reparsed source fragment.

The exact Rust-shaped identity contract is in `IDENTITY_AND_OWNERSHIP.md`.
`TraitId`, `ImplId`, `AssociatedTypeId`, and `TraitWitnessId` remain compact
catalog-local handles in this cut. They are not callable/effect identity and
MUST NOT be published as such.

Compiler lowering MUST derive an opaque `RuntimeCallableId` from a
`CheckedCallableId` exactly once. That value is a downstream runtime-plan
projection, not another declaration/effect authority: it is never used to
resolve names, select a trait/impl, recover source, or obtain a row. The exact
projection, runtime identity shape, and deletion of the current string/local-
index trait-method identity are normative in `IDENTITY_AND_OWNERSHIP.md` §11.

### 1.2 Sole effect authority

The immutable `CheckedCallableCatalog` is the sole checked effect authority.
For each declaration it owns one `CheckedCallableFacts` record containing:

- the `CheckedCallableId`;
- resolved callable signature;
- revision-bound declaration/name/signature/contract/body source;
- typed access policy;
- checked execution role (`Runtime(CallableExecutionMode)` or
  `DispatchContract`); and
- `CheckedCallableEffects`.

`CheckedCallableEffects::Body` owns the callable's one final inferred row and
its one effect contract. `CheckedCallableEffects::BodylessTraitRequirement`
owns the dispatch contract and has no inferred body row. No trait catalog,
resolver candidate, method value, call fact, project-index record, CLI record,
or LSP record owns a second authoritative row.

The current effect fixed-point engine remains the inference algorithm. It is
fed by the existing checker traversal. A second HIR/body walk dedicated to
trait effects is prohibited.

### 1.3 Actual row and exposed row

For a body-bearing callable:

- `actual_row()` is the fully resolved row inferred from its body;
- `exposed_row()` is the authored bounded row when one exists, otherwise the
  actual row.

For a bodyless trait requirement, `exposed_row()` is its bounded dispatch row
and `actual_row()` is unavailable.

Named calls and function/method values propagate the **exposed** row. This
preserves an authored public effect contract and separate-compilation safety.
Body validation and trait-implementation conformance use the **actual** row.
A concrete method may therefore implement a broad contract with a narrower
body; a static witness call still propagates the requirement contract.

## 2. Syntax and exact source ownership

### 2.1 One contract-clause AST

The current source-less `ContractClause` representation MUST be directly
replaced, not wrapped by a parallel reader:

```rust
pub struct ContractClause {
    kind: ContractClauseKind,
    source: ContractClauseSource,
}

pub enum ContractClauseKind {
    Requires { mode: Option<String>, expr: Expr },
    Ensures { mode: Option<String>, expr: Expr },
    Invariant { mode: Option<String>, expr: Expr },
    Assume { expr: Expr },
    Reads(Vec<Expr>),
    Effects(Vec<Expr>),
    NoEffect(Expr),
    Modifies(Vec<Expr>),
    Decreases(Expr),
}

pub struct ContractClauseSource {
    whole: TextRange,
    keyword: TextRange,
    payload: TextRange,
    items: Box<[TextRange]>,
}
```

Fields remain private. `kind()`, `source()`, `mode()`, `solver_assumption()`,
`solver_invariant()`, `solver_claim()`, and effect classification are inherent
methods on the owning types. There is no extension trait or endpoint-specific
helper that reconstructs behavior from strings.

`parse_contract_clause_at` is the one parser entry used by ordinary functions,
flows, trait members, impl members, and the shadow/event parser. It recognizes
all already accepted contract clauses, including `effects` and `no_effect`.
The current shadow path that recognizes only a subset MUST be replaced in the
same syntax cut. There is no dual contract reader.

### 2.2 Trait and impl member source records

`TraitMember::Function` and `ImplMember::Function` MUST retain the same exact
source categories as an ordinary function:

```rust
pub struct MethodDeclarationSource {
    whole: TextRange,
    name: TextRange,
    signature: FunctionSignatureSource,
    contracts: Box<[ContractClauseSource]>,
    body: Option<TextRange>,
}
```

`parse_trait_member` and `parse_impl_member` MUST use
`split_function_header_lines`, the existing function signature parser, and the
single contract-clause parser. The method body is parsed exactly once by the
existing expression/body parser. No sema source reparse is permitted.

### 2.3 Effect-clause binding

Each `effects` clause lowers through the existing effect catalog into one
`EffectClauseSource`:

```rust
pub struct EffectClauseSource {
    whole: SourceSpan,
    keyword: SourceSpan,
    items: Box<[EffectItemSource]>,
}

pub struct EffectItemSource {
    effect: EffectId,
    span: SourceSpan,
}
```

Clause order and item order preserve source order. Semantic effect sets remain
canonical/sorted `EffectSet`s.

Multiple authored `effects` clauses are one contract: their concrete effects
are unioned. The first clause in source order is the diagnostic primary span;
later clauses are related spans. An explicit empty clause `effects {}` is a
closed empty row.

`no_effect` continues to add a forbidden-effect constraint. It is not an empty
upper-bound spelling.

## 3. Effect-contract construction

### 3.1 Existing row model only

The implementation MUST use the existing `EffectRow`, `EffectRowTail`,
`EffectVar`, and `EffectSubstitution`. It MUST NOT add a trait-only row type or a
second row grammar.

- Authored `effects` clauses contribute the concrete row head.
- An already supported typed effect-row variable in the callable signature
  contributes the row tail and its existing `TypeSourceEvidence`.
- No typed tail means `EffectRowTail::Closed` for an authored row.
- `EffectRowTail::Unknown` is never accepted as an open contract.

Thus `effects { fs.read }` is `{fs.read | closed}`; the same head plus typed tail
`rho` is `{fs.read | rho}`. No new source syntax for an open row is introduced
by this cut.

### 3.2 Exact constructors

The final contract owner is:

```rust
pub struct CallableEffectContract {
    permission: EffectPermission,
    forbidden: EffectSet,
    source: EffectContractSource,
}

pub enum EffectPermission {
    UnboundedInference,
    Bounded(EffectRow),
}

pub enum EffectContractOrigin {
    BodyInference,
    Authored,
    OmittedBodylessTraitRequirement,
}
```

Its fields are private. These are the only crate-visible constructors:

```rust
pub(crate) fn body_inference(
    anchor: SourceSpan,
    forbidden: EffectSet,
    forbidden_sources: Box<[EffectItemSource]>,
) -> Self;

pub(crate) fn authored(
    row: EffectRow,
    clauses: Box<[EffectClauseSource]>,
    typed_tail_source: Option<TypeSourceEvidence>,
    forbidden: EffectSet,
    forbidden_sources: Box<[EffectItemSource]>,
) -> Result<Self, EffectContractBuildError>;

pub(crate) fn omitted_bodyless_trait(method_name: SourceSpan) -> Self;
```

`authored` requires at least one clause or typed-tail source and rejects an
unknown tail. `omitted_bodyless_trait` always constructs:

```rust
EffectPermission::Bounded(EffectRow::closed(EffectSet::new()))
```

with origin `OmittedBodylessTraitRequirement`, no clause span, and the exact
method-name span as its implicit anchor. Absence of an authored clause is not
stored as `None` at the semantic contract boundary.

### 3.3 Body-bearing methods

A body-bearing inherent method or trait implementation method follows the same
rules as an ordinary function:

- no clauses/tail: `UnboundedInference`;
- authored closed row: bounded closed upper bound;
- existing typed open row: bounded open upper bound;
- body is checked once and produces one final actual row.

A bodyless trait requirement does not infer. A trait default body remains the
currently deferred feature and continues to be rejected by the existing typed
trait-member support boundary; this cut does not use it as effect evidence.

An **authored** bodyless impl method is invalid implementation
syntax/semantics and MUST be rejected before callable facts are frozen. It does
not become an empty body row. A programmatically installed standard/builtin
implementation has no authored body and is represented only by
`CheckedCallableEffects::ExternalOrStandard { exposed }`; its declared row is
installed by the standard catalog version and is never inferred as empty.

## 4. Trait method catalog and conformance

### 4.1 Trait records refer to the checked owner

The following current fields MUST be added/replaced:

```rust
pub struct TraitMethodRequirement {
    declaration: CheckedCallableId,
    trait_id: TraitId,
    name: String,
    signature: FnSignature,
    self_parameter: GenericTypeParameterId,
    param_groups: Vec<Vec<FunctionParam>>,
    return_type: TypeKind,
}

pub struct TraitMethodImpl {
    declaration: CheckedCallableId,
    trait_id: Option<TraitId>,
    signature: FnSignature,
    param_groups: Vec<Vec<FunctionParam>>,
    return_type: TypeKind,
    body: Option<TraitMethodBody>,
}
```

Neither record has an effect row, copied effect set, effect clause list, or
string callable ID. It reaches effects only through `declaration` and the
`CheckedCallableCatalog`.

Inherited requirement collection MUST retain the original declaring
requirement ID. It MUST NOT clone a requirement into a child-trait-owned method
record. The current predicate/static path that synthesizes a
`TraitMethodImpl` from a requirement MUST be removed; resolution returns the
canonical requirement ID directly.

### 4.2 Conformance owner

One implementation method can satisfy one or more original inherited
requirements. Each relation has an exact key and stores no copied row:

```rust
pub struct TraitMethodConformanceId {
    implementation: CheckedCallableId,
    requirement: CheckedCallableId,
}

pub struct TraitMethodConformance {
    id: TraitMethodConformanceId,
    witness: TraitWitnessId,
    substitution: TraitMethodSubstitution,
}

pub struct TraitMethodSubstitution {
    types: BTreeMap<GenericTypeParameterId, TypeKind>,
    effects: EffectSubstitution,
}
```

Fields are private; types and read-only accessors are public where compiler or
tooling consumes them. Construction is `pub(crate)` in the trait-catalog
builder and succeeds only after signature compatibility and effect subset
validation. `CheckedCallableCatalog` owns the conformance map.

### 4.3 Exact subset operation

`EffectRow` owns the operation:

```rust
pub fn check_subset(
    actual: &EffectRow,
    permitted: &EffectRow,
    substitution: &mut EffectSubstitution,
) -> Result<(), EffectSubsetError>;
```

It is an inherent method/associated operation in `effect_row.rs`; no trait-only
helper or second algorithm is permitted.

The algorithm is normative:

1. Resolve both rows transitively through the supplied substitution.
2. Compute `residual = actual.concrete - permitted.concrete`.
3. If the permitted tail is closed, every residual effect is missing. An
   unresolved actual variable tail is `UnresolvedActualTail`, never accepted as
   empty.
4. If the permitted tail is variable `rho`, bind or constrain `rho` to the
   complete residual row. If the actual row has variable tail `tau`, the bound
   is `{residual | tau}`; no concrete effect is discarded.
5. If `rho` already has a binding, recursively check the residual actual row
   against that binding. The substitution is merged, not overwritten.
6. `Unknown` on either checked side is `UnknownRow` and blocks lowering; it is
   not an open row.
7. Missing concrete effects are returned as one sorted `EffectSet`.

Examples:

```text
actual    = {control.suspend, fs.read, log.write | tau}
permitted = {log.write | rho}
result    = bind rho to {control.suspend, fs.read | tau}
```

```text
actual    = {control.suspend | closed}
permitted = {} closed
result    = missing {control.suspend}
```

The operation is applied after type, associated-type, self-type, and effect
substitutions. `control.suspend` is an ordinary `EffectId`; it receives no
trait-specific exception.

### 4.4 Inherited and colliding requirements

The transitive requirement set is keyed by canonical requirement ID.

- One implementation method MAY satisfy multiple inherited requirements when
  their substituted signatures are compatible. Each pair receives a distinct
  conformance record and is effect-checked separately.
- If same-named inherited requirements have incompatible signatures, the
  existing typed inherited-method conflict is emitted before effect checking.
- If signatures agree but effect contracts differ, the implementation must be
  a subset of every requirement. Failure reports each distinct violated
  requirement; identical primary/implementation/effect payloads are
  deterministically coalesced.
- Traits with the same name in different modules remain distinct because the
  declaration key includes package/module identity. Unqualified ambiguity is a
  normal typed lookup ambiguity, not a string-name tie-break.

## 5. Resolution, calls, currying, and method values

### 5.1 Resolver authority switch

The shared callable resolver retains its accepted candidate precedence. Its
trait-method identity and effect projection change atomically:

- delete `TraitCallableId`;
- method candidates carry `CheckedCallableId`;
- `CallableEffectSchema::Project { declaration, declared }` becomes an ID-only
  checked schema and queries `CheckedCallableCatalog::exposed_row`;
- delete the resolver's hard-coded `EffectRow::closed(EffectSet::new())` for
  methods;
- delete any trait-name/local-impl-index identity reconstruction.

A concrete resolved method target carries the implementation method ID and,
when applicable, its conformance/witness. A generic static-witness target may
have no concrete implementation ID; it carries the original requirement ID and
the typed witness/predicate evidence.

### 5.2 Call-row selection

| Call form | Row propagated to caller |
|---|---|
| ordinary source callable | callable `exposed_row()` |
| concrete inherent method | inherent method `exposed_row()` |
| concrete trait impl method | impl method `exposed_row()` |
| generic/static witness dispatch | requirement `exposed_row()` after substitution |
| function/method value creation | none; row stored latently in value type/facts |
| non-final curried application | argument-evaluation effects only |
| final curried application | stored exposed row |
| unknown indirect row | reject before lowering |

No call form infers or copies a trait row locally.

### 5.3 Bound method values

The current project-method rejection path is replaced by:

```rust
pub struct BoundMethodValue {
    receiver: TypeExpressionId,
    target: BoundMethodTarget,
    signature: FunctionSignature,
    effects: EffectRow,
    next_group: CallableGroupIndex,
}

pub enum BoundMethodTarget {
    Inherent {
        implementation: CheckedCallableId,
    },
    ConcreteTrait {
        implementation: CheckedCallableId,
        conformance: TraitMethodConformanceId,
    },
    StaticWitness {
        requirement: CheckedCallableId,
        witness: TraitWitnessId,
    },
}
```

Fields are private. Construction is `pub(crate)` in method resolution; compiler
and tooling receive read-only accessors. The receiver is checked/evaluated once
and captured as group zero. The resulting function signature removes the
receiver parameter and preserves all remaining parameters and curried groups.
The same substituted exposed effect row is latent until the final group is
invoked.

The implementation MUST NOT lower a bound method value to a synthetic named
closure or allocate a string callable ID. Direct method calls and final bound
method invocations converge on the same typed method call-target lowering.

### 5.4 Aliases and reexports

An import alias or reexport is a lookup binding to the original typed trait or
callable declaration. It does not create a checked callable record. A method
reached through an aliased/reexported trait binding retains the original
requirement and implementation IDs.

For this contract, “method alias” means that existing binding behavior; no new
member-alias syntax is introduced. Any catalog alias record contains only
`target: CheckedCallableId` and source visibility/spelling metadata. It has no
row field.

### 5.5 Visibility, missing, and ambiguity

Visibility is resolved before a candidate or call edge is committed.

- A trait requirement inherits the trait declaration visibility.
- A trait impl method is reachable only through an accessible requirement and
  valid impl/witness; it is not inserted into module value scope.
- An inherent method is reachable under the current impl/owner module access
  rule and is not a top-level binding.
- Private, missing, or ambiguous resolution produces the existing typed
  lookup/trait diagnostic and records no effect edge or method value.
- There is no same-spelling fallback after a typed failure.

## 6. Effect collection and fixed point

### 6.1 One checker traversal

Before any body is checked, all source callable shells and effect contracts are
registered by checked ID. The existing `TypeChecker` then enters each ordinary
function, trait impl method, and inherent method exactly once and uses the
existing expression/statement traversal to record:

- direct primitive effects;
- typed local/project method call edges;
- typed indirect rows; and
- exact source spans.

`EffectCollector` journal entries, current callable, known-callable lookup,
inferred-row map, call edges, and rollback operate on
`CheckedEffectCallableId`, never `String` or the legacy `CallableId(String)`.

### 6.2 Finalization order

1. Finish all body checks.
2. Apply already accepted pending higher-order/currying effect calls.
3. Run the existing least fixed point once over every declaration and closure.
4. Resolve every body row; unresolved/unknown rows produce a typed blocking
   diagnostic and do not reach lowering.
5. Validate each body's own authored bounded contract.
6. Validate trait-method conformance against original requirement rows.
7. Freeze `CheckedCallableCatalog` and publish one `Arc` in `TypeCheckReport`.

No body is revisited for trait conformance.

### 6.3 Rollback and stale identities

A checked ID is revision-bound by its context. A project ID from a different
world or `ProjectSymbolRevision`, a detached ID from a different
`SourceDocumentIdentity`, or a standard ID from a different standard-catalog
revision is stale and is rejected by typed lookup.

Speculative checker/resolver rollback journals records and edges by typed ID.
Uncommitted records never escape the builder. A rolled-back ID may become valid
again only if the same exact source declaration is legitimately re-registered
and committed; consumers always query the current immutable catalog rather than
trusting a cached row.

## 7. Diagnostics

The exact variants, codes, payloads, spans, deterministic trace algorithm,
ordering, and rendering contract are normative in
`DIAGNOSTIC_AND_SOURCE_CONTRACT.md`.

At the authority switch:

- remove legacy `UpperBoundExceeded`/`AWF-EFX-001`;
- add the exact E015/E016/E022/E023 typed variants;
- replace text/path/line/column effect traces with typed revision-bound spans;
- retain unrelated legacy diagnostic variants only when they express a still
  valid distinct invariant; and
- do not provide a compatibility code mapping or emit old and new diagnostics
  for the same violated contract.

A callable's own authored contract and a trait conformance contract are distinct
constraints. If one body violates both, one diagnostic for each violated owner
is valid; this is not legacy/new dual emission. Diagnostics are sorted and
coalesced as specified in the diagnostic sidecar.

## 8. Project indexing, compiler, CLI, and LSP

### 8.1 Project symbol publication

Project symbol linking publishes typed trait declarations, impl declarations,
and method callable declarations before sema body checking. Method callable
symbols are stored by declaration ID but are not inserted into module value
scope.

The project symbol table's world/revision is copied into
`CheckedCallableContext::Project`; aliases and reexports retain target IDs.

### 8.2 TypeCheckReport and project index

`TypeCheckReport` owns one `Arc<CheckedCallableCatalog>`. Separate public maps of
source callable effect rows are removed or become derived iterator APIs over
that catalog.

`ProjectSemanticIndex` also retains the same `Arc` for the live checked project.
`ProjectCallableSymbol` stores declaration ID, kind, signature, source, and
semantic hash but no authoritative row. Effect queries delegate to the checked
catalog. Any durable external index serialization is an output projection and
must include the checked catalog semantic hash; it is not a second compiler
owner.

`ProjectCallableKind` gains `TraitRequirement`, `TraitImplementation`, and
`InherentMethod`, with `as_str` implemented on the enum itself. A requirement is
indexed for navigation/signature/effect contract but is not a runtime target.

### 8.3 Compiler and runtime-plan projection

Compiler call-target facts consume `BoundMethodTarget` or the existing ordinary
call target with `CheckedCallableId`. Static witness lowering retains the
requirement ID and exact `TraitMethodConformanceId`; concrete lowering retains
the implementation ID and conformance when present. The compiler does not
re-run trait resolution or query source text.

The compiler bridge in `crates/arcweft-compiler/src/trait_methods.rs` converts
each executable method declaration to the existing general `RuntimeCallableId`
through the exact checked-callable digest defined in the identity sidecar. It
also keeps a compiler-only map from typed `TraitMethodConformanceId` (or the
inherent method `CheckedCallableId`) to the emitted `RuntimeTraitMethodId`.
That map is not serialized and contains no method-name key.

The final runtime-plan identity is:

```rust
pub struct RuntimeTraitMethodIdentity {
    implementation: RuntimeCallableId,
    requirement: Option<RuntimeCallableId>,
}
```

`RuntimeTraitMethodId` remains the typed plan-local executable/code identity.
Runtime trait-method inputs are sorted by the typed checked lowering key before
those IDs are assigned. `RuntimeTraitMethodInventory` stores only the method
vector; the current `by_witness_method: BTreeMap<(usize, String), ...>` is
deleted. Iterator/static-witness evidence receives direct emitted
`RuntimeTraitMethodId` values selected by the compiler-only typed map.

The following current identity fields are deleted together:
`impl_id: usize`, `trait_id: Option<usize>`, `witness: Option<usize>`,
`trait_name: Option<String>`, `self_type: String`, `method_name: String`, and
`monomorph_label: String`. Human-readable labels, when needed for debugging,
are non-authoritative derived display data outside identity and are not used
for lookup. Runtime never reparses a `RuntimeCallableId` or display label.

This runtime-plan identity replacement changes the serialized trait-method
shape, so the directly owned plan serialization schema/fingerprint MUST be
updated in the same cut. It MUST NOT add a compatibility decoder,
dual identity, Stream opcode, or AWBC dispatch path.

### 8.4 CLI and LSP

`EffectDiagnostic::diagnostic()` is the one renderer into
`arcweft_source::Diagnostic`. `TypeCheckError::diagnostic()` delegates. CLI and
LSP consume that result; LSP only validates the `SourceSpan` against the open
revision and converts it to protocol ranges. Neither consumer reparses source
or branches on authored spelling.

## 9. E017 disposition

Parent E017 is `SUPERSEDED_FOR_LANG_01_1_1`. Dynamic trait objects remain a
future feature and are not implementation scope for this cut. The parser keeps
ordinary rejection for unsupported type grammar; no dedicated `dyn` branch or
diagnostic is added.

Replacement `E017S` covers static witness dispatch and is normative. The full
rationale and future-feature boundary are in
`DYNAMIC_DISPATCH_DISPOSITION.md`.

## 10. Required deletion state

The final production state has none of the following authorities:

- `TraitCallableId`;
- trait-name plus local impl-index callable identity;
- legacy local `CallableId(String)` for source callables/closures;
- hard-coded closed-empty effects in method resolver results;
- copied effect rows in `TraitMethodRequirement`, `TraitMethodImpl`, resolver
  schemas, or project callable symbols;
- synthesized requirement-as-implementation method records;
- a project-method-specific method-value rejection path;
- legacy `UpperBoundExceeded`/`AWF-EFX-001` for closed rows;
- path/line/column-only or text-only effect traces;
- a second contract-clause parser;
- `RuntimeTraitMethodIdentity` fields based on local trait/impl/witness indices,
  trait/method/self-type strings, or a monomorph display label;
- `RuntimeTraitMethodInventory::by_witness_method` keyed by `(usize, String)`;
- compiler trait-method selection by a witness plus method-name string; or
- source-text removal gates.

Removal is proven by typed behavior, compile-fail API visibility, diagnostic
payloads, resolver work accounting, and structured dependency tests listed in
`TEST_MATRIX.md`.

## 11. Independence from adjacent programs

No exact dependency was found on the pending Stream wire/opcode correction or
the Proof syntax/HIR public switch. This semantic authority cut MUST proceed
independently. It may use the already accepted callable identity/catalog,
source revision, generic substitution, direct Await source, and static witness
substrates, but MUST NOT wait for or redesign those adjacent programs.
