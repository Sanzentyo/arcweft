# Final contract

## 1. Normative outcome

This contract selects `READY_FOR_IMPLEMENTATION`.

A current callable surface can expose a structural character nominal parameter:

- `show(character, look=...)` obtains the character owner from its required
  first positional argument and exposes `TypeKind::CharacterNominal` for
  `look`;
- colon-style speaker lines and canonical content calls obtain the character
  owner from their semantically resolved speaker and expose the same structural
  `look` type; and
- any accepted project, standard, adapter, method, constructor, or function
  value signature can expose an already-typed `CharacterNominalType` parameter.

The implementation is a generic native signature-help feature with typed
character support. It is not a synthetic character constructor and is not a
character-only text recognizer.

The words **must**, **must not**, **should**, and **may** are normative in this
archive.

## 2. Ownership and dependency direction

The public semantic query is owned by:

```text
arcweft-lang-sema::signature
```

The single internal resolver used by both the checker and the query is owned by:

```text
arcweft-lang-sema::call_resolution
```

Exact call/argument syntax ranges are owned by:

```text
arcweft-lang-syntax::call
```

LSP position conversion, accepted-environment acquisition, cache publication,
and conversion to `lsp_types::SignatureHelp` remain owned by:

```text
arcweft-lsp::positions
arcweft-lsp::profiles::cache
arcweft-lsp::features::signature
arcweft-lsp::session
```

The dependency direction remains:

```text
arcweft-lang-syntax -> arcweft-lang-hir -> arcweft-lang-sema -> arcweft-lsp
                                              ^
                                              |
                                  arcweft-adapter-context
```

`arcweft-verify-lsp` ceases to resolve signature help. No lower crate depends on
LSP types, accepted-environment generations, document versions, or adapter
manifests.

## 3. Exact syntax/range carrier

### 3.1 Generic argument-list model

`arcweft-lang-syntax::call` adds these public, parser-constructed types:

```rust
use crate::ast::common::TextRange;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallExpressionSyntax {
    full: TextRange,
    callee: TextRange,
    arguments: ArgumentListSyntax,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentListSyntax {
    open: TextRange,
    content: TextRange,
    close: Option<TextRange>,
    arguments: Vec<ArgumentSyntax>,
    separators: Vec<TextRange>,
    recovery: ArgumentListRecovery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentSyntax {
    full: TextRange,
    name: Option<TextRange>,
    value: TextRange,
    form: ArgumentSyntaxForm,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentSyntaxForm {
    Positional,
    Named,
    Spread,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentListRecovery {
    Complete,
    MissingCloseDelimiter { recovery_end: usize },
    Recovered { recovery_end: usize, nodes: u16 },
}
```

All fields remain private. Public accessors expose the exact ranges and slices.
Only parser modules receive `pub(crate)` constructors. Constructors validate:

- `full`, `callee`, `open`, `content`, every argument, every separator, and an
  optional `close` are ordered, in bounds, and nested consistently;
- `arguments.len()` equals the semantic `CallArg` count;
- `separators.len()` is either `arguments.len() - 1` or `arguments.len()` when a
  trailing comma is present;
- named arguments have a non-empty name range inside `full`;
- positional and spread arguments have no name range;
- `recovery_end` is a UTF-8 boundary at or after `content.start()` and no later
  than the owning parsed construct's exact boundary.

This is a general parser boundary, not a signature-specific node identity.

### 3.2 Existing owned types are extended directly

The parser changes the Arcweft-owned call variant directly:

```rust
Expr::Call {
    callee: Box<Expr>,
    args: Vec<CallArg>,
    syntax: CallExpressionSyntax,
}
```

It does not add an extension trait, source-search helper, parallel call AST, or
compatibility accessor. All existing exhaustive matches migrate in the same
cut.

`SpeakerLineSurface` and `ContentCall` gain an optional
`ArgumentListSyntax`. Their inherent accessors return it. Colon-style lines or
content calls without parentheses return `None`; signature help is then not
applicable at the colon or content bracket.

### 3.3 Recovery boundary

When an opening `(` has been accepted and the callee plus an argument prefix is
structurally typed, a missing `)` still produces the typed call node. The
parser records `MissingCloseDelimiter` and uses the owning expression,
statement, speaker head, or content-call head boundary as `recovery_end`.
That boundary comes from parser control flow and token nesting. It is not found
by scanning source text after parsing.

