# Line-ID candidate builder

## 1. Durable ID types

The lower identity crate owns:

```rust
pub const MAX_DIALOGUE_ID_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueLineId(PublicId);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DialogueTextKey(TextKey);
```

Checked constructors enforce:

- exact family and a nonempty tail (`say.` or `text.`);
- generic `PublicId`/`TextKey` validation;
- inclusive byte length 256, measured on UTF-8 bytes; and
- no normalization, case folding, alias expansion, or segment decoding.

They expose `as_public_id`/`as_text_key`, `as_str`, and owned extraction.
Neither derives Serde. Data-format owners must decode a string through the
checked constructor.

## 2. Candidate shapes

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineIdOrigin {
    ExplicitAbsolute,
    ExplicitRelative,
    ExplicitFamilyRelative,
    Generated,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueTextKeyOrigin {
    Explicit,
    Derived,
}

pub(crate) struct HirDialogueLineCandidate {
    id: DialogueLineId,
    id_origin: DialogueLineIdOrigin,
    text_key: DialogueTextKey,
    text_key_origin: DialogueTextKeyOrigin,
    site: HirDialogueLineSourceSite,
}
```

All fields are private. Construction is possible only through
`HirDialogueLineCandidateBuilder`.

## 3. Builder context

```rust
pub(crate) struct HirDialogueLineCandidateBuilder<'a> {
    module: &'a HirModuleKey,
    generated: BTreeMap<DialogueLinePrefix, DialogueGeneratedOrdinal>,
    candidates: Vec<HirDialogueLineCandidate>,
    diagnostics: Vec<DialogueLineDiagnostic>,
    source_order: u32,
    work: u32,
}
```

`DialogueLinePrefix` is an owned checked internal value constructed from typed
owners/scopes. It is not a public dotted-string wrapper. The type owns append
and byte-count behavior.

The builder consumes source-backed application roots in deterministic HIR
traversal order. Each call is a small transaction:

1. validate source/owner/scopes/component spans;
2. increment the source-order coordinate with checked arithmetic;
3. classify immediate `id` and `text_key` coordinates from AW-AH-009.4.2 typed
   HIR facts;
4. resolve or tentatively generate the line ID;
5. validate/derive the text key;
6. charge work and candidate limits; and
7. only now commit a generated ordinal and append the candidate.

Any user error appends one structured diagnostic, marks the module
non-executable, and appends no candidate. Any fatal invariant/budget failure
aborts module lowering and publishes no module snapshot.

## 4. Immediate coordinates

Only the immediate outer application coordinates selected by AW-AH-009.4.2 are
read. Transparent grouping may be followed exactly as that contract permits.

### `id`

| Typed value | Result |
|---|---|
| absent, owned | generated candidate |
| absent, ownerless | AW-CD-021 |
| one absolute `HirIdRef` | validate/preserve `@say.*` |
| one relative `HirIdRef` | resolve below owner prefix |
| one `say` family-relative `HirIdRef` | same as relative |
| another family | AW-CD-013 |
| runtime/non-ID expression | AW-CD-023 |
| duplicate coordinates | AW-CD-027 |
| recovered/error value | no candidate; existing poison diagnostic remains |

### `text_key`

| Typed value | Result |
|---|---|
| absent | derive from resolved line ID |
| one absolute `HirIdRef` in `text` family | validate/preserve |
| relative or family-relative | AW-CD-024 |
| another absolute family | AW-CD-024 |
| runtime/non-ID expression | AW-CD-023 |
| duplicate coordinates | AW-CD-027 |
| recovered/error value | no candidate |

The old relative text-key path is deliberately removed because it inserted a
speaker segment and no final owner-relative text-key contract exists.

## 5. Relative resolution

Given `scopes = [s0, ..., sn]` and `parent_depth = d`:

```text
if d > scopes.len: AW-CD-022
remaining = scopes[..scopes.len - d]
resolved = owner_prefix(remaining) + "." + relative.suffix
```

The suffix must be nonempty and pass the final `DialogueLineId` constructor.
Parent traversal cannot remove any owner component.

`@.greeting` and `@say:.greeting` carry different source variants but produce
the same final ID and retain different `DialogueLineIdOrigin` values.

## 6. Generated ordinals

`DialogueGeneratedOrdinal(u32)` owns:

```rust
fn peek_next(current: Option<Self>) -> Result<Self, DialogueLineBuildFatal>;
fn format(self) -> String;
```

- first value: 1;
- maximum: 262,144;
- minimum width: 3;
- formatting: zero-padded ASCII decimal;
- no wrap, saturation, reuse, or probing.

The builder does not mutate the map until line ID and text key both validate.
Thus a derived text key that exceeds 256 bytes also leaves the ordinal
unconsumed.

## 7. Ownerless behavior

`Ownerless + absolute @say.*` constructs the exact candidate and derives or
validates its text key. The application span is the source evidence.

Every other ownerless identity request is AW-CD-021. The builder never creates
an implicit module/package prefix.

## 8. Errors and atomicity

User diagnostics are the typed enum in `DIAGNOSTIC_MODEL.md`. Fatal builder
errors include source mismatch, stale HIR ID, checked arithmetic overflow,
invalid internal prefix construction, candidate/diagnostic/work exhaustion,
and impossible source-component containment.

A fatal error returns no `HirDialogueLineCandidates`. A user diagnostic returns
a complete HIR tooling snapshot marked non-executable and no candidate for the
failed site.

## 9. Module product and fatal API

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HirDialogueLineCandidates {
    module: HirModuleKey,
    records: Arc<[HirDialogueLineCandidate]>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirDialogueLineBuildFatal {
    SourceIdentityMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
    },
    StaleHirId { error: IdResolveError },
    ArithmeticOverflow { operation: DialogueLineBuildOperation },
    CandidateLimit { observed: usize, maximum: usize },
    DiagnosticLimit { observed: usize, maximum: usize },
    WorkLimit { observed: u32, maximum: u32 },
    InvalidInternalPrefix,
    InvalidSourceComponent,
}
```

`HirDialogueLineCandidates` construction is crate-private and occurs only as
part of a successful module transaction. `records()` is crate-private because
only project construction accepts them; public tooling reaches accepted facts
through `HirProject` or recovered source diagnostics through the HIR snapshot.

Fatal variants expose typed accessors and an inherent diagnostic projection;
callers do not map them through free-standing string helpers.

## 10. Candidate accessors and derives

`HirDialogueLineCandidate` derives `Clone, Debug, Eq, PartialEq`. It exposes
crate-private `id()`, `text_key()`, `id_origin()`, `text_key_origin()`, and
`site()` accessors. It deliberately does not implement `Hash`/`Ord`; canonical
order is the explicit source key, not incidental struct-field order.

`DialogueLineIdOrigin` and `DialogueTextKeyOrigin` derive
`Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd`. Their stable labels
are inherent methods on the owning enums.
