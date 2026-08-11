# Production reconciliation

## 1. Reconciliation statement

Current production has authoritative type behavior but distributes successful
call resolution across checker-local branches, environment maps, trait lookup,
method fallback, and presentation/dialogue helpers. The target does not replace
those semantics with a generic approximation. It extracts identity, schema,
precedence, argument mapping, and target facts into one sema-owned substrate,
then keeps only irreducibly family-specific value validation behind the selected
candidate.

The required end state is one successful resolver. “Catalog” does not mean that
every language special form must be persisted in `RegisteredTypeCheckEnv`.
Project and environment declarations are immutable catalog records; closed
language families construct the same schema type from typed inherent IDs. Both
routes meet in `ResolvedCallable` before any argument is committed.

## 2. Current production seams and exact target owners

| Current seam | Current role | Target owner | Reconciliation |
|---|---|---|---|
| `checker/expr.rs::check_call_expr` | ordered free-call dispatch | `callable::resolver` | replace branch sequence with one request; preserve order exactly |
| `checker/expr.rs::check_selected_callee_call` / `check_typed_method_call` | selected dispatch | `callable::resolver` | one selected precedence table; receiver checked once |
| `checker/expr/fx.rs` | FX resolution and validation | ID/schema inherent impl + FX validator | move resolution/schema; retain property/value validation |
| `checker/expr/builtin.rs` and builtin matches in `expr.rs` | builtin string matching and argument checks | `BuiltinCallableId` inherent impl + builtin validator | typed path resolution and common schema; delete checker name match |
| `checker/expr/enum_variant.rs` and Result/Option helpers | expected-type constructors | resolver + constructor validator | candidate ID excludes expected type; instantiation carries it |
| `checker/expr/agent.rs` | Agent name match and validation | `AgentIntrinsicSignatureId` inherent impl + Agent validator | move name/schema; retain semantic checks/effects |
| `checker/presentation.rs` | presentation name match, value checks, state | `PresentationCallableId` schema + validator | schema supplies structural look; state mutates only on commit |
| dialogue option checking in checker/module/line paths | option expressions and content validation | `DialogueCallableId` schema + dialogue validator | options use shared mapping; content validation remains separate |
| `TypeCheckEnv` function/method maps | overwrite-based callable storage | `RegisteredCallableCatalog` | normalize records, reject collisions, delete successful map reads |
| `RegisteredTypeCheckEnv` | base env + character/external facts | same type with immutable callable catalog | build catalog in same atomic world transaction |
| adapter manifest `apply_to_type_check_env` | infallible overwrite | inherent `try_callable_publication` | typed fallible publication; delete callable mutation |
| HIR project/source functions | checker-local binding | `HirCallableSignatureSource` | publish canonical source signature/docs/spans/effects |
| trait catalog resolution | visible trait selection | existing trait catalog + typed `TraitCallableId` | resolver consumes existing outcome; no duplicate trait engine |
| `checker/expr/method_fallback.rs` | data-last lookup/check/effects | resolver candidate + data-last validator | shared visibility, mapping, transaction, shadow warning |
| `well_known_capacity_method_type` | untyped capacity table | owner table inherent schema construction | normalize to typed capacity ID/schema |
| function-value/curried checker fields | transient effect/group communication | `ResolvedFunctionValue` and target facts | explicit typed product, no hidden second lookup |
| signature-help feature fallback | word/Rust metadata lookup | projection from checker facts | delete after native query consumes target facts |

## 3. Current free-call behavior preserved

The existing checker order is behavior, not an implementation accident. The
reconciliation preserves:

1. FX definition validation before target selection;
2. closed FX namespace, expected enum/Result/Option, builtin, Agent, and
   presentation priorities;
3. ordinary function signature checking, virtual-path policy, effects, and
   curried/partial results;
4. path-special promotion/assumption/speaker/local/function-value behavior;
5. recovery checking of arguments when a target is non-callable or missing.

