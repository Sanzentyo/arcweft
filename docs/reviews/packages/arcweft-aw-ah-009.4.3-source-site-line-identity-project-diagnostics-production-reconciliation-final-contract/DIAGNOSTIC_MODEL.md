# Diagnostic model

## 1. Domain owner

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DialogueLineDiagnosticCode {
    InvalidLineIdFamily,              // AW-CD-013
    LineIdCollision,                  // AW-CD-020
    MissingLineSourceOwner,           // AW-CD-021
    RelativeLineIdEscapesOwner,       // AW-CD-022
    InvalidLineIdentityCoordinate,    // AW-CD-023
    InvalidTextKeyFamily,             // AW-CD-024
    DialogueLineIdentityLimit,        // AW-CD-025
    DialogueLineSourceMismatch,       // AW-CD-026
    DuplicateLineIdentityCoordinate,  // AW-CD-027
    InvalidDialogueLineIdentity,      // AW-CD-028
}
```

`as_str()` is an inherent exhaustive method on this enum. Callers do not repeat
matches or hard-code codes.

## 2. Structured variants

```rust
pub enum DialogueLineDiagnostic {
    InvalidLineIdFamily {
        found: String,
        span: SourceSpan,
    },
    LineIdCollision {
        id: DialogueLineId,
        first: DialogueLineCollisionSite,
        conflicting: DialogueLineCollisionSite,
    },
    MissingLineSourceOwner {
        application: SourceSpan,
        coordinate: Option<SourceSpan>,
        request: OwnerlessLineRequestKind,
    },
    RelativeLineIdEscapesOwner {
        requested: u16,
        available: u16,
        span: SourceSpan,
    },
    InvalidLineIdentityCoordinate {
        coordinate: DialogueIdentityCoordinateKind,
        reason: InvalidCoordinateReason,
        span: SourceSpan,
    },
    InvalidTextKeyFamily {
        found: Option<String>,
        span: SourceSpan,
    },
    DialogueLineIdentityLimit {
        kind: DialogueLineLimitKind,
        observed: u64,
        maximum: u64,
        span: Option<SourceSpan>,
    },
    DialogueLineSourceMismatch {
        expected: SourceDocumentIdentity,
        actual: SourceDocumentIdentity,
        span: Option<SourceSpan>,
    },
    DuplicateLineIdentityCoordinate {
        coordinate: DialogueIdentityCoordinateKind,
        first: SourceSpan,
        duplicate: SourceSpan,
    },
    InvalidDialogueLineIdentity {
        coordinate: DialogueIdentityCoordinateKind,
        reason: DialogueIdentityErrorKind,
        span: SourceSpan,
    },
}
```

All auxiliary enums are closed, typed, and expose inherent stable labels.
`DialogueLineCollisionSite` retains `HirModuleKey`, `ExprId`, source order, and
the exact source evidence span. It has no display-name field.

## 3. Stage ownership

| Failure | Owning stage | Publication result |
|---|---|---|
| wrong line family, missing owner, scope escape, dynamic/duplicate coordinate, invalid/oversized ID/key | module candidate construction | recovered tooling HIR with structured diagnostic; module non-executable; no candidate |
| stale source/snapshot, checked arithmetic, module candidate/work/diagnostic exhaustion | fatal module lowering | no HIR snapshot |
| AW-CD-020 collision | `HirProjectBuilder::finish` | typed project rejection; no `HirProject` |
| project candidate/work/diagnostic exhaustion | fatal project construction | no project and incomplete collision set not published |
| sema type error unrelated to identity | sema | existing sema diagnostic; line identity is not reconstructed |

Current `HirLowerError` may remain for unrelated provisional HIR paths during
the staged migration, but no final line diagnostic is converted to it. The
final proof-HIR migration should replace it generally rather than wrapping line
errors in strings.

## 4. Source diagnostic projection

`DialogueLineDiagnostic::to_source_diagnostic()` creates one
`arcweft_source::Diagnostic`:

- severity `Error`;
- exact stable code;
- deterministic message generated from typed fields;
- one primary label when source evidence exists; and
- zero or one secondary label, except no line diagnostic needs more than two
  total labels.

### AW-CD-020

```text
message: dialogue line ID `@<id>` is produced by more than one source site
primary: later canonical site — "this site also produces `@<id>`"
secondary: first canonical site — "first site producing `@<id>`"
```

The diagnostic's top-level span is the primary label. Both labels retain full
`SourceDocumentIdentity` and byte range. LSP publishes the primary document and
projects a secondary-document label through related information. CLI, compiler,
Agent, and MCP receive the same source diagnostic product.

### AW-CD-013

Primary is the exact authored ID value span, not the whole application. The
message names the found family and expected `say` family.

### Derived-text-key limit

Primary is the application span. If an explicit line ID caused the derived
value, its coordinate span is a secondary explanatory label. No fake text-key
source span is constructed.

## 5. Ordering

Diagnostics sort by:

1. primary source document ID;
2. primary source revision identity;
3. primary start and end byte;
4. stable code string;
5. typed subject (`DialogueLineId`, coordinate, or limit kind);
6. secondary source ID/revision/range; and
7. complete typed variant ordering.

Deduplication requires complete equality. Two collisions with the same line ID
but different later sites are distinct.

## 6. Limits

Module and project diagnostic collections each have an inclusive maximum of
1,024, reusing `HirLimit::Diagnostics`. A one-over event is a fatal transaction
limit, not a silently dropped diagnostic.

A source diagnostic projection validates every SourceSpan against the accepted
source registry before rendering. Cross-document diagnostics are registry-
validated label by label; `Diagnostic::validate_source` is not misused as a
single-document validator for both labels.

## 7. No second transport

Compiler, CLI, LSP, Agent, and tooling consume `arcweft_source::Diagnostic`.
`DialogueLineDiagnostic` is domain identity, not a new protocol DTO. No JSON,
LSP, or CLI layer reparses the message to recover the code, line ID, or related
span.

## 8. Construction and inspection API

All enum fields are private outside the owning module through crate-private
variant constructors. The public inspection surface is:

```rust
impl DialogueLineDiagnostic {
    pub const fn code(&self) -> DialogueLineDiagnosticCode;
    pub fn primary_span(&self) -> Option<&SourceSpan>;
    pub fn related_spans(&self) -> impl ExactSizeIterator<Item = &SourceSpan>;
    pub fn line_id(&self) -> Option<&DialogueLineId>;
    pub fn to_source_diagnostic(&self) -> Diagnostic;
}
```

The owning module provides named inherent constructors such as
`invalid_line_id_family`, `collision`, `missing_owner`, and `source_mismatch`.
No caller constructs repeated message/code/label combinations manually.

`DialogueLineDiagnostic` derives `Clone, Debug, Eq, PartialEq`; deterministic
sorting uses an explicit `sort_key()` owned by the enum rather than deriving an
order whose field layout could become accidental API.