A malformed argument that can be isolated produces `Recovered` and retains the
exact recovered token range. A malformed construct that cannot preserve a
callee and argument-list boundary remains ordinary parser recovery and cannot
produce signature help.

## 4. HIR source binding

`HirModule` directly gains these inherent public accessors and state:

```rust
impl HirModule {
    pub fn source_identity(&self) -> Option<&SourceDocumentIdentity>;
    pub fn module_path(&self) -> &CanonicalModulePath;
}
```

`lower_document_to_hir` remains the only document-bound lowering entry point.
It verifies exact text equality, binds `SourceDocumentIdentity`, stores the
canonical module path (`crate` when the source declaration omits one), and
preserves every call range by cloning the typed syntax nodes into HIR-owned
expressions and dialogue records.

`ProjectSymbolTable` directly gains:

```rust
impl ProjectSymbolTable {
    pub fn source_identity(
        &self,
        module: &CanonicalModulePath,
    ) -> Option<&SourceDocumentIdentity>;
}
```

The table records one identity for every project module, including modules with
no declarations. This is populated when the table is built from `HirProject`;
it is not reconstructed by the query.

## 5. Public sema query API

### 5.1 Request and entry point

`arcweft-lang-sema::signature` exposes exactly one entry point:

```rust
pub fn query_signature(
    request: SignatureQuery<'_>,
) -> Result<SignatureQueryOutcome, SignatureQueryError>;
```

The request is non-serializable and has private fields:

```rust
pub struct SignatureQuery<'a> {
    world: &'a RegisteredSemanticWorld,
    document: &'a SourceDocument,
    hir: &'a HirModule,
    byte_offset: usize,
    limits: SignatureQueryLimits,
    control: SignatureQueryControl<'a>,
}

impl<'a> SignatureQuery<'a> {
    pub fn try_new(
        world: &'a RegisteredSemanticWorld,
        document: &'a SourceDocument,
        hir: &'a HirModule,
        byte_offset: usize,
        limits: SignatureQueryLimits,
        control: SignatureQueryControl<'a>,
    ) -> Result<Self, SignatureQueryError>;
}
```

`try_new` verifies the document/HIR/module/world identities before any semantic
work. The byte offset must be at most `document.text().len()` and must be a
UTF-8 character boundary.

Control is explicit and borrowed:

```rust
pub struct SignatureQueryControl<'a> {
    cancelled: &'a std::sync::atomic::AtomicBool,
    deadline: Option<std::time::Instant>,
}

impl<'a> SignatureQueryControl<'a> {
    pub const fn new(
        cancelled: &'a std::sync::atomic::AtomicBool,
        deadline: Option<std::time::Instant>,
    ) -> Self;
}
```

The query checks cancellation, deadline, and work before every resolver-family
probe and every loop that can exceed a production limit.