The migration must not first publish project/environment names into one global
map and then let that map shadow language-owned families. Reserved family
priority stays explicit in the shared resolver.

## 4. Current selected-call behavior preserved

Production resolves methods in an order that differs from a conventional
“inherent, trait, extension” language. The target retains:

```text
drop
traverse / parallel
environment method
collection
presentation handle
integer
domain
capacity
trait
data-last
unknown
```

In particular, environment methods currently precede collection and other
builtin methods. The target catalog therefore cannot simply be consulted after
all inherent tables. Direct tests must pin every collision listed in
`SURFACE_INVENTORY.md`.

Trait ambiguity remains terminal. Capacity remains untyped. Data-last remains a
fallback and warning source, not an ordinary equal-priority overload.

## 5. HIR publication reconciliation

### 5.1 Required additions

`arcweft-lang-hir` adds immutable callable signature-source rows during normal
lowering/project assembly. The rows reuse:

- existing `CallableDeclarationId`;
- canonical package and module identity;
- typed `FnSignature` and parameter groups;
- existing docs and source spans;
- existing effect declaration/inferred callable identity.

`HirProject` retains one ordered callable slice for every module, including
empty slices. This prevents registration from discovering modules by observing
only callables and gives accepted-world construction complete module/source
coverage.

### 5.2 Required non-additions

The HIR cut does not add:

- a sema candidate ID;
- a rendered signature string;
- a source-text parser;
- a source `impl` method catalog;
- a serialized callable catalog;
- a compatibility copy of old function maps.

### 5.3 Non-callable shadowing

The callable catalog and project symbol binding map are built together. A
project symbol path resolving to a non-callable stores its exact `TypeKind` and
blocks environment fallback. The resolver never equates “no callable record”
with “unbound”; it asks the typed project binding authority.

## 6. Adapter reconciliation

### 6.1 Identity

`AdapterManifest::id` alone forms `AdapterPackageId`. Known standard manifest
IDs map to typed standard owners. Manifest display name, source path, selected
profile, Rust package name, Rust item path, docs, and symbols never participate.

### 6.2 Typed normalization

The manifest parser/builder produces typed callable paths, method names,
receiver types, parameter groups/kinds/default/rest facts, result type, effects,
and docs. Typed Rust metadata contributes provenance and any typed parameter
documentation. Sema receives only `EnvironmentCallablePublication`.

No sema code imports adapter structs or reconstructs identity by parsing:

- a dotted function label;
- a Rust `foo::bar` display path;
- a human-readable signature;
- tooling documentation;
- comments.

### 6.3 Collision behavior

Current map insertion can overwrite. The target rejects same-rank providers,
rejects duplicate typed IDs, allows explicit overloads within a provider,
coalesces exact Standard/Adapter duplicates, and retains non-equal
Standard/Adapter overloads with deterministic ordering. The entire accepted
world fails before publication on invalid input.

## 7. Schema reconciliation

### 7.1 One schema type

Project, environment, Rust-adapter, language, trait, data-last, capacity,
function-value, and public semantic results all use
`CallableSignatureSchema`. There is no checker-only `FunctionSignature` success
path once migration completes. Existing `FunctionSignature` may remain as a
private construction input during a cut, but it cannot remain an independent
successful resolver or public signature-help source.

### 7.2 Family validators

A schema expresses parameter groups, names, passing modes, required/defaulted/
optional/rest status, exact or unchecked type, result, effects, open-name and
spread policy, and validator identity. Family validators retain checks that are
not ordinary signature compatibility. They take a selected candidate; they do
not match a path/method string.

### 7.3 Presentation/dialogue structural typing

Current `show.look` and dialogue `look` are ordinary expression checks on the
audited main. The target schema acquires the typed character owner before
argument checking and installs an exact structural nominal expectation.
Unknown ownership does not change the selected presentation/dialogue family; it
creates an unchecked affected parameter plus typed diagnostic/poison.

## 8. Resolver/checker reconciliation

### 8.1 Name resolution happens once