### 5.2 Result shape

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureQueryOutcome {
    Help(SemanticSignatureHelp),
    NotApplicable(SignatureNotApplicable),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SignatureIndex(u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SignatureParameterIndex(u32);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignatureHelp {
    source: SourceDocumentIdentity,
    call: SourceSpan,
    arguments: SourceSpan,
    signatures: Vec<SemanticSignature>,
    active_signature: Option<SignatureIndex>,
    active_parameter: Option<SignatureParameterIndex>,
    recovery: SignatureRecovery,
    diagnostics: Vec<SignatureDiagnostic>,
    omitted_diagnostics: u64,
    work: SignatureWork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignature {
    id: SignatureCandidateId,
    origin: SignatureOrigin,
    authored_callee: String,
    canonical_callee: String,
    parameters: Vec<SemanticSignatureParameter>,
    return_type: SignatureParameterType,
    active_parameter: Option<SignatureParameterIndex>,
    documentation: Option<SignatureDocumentation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticSignatureParameter {
    name: Option<String>,
    expectation: SignatureParameterType,
    kind: SignatureParameterKind,
    has_default: bool,
    documentation: Option<SignatureDocumentation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureParameterType {
    Known(TypeKind),
    Unconstrained,
    Unavailable(SignatureTypeUnavailable),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignatureParameterKind {
    PositionalOnly,
    PositionalOrNamed,
    NamedOnly,
    PositionalRest,
    OpenNamed,
}
```

All result fields have immutable public accessors. Constructors are
`pub(crate)` and preserve these invariants:

- `signatures` is non-empty;
- every active index is in range;
- top-level `active_parameter` is the selected signature's active parameter, or
  the single common active parameter across all candidates when no signature is
  selected;
- `SourceSpan` values have the same `SourceDocumentIdentity` as `source`;
- diagnostics are already sorted and bounded.

`Unconstrained` is used only where current semantics intentionally accept an
open value. It is not encoded as `TypeKind::Named("_")`. `Unavailable` records
a typed reason and is never treated as a type-compatible wildcard.

### 5.3 Candidate identity

Labels are never identity. The public candidate identity is:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SignatureCandidateId {
    Project(CallableDeclarationId),
    Environment(EnvironmentCallableId),
    Presentation(PresentationSignatureId),
    Dialogue(DialogueSignatureId),
    Builtin(BuiltinCallableId),
    EnumVariant(EnumVariantSignatureId),
    Fx(FxCallableSignatureId),
    AgentIntrinsic(AgentIntrinsicSignatureId),
    FunctionValue(FunctionValueSignatureId),
}
```

Environment identity is typed and registered with the callable:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentCallableOwner {
    Standard,
    Adapter(AdapterPackageId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum EnvironmentCallableId {
    Function {
        owner: EnvironmentCallableOwner,
        path: SymbolPath,
    },
    Method {
        owner: EnvironmentCallableOwner,
        receiver: TypeKind,
        path: SymbolPath,
    },
}
```

`AdapterPackageId` is a validated non-empty, control-free newtype constructed
from accepted manifest identity. It has no label parser. The environment stores
`EnvironmentCallableId`, signature, origin, and documentation atomically.

Presentation and dialogue identities are structural:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PresentationCallableId {
    View,
    Menu,
    Overlay,
    Background,
    Image,
    PlayerViewport,
    ShowCharacter,
    BackgroundRef,
    CharacterRef,
    ClearBackground,
    HideCharacter,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PresentationSignatureId {
    callable: PresentationCallableId,
    character: Option<CharacterId>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueCallableId {
    SpeakerLine,
    ContentCall,
    SpeakerPreset,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueOwnerId {
    Character(CharacterId),
    Project(ProjectSymbolTargetId),
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueSignatureId {
    callable: DialogueCallableId,
    owner: DialogueOwnerId,
}
```

`PresentationCallableId` and `DialogueCallableId` own inherent `resolve` and
`signature_schema` methods. The checker and signature resolver call those
methods; no feature-local match duplicates their behavior.

Builtin, enum-variant, FX, agent-intrinsic, and function-value IDs are opaque
sema types with private constructors and typed accessors. A function-value ID
contains its exact callee `SourceSpan`, its `TypeKind::Function`, and the
zero-based curried call-group index. None contains a formatted signature label.

## 6. Accepted world and callable facts

The query receives `&RegisteredSemanticWorld` obtained from the current
`AcceptedProfileEnvironment`. It never calls `LspProfile::typecheck_env()` and
never applies adapter manifests itself.

`RegisteredTypeCheckEnv` gains read-only inherent query methods for:

- project callable signatures keyed by `CallableDeclarationId`;
- standard and adapter free callables keyed by typed lookup path;
- standard and adapter methods keyed by receiver `TypeKind` and method path;
- function effects and curried parameter groups;
- character manifests, variants, world/revision, and character
  digest/revision.

The registered environment stores a non-empty ordered candidate set per lookup
key. Insertion rejects duplicate `EnvironmentCallableId` values and duplicate
same-rank authorities. Hash-map replacement is not permitted.

Project-source signature collection runs while `CharacterRegistrar` has the
accepted `HirProject`; it records `FunctionSignature`, parameter docs, callable
docs, declaration ID, and source spans in `RegisteredTypeCheckEnv`. This is part
of the same atomic `RegisteredSemanticWorld` publication. It is not a second
project index.

Adapter context normalizes Rust metadata into the same environment records.
The existing Rust display strings remain documentation/provenance only.

## 7. One resolver

### 7.1 Resolver product

`arcweft-lang-sema::call_resolution` owns an internal typed product:

```rust
pub(crate) enum ResolvedCallTarget {
    Candidates(Vec<ResolvedCallable>),
    FunctionValue(ResolvedFunctionValue),
    NotCallable,
    Unresolved,
}
```

The vector is non-empty and already deterministically ordered. `TypeChecker`
uses it for argument checking and result type. `query_signature` uses the same
value for signatures and active-argument mapping. The query may collect
position-specific facts from the checker, but it may not re-resolve a name.

### 7.2 Precedence

The shared resolver applies this exact precedence, matching current special-form
dispatch while closing the project/environment seam:

1. FX constructor;
2. enum-variant or `Result` constructor;
3. builtin call;
4. agent intrinsic;
5. presentation special form;
6. lexically resolved project callable;
7. accepted standard/adapter free callable, only when project lookup is
   unresolved;
8. selected call: project method when published, otherwise accepted
   standard/adapter method for the resolved receiver type;
9. dialogue speaker/preset call;
10. function value, including the current curried call group;
11. non-callable or unresolved.

A resolved non-callable project symbol blocks an environment fallback. A
project callable of the same authored name wins over an environment callable.
Special-form names remain reserved because the current checker already selects
them before ordinary functions.

Current source `impl` methods are not published as project method signatures.
They become applicable only after the ordinary project method catalog exists;
this contract does not synthesize them. Accepted standard/adapter methods are
applicable now.

### 7.3 Native/adapter same-name rule

Rust-adapter metadata is normalized before publication. There is no merge in
LSP and no call to `arcweft_verify_lsp::rust_adapter_signature_help`.

- project callable plus adapter callable: project wins by rule 6;
- no project callable plus one accepted environment candidate: that candidate
  succeeds;
- multiple valid overloads under one accepted lookup key: return the ordered
  candidate set and apply overload selection;
- two authorities with the same `EnvironmentCallableId` or two same-rank
  standard/adapter claims that cannot be distinguished structurally: accepted
  world construction fails; a defensive query observation returns
  `SignatureQueryError::AmbiguousAuthority` and publishes no cache entry.

## 8. Source and semantic fact acquisition

The selected call is located by traversing typed HIR call/dialogue nodes and
comparing parser-retained ranges. The query then runs the existing sema checker
in a target-fact mode that records the shared resolver product and local
function-value type for that exact `SourceSpan`. The checker is not forked and
the environment is not rebuilt.

Before traversal, the query verifies:

1. `hir.source_identity() == Some(document.identity())`;
2. `world.symbols().source_identity(hir.module_path()) ==
   Some(document.identity())`;
3. symbol-table and registered-environment world IDs are equal;
4. symbol-table and registered-environment symbol revisions are equal;
5. source length is within the production limit;
6. the byte offset is valid and on a UTF-8 boundary.

A mismatch is typed stale/unavailable failure. No source substring is reparsed,
no word is extracted, and no `SourceSnapshotId` is forged or derived.

## 9. Call selection

### 9.1 Containment

For a complete list, the cursor belongs to the list when:

```text
open.end <= byte_offset <= close.start
```

The opening delimiter itself is outside; the point immediately after it is
inside. `close.start` is inside for active-parameter calculation; `close.end`
and later points are outside.

For a recovered list, containment is:

```text
open.end <= byte_offset <= recovery_end
```

### 9.2 Nested precedence

All containing typed lists are collected. The selected list is ordered by:

1. greatest syntactic nesting depth;
2. shortest argument-content range;
3. greatest opening-delimiter start;
4. typed surface rank (`Expr::Call`, dialogue option list) only when one wrapper
   deliberately shares an identical list.

Two different parser nodes that remain tied after these keys are invalid typed
syntax and return `SemanticUnavailable::AmbiguousCallRange`. Traversal order is
never a tie-breaker.

### 9.3 Outside behavior

No containing argument list returns
`SignatureQueryOutcome::NotApplicable(CursorOutsideArgumentList)`. The LSP
result is `null` and no diagnostic is emitted.

## 10. Active argument and parameter

### 10.1 Argument slot

The active argument slot is determined only from `ArgumentListSyntax`:

- empty list, opening-delimiter trailing whitespace, or a point before the first
  argument: slot `0`;
- inside an argument's full/name/value range: that argument's ordinal;
- whitespace after an argument and before its following comma: that argument;
- on a comma, or whitespace after that comma and before the next argument: the
  following slot;
- after a trailing comma through `close.start` or `recovery_end`: one past the
  last argument;
- after the final argument without a trailing comma through `close.start`: the
  final argument;
- for zero-width recovered argument ranges: the range's ordinal, with the comma
  rule taking precedence at a shared boundary.

All boundaries are UTF-8 byte ranges. LSP conversion occurs before the query.

### 10.2 Parameter binding

For each candidate independently:

1. Named arguments bind the parameter with the exact semantic name.
2. Reordered named arguments remain valid.
3. A repeated name maps to the same parameter and emits
   `DuplicateNamedArgument` at the later occurrence.
4. Positional arguments bind the next unbound parameter whose kind is
   `PositionalOnly` or `PositionalOrNamed`.
5. Once fixed positional parameters are consumed, additional positional
   arguments bind `PositionalRest` when present.
6. An unknown named argument binds `OpenNamed` when present; otherwise it emits
   `UnknownNamedArgument` and has no active parameter.
7. A spread binds `PositionalRest` when present. Without one it emits
   `UnsupportedSpread`, has no active parameter, and prevents deterministic
   positional binding of later arguments for that candidate.
8. A positional argument targeting a parameter already bound by name emits
   `ParameterAlreadyBound` and advances to the next eligible parameter.
9. A slot one past the last argument maps to the next unbound required or
   optional parameter, then `PositionalRest`, then no parameter.

Partial and curried calls use `FunctionSignature::remaining_param_group` for the
current call group. Function values without authored parameter names expose
`arg1`, `arg2`, and so on; those generated names are display-only and positional
arguments remain the only bindable form.

### 10.3 Overload selection

Every resolved candidate receives an argument-binding score:

```text
exact known type match        4
compatible known type match   3
unconstrained expectation     2
unavailable expectation       1
mismatch                      0
```

The candidate is viable only when it has no missing required fixed parameter,
extra argument, unsupported spread, unknown named argument, or known type
mismatch. Among viable candidates, a candidate is strictly more specific when
all per-argument scores are at least the other candidate's scores and at least
one is greater; fixed parameters also outrank rest/open parameters for the same
argument.

- one viable candidate: it is active;
- one unique strictly most-specific viable candidate: it is active;
- multiple incomparable viable candidates: no active signature and an
  `AmbiguousOverload` diagnostic;
- no viable candidate: no active signature and a `NoViableSignature`
  diagnostic; all bounded resolved candidates remain visible.

The top-level active parameter is the active candidate's value. When no
candidate is active, it is present only when every candidate has the same active
parameter index.

## 11. Character nominal expectations

### 11.1 Presentation `show`

The first positional `character` argument is resolved through
`ProjectSymbolTable` and `RegisteredTypeCheckEnv` to a canonical `CharacterId`.
The `look` parameter expectation is:

```rust
SignatureParameterType::Known(TypeKind::CharacterNominal(
    CharacterNominalType::look(character_id)
))
```

The checker invokes `check_expr_with_expected` with that exact `TypeKind`. The
signature query obtains the same value from the shared presentation schema.

If the character argument is absent or cannot resolve to one registered
character, the query still returns the `show` signature, sets the look
expectation to `Unavailable(UnknownCharacterOwner)`, and emits a structured
`UnknownCharacterOwner` diagnostic. It never selects a global look by spelling.

### 11.2 Dialogue

For `SpeakerLine`, `ContentCall`, and a speaker/preset call with an argument
list, the callee is resolved first. A character owner yields the same
`CharacterNominalType::look` expectation for named `look`. A non-character
speaker preset exposes its registered typed option schema; it does not receive a
fabricated character type.

### 11.3 General accepted callables

A project, standard, adapter, method, constructor, overload, or function-value
parameter carrying `TypeKind::CharacterNominal(nominal)` is returned unchanged.
`nominal.family()`, `nominal.character()`, and `nominal.part()` remain the only
scope. Equal local spellings across characters, families, or parts never
coalesce.

An unknown registered part yields
`Unavailable(UnknownCharacterPart { nominal })` plus a diagnostic. It is not
rewritten to a look, a family-wide type, or a `Named` string.

### 11.4 Alias policy

The signature label preserves the authored callee or character alias exactly
because that text comes from the parser-owned callee range. The parameter type
label is always `CharacterNominalType::source_label()` for the canonical typed
identity. Documentation adds `Canonical owner: ...` when authored spelling and
canonical owner differ.

Aliases never affect equality, hashing, overload selection, cache identity, or
resolution. `Display` and `source_label()` have no inverse parser.

## 12. Presentation labels and documentation

### 12.1 Signature label

The LSP adapter builds:

```text
<authored-callee>(<parameter-labels>) -> <return-type>
```

Parameter labels are:

```text
name: Type
argN: Type                 # unnamed fixed parameter
...name: Type              # positional rest
<named>: _                 # open named parameters
```

` = _` is appended for an authored/defaulted parameter. `?` is used for an
unavailable type and `_` for an intentionally unconstrained type. Known types
use their canonical sema source-label method; the formatter must add inherent
label behavior to `TypeKind`/owned semantic types where missing rather than a
feature-local exhaustive helper.

The return type follows the same type-label policy. Label text is never fed back
into resolution.

### 12.2 ParameterInformation ranges

The label is built incrementally. For each parameter, the adapter records the
inclusive start and exclusive end offsets in **UTF-16 code units**, independent
of the negotiated document position encoding. Every addition uses checked
`u32` conversion. Overflow is a typed arithmetic error and publishes no cache
entry.

### 12.3 Documentation priority

For the selected candidate, documentation priority is:

1. project source `DocBlock` and parameter `DocBlock`;
2. accepted adapter tooling documentation;
3. accepted Rust export path/provenance text;
4. static sema-owned presentation/dialogue documentation;
5. absent.

Documentation is `MarkupContent` only after LSP conversion. Sema stores:

```rust
pub enum SignatureDocumentationKind {
    Markdown,
    PlainText,
}

pub struct SignatureDocumentation {
    kind: SignatureDocumentationKind,
    value: String,
}
```

Empty documentation normalizes to `None`.

### 12.4 Ordering and coalescing

Candidate order is deterministic:

1. resolver precedence rank;
2. origin rank: project source, standard environment, adapter environment,
   special form, function value;
3. typed `SignatureCandidateId` order;
4. declaration/source span;
5. parameter type vector and return type.

Exact duplicates are coalesced only when candidate ID, parameter kinds, typed
expectations, defaults, return type, and documentation provenance are all equal.
Identical labels with different IDs or types remain separate.

Incomplete source retains a signature when callee and list boundaries are
structural. Unavailable parameter types render as `?` and carry diagnostics;
missing documentation remains absent.

## 13. Recovery and typed outcomes

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureRecovery {
    Complete,
    MissingCloseDelimiter,
    Recovered { nodes: u16 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureNotApplicable {
    CursorOutsideArgumentList,
    UnknownCallee,
    NonCallableCallee,
    UnsupportedSurface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SignatureTypeUnavailable {
    UnknownCharacterOwner,
    UnknownCharacterPart { nominal: CharacterNominalType },
    MissingRegisteredMetadata,
    PoisonedExpression,
}
```

Structured diagnostics include:

```rust
pub enum SignatureDiagnosticKind {
    AmbiguousOverload,
    NoViableSignature,
    DuplicateNamedArgument { name: String },
    ParameterAlreadyBound { parameter: SignatureParameterIndex },
    UnsupportedSpread,
    MissingCloseDelimiter,
    RecoveredArgument,
    UnknownCharacterOwner,
    UnknownCharacterPart { nominal: CharacterNominalType },
    UnknownNamedArgument { name: String },
    ExtraArgument,
    MissingRequiredParameter { parameter: SignatureParameterIndex },
    ParameterTypeUnavailable { reason: SignatureTypeUnavailable },
    DiagnosticsTruncated,
}
```

Each diagnostic contains a primary `SourceSpan`, ordered related spans, and its
kind. Human-readable messages are rendered later from the kind; message text is
not error identity.

Behavior is fixed:

| Condition | Sema outcome | LSP outcome |
| --- | --- | --- |
| cursor outside a typed argument list | `NotApplicable` | `null` |
| unknown or non-callable callee | `NotApplicable` | `null` |
| unsupported surface such as `goto` or a dialogue tag | `NotApplicable` | `null` |
| ambiguous overload | partial `Help` + diagnostic | signature list, no active signature |
| missing close delimiter | partial `Help` + diagnostic | signature list |
| duplicate named argument | partial `Help` + diagnostic | signature list |
| unsupported spread | partial `Help` + diagnostic | signature list |
| unknown character owner/part | partial `Help`, unavailable type + diagnostic | signature list with `?` |
| poisoned syntax with exact call boundary | partial `Help` when target resolves; otherwise typed semantic-unavailable error | result or request error |
| stale document/world/revision/generation/profile | typed stale error | `ContentModified` |
| invalid LSP position | typed position error | `InvalidParams` |
| no accepted semantic world | request-layer unavailable error | `RequestFailed` |
| client cancellation | `Cancelled` | `RequestCancelled` |
| deadline/work/resource exhaustion | typed failure | `ServerCancelled` |
| arithmetic overflow or corrupt same-rank authority | typed failure | `RequestFailed` |

## 14. Query errors

Sema errors are exact enums:

```rust
#[derive(Clone, Debug, thiserror::Error, PartialEq)]
pub enum SignatureQueryError {
    #[error(transparent)]
    Stale(#[from] SignatureSemanticStale),
    #[error(transparent)]
    InvalidPosition(#[from] SignaturePositionError),
    #[error(transparent)]
    SemanticUnavailable(#[from] SignatureSemanticUnavailable),
    #[error(transparent)]
    LimitExceeded(#[from] SignatureLimitExceeded),
    #[error("signature query counter overflowed")]
    ArithmeticOverflow { counter: SignatureWorkKind },
    #[error("ambiguous callable authorities")]
    AmbiguousAuthority { candidates: Vec<SignatureCandidateId> },
    #[error("signature query was cancelled")]
    Cancelled,
    #[error("signature query deadline elapsed")]
    DeadlineExceeded,
}
```

`SignatureSemanticStale` has separate variants for HIR/document identity,
project-module identity, symbol world, and symbol revision, each retaining the
expected and actual typed values. `SignaturePositionError` has `OutOfBounds`
and `NotUtf8Boundary`. `SignatureSemanticUnavailable` has
`MissingSourceIdentity`, `MissingProjectModule`, `AmbiguousCallRange`,
`MissingCallableFacts`, and `PoisonedCallBoundary`.

The LSP layer wraps sema failures with request-stamp failures:

```rust
pub(crate) enum SignatureRequestError {
    Query(SignatureQueryError),
    NoAcceptedEnvironment,
    Stale(SignatureRequestStale),
    InvalidLspPosition(CheckedPositionError),
}

pub(crate) enum SignatureRequestStale {
    Profile,
    AcceptedGeneration,
    DocumentIdentity,
    DocumentVersion,
    SymbolWorld,
    SymbolRevision,
    CharacterRevision,
    CharacterDigest,
}
```

The session maps these variants to the LSP codes in the table above and places
structured variant data in `ResponseError.data`. It does not use a formatted
string to choose a code.

## 15. LSP position conversion

`LineIndex` gains an inherent checked method without changing existing feature
behavior:

```rust
pub fn try_byte_offset_from_position(
    &self,
    position: lsp_types::Position,
) -> Result<usize, CheckedPositionError>;
```

`CheckedPositionError` variants are `LineOutOfBounds`, `CharacterOutOfBounds`,
`SplitUtf8Scalar`, `SplitUtf16Scalar`, and `ArithmeticOverflow`. The method does
not clamp. It honors the negotiated UTF-8 or UTF-16 document position encoding
and returns an exact UTF-8 byte offset.

## 16. Explicit non-goals

This implementation does not:

- add a constructor solely to expose character nominal types;
- add a `move` presentation special form absent from the current checker;
- treat inline dialogue tags as ordinary calls;
- publish current source `impl` methods before a canonical project method
  catalog exists;
- redesign completion, hover, definition, rename, Character registration,
  source identity, incremental syntax, or proof typed-node identity;
- parse `source_label()`, `Display`, aliases, signature labels, comments, or
  source substrings;
- create a signature-specific node ID, syntax database, persisted result
  format, compatibility shim, extension trait, or second successful resolver;
- cache errors, cancelled/expired work, stale results, or truncated candidate
  sets.