`resolve_call_target` consumes typed callee identity, lexical scope, project
symbols, registered catalog, expected and receiver types, trait predicates,
module, source identity, group, cancellation, and work. Its result is resolved,
missing, non-callable, or typed rejection. It never checks argument expressions.

### 8.2 Candidate checking is transactional

Overloads require viability checks, but not a second name resolver. Every
candidate is checked against a rollback checkpoint. Only one selected
transaction is committed. An ambiguity commits only poisoned argument recovery,
not candidate effects or side effects.

### 8.3 Target facts are checker output

The checker records the primary/equivalent/considered IDs, instantiated schema,
per-argument mapping/inferred/expected types, return, effects, function-value
type, group, poison, diagnostics, and source identity. Signature help reads this
single fact. It cannot run a “lighter” resolver that diverges from checking.

## 9. Presentation/dialogue state reconciliation

The resolver and schema constructors are pure. Presentation state such as
`active_presentation_defaults` is modified only in candidate commit. Dialogue
mark/content/line-plan state remains in its existing validator and is likewise
updated only once. Rejected overload checkpoints leave no state.

Dynamic owner acquisition is a checker judgment because it may depend on a
lexical expression. The resulting `ResolvedCharacterOwner` is copied into the
schema instantiation and facts. Registered character facts remain immutable.

## 10. Effects and higher-order reconciliation

Current effect behavior is distributed across function, Agent, presentation,
data-last, and higher-order paths. The target schema carries the declared
`EffectRow`; the validator stages family-specific effect edges. Function-value
facts carry the existing effect-callable identity. Candidate rollback includes
effect collector state. The selected candidate commits exactly once.

Curried/partial calls retain base candidate identity plus current/next group;
there is no new synthetic function declaration. A later group uses
`CurriedCallableId` only for transient target identity.

## 11. Failure domains

### 11.1 Accepted-world construction failures

The following reject the candidate world atomically:

- invalid adapter package ID or typed callable path;
- duplicate typed callable ID;
- same-rank provider collision;
- non-contiguous overloads;
- project declaration/path/source mismatch;
- invalid schema/source/doc coordinates;
- module/record/overload/parameter/group/build-work limits;
- arithmetic overflow.

The previous accepted `Arc`, generation, source registry, character facts,
callable catalog, and caches remain pointer-identical.

### 11.2 Query failures

Unknown/non-callable calls produce ordinary typed checker recovery. Ambiguity,
trait ambiguity, data-last ambiguity, corrupt catalog, cancellation, source/world
mismatch, and query limits produce typed outcomes. Cancelled, stale, corrupt,
or exhausted results are not published in signature-help cache.

## 12. Compatibility and deletion policy

There is no compatibility period with two successful paths. Each family cut:

1. adds its ID/schema/validator and parity tests;
2. routes checker dispatch through the shared resolver;
3. proves through a crate-owned typed test hook that the old branch has no
   successful callers;
4. deletes the old branch and obsolete map/API in the same cut.

Prohibited reconciliation techniques:

- retaining old lookup “just in case”;
- extension traits around Arcweft-owned enums;
- ad hoc string-to-ID helpers outside the owning inherent implementation;
- feature gates selecting old versus new resolution;
- deprecated wrappers;
- source scans as acceptance tests;
- parsing labels, aliases, comments, source text, or Rust display paths;
- synthesizing source `impl` publication;
- adding CSS/Takumi or removed-syntax diagnostics.

## 13. Final production shape

After the final cut:

- `RegisteredTypeCheckEnv` exposes one immutable callable catalog;
- project and adapter registration is fallible and atomic;
- free and selected calls enter one resolver;
- every successful result has one typed candidate ID;
- every argument is checked once in one committed transaction;
- signature help consumes checker facts;
- presentation/dialogue look expectations are structural;
- old callable map lookups and checker-local successful dispatch are gone;
- public and dependency evidence is tested through typed APIs and Cargo
  metadata, not implementation text.
