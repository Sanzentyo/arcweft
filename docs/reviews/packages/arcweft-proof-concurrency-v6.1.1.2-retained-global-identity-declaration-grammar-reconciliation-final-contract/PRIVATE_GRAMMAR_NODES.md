# Private lossless grammar nodes

## 1. Final item inventory

The private one-pass grammar has these retained source item kinds:

```rust
CharacterDeclarationItem
ViewDeclarationItem
ActionDeclarationItem
ActivityDeclarationItem
SignalDeclarationItem
MetricDeclarationItem
LayerDeclarationItem
```

There is no `AssetDeclarationItem`. `ResourceDeclarationItem` remains separate and is not part of this seven-item list.

## 2. Required shared and family nodes

The final inventory uses the existing names below.

### Shared retained header and recovery

```text
DeclarationHeader
DeclarationPublicId
SurfaceAlias
NameDefinition
MissingName
WrongFamilyReference
MissingDeclarationId
RetainedReference
MissingMemberValue
ErrorDeclarationMember
ErrorNode
MissingTokenNode
```

Common typed descendants include `Visibility`, `DocBlock`, `OuterAttribute`, `FixedParameterGroup`, `Parameter`, common pattern nodes, common type nodes, common expression nodes, delimiter nodes, path/reference nodes, list nodes, and zero-width missing nodes.

### Character

```text
CharacterBody
CharacterDisplayNameMember
```

### View

```text
ViewDeclarationBody
ViewExportBlock
ViewExportDeclaration
ViewFragment
```

### Action

```text
ActionSignature
```

### Activity

```text
ActivityBody
ActivityModeMember
ActivityLifecycleMember
ActivityInputBlock
ActivityOutputBlock
ActivityPort
ActivityContractBlock
RequiresClause
EnsuresClause
```

### Signal

```text
SignalObservableType
```

### Metric

```text
MetricKind
MetricBody
MetricUnitMember
MetricLabelsBlock
MetricLabel
MetricBucketsMember
```

### Layer

```text
LayerKindNode
LayerBody
LayerMember
LayerPolicyValue
```

## 3. Semantic roles

The accepted `SyntaxRole` vocabulary is used directly. The binding roles for this slice are:

```text
Documentation
Attribute(n)
Visibility
PublicId
Alias
Kind
Name
ParameterGroup
Parameter(n)
ParameterPattern
ParameterType
Body
Initializer
Condition
Type
Member(n)
InputPort(n)
OutputPort(n)
RequiresClause(n)
EnsuresClause(n)
Export(n)
Label(n)
Bucket(n)
Policy(n)
Reference(n)
Element(n)
OpenDelimiter
CloseDelimiter
Recovery(n)
```

Ordinals are source order within the owning semantic list and are included in path-authoritative attachment. Role class strips only the ordinal for reconciliation candidate grouping; exact role plus event path still distinguishes siblings.

## 4. Identity class

Identity-bearing:

- each of the seven declaration items;
- `DeclarationHeader`, visibility, public ID, name, alias, every parameter, pattern, type, expression, path/reference, body, family member, export, port, contract clause, label, bucket expression, policy, delimiter, error node, and zero-width missing node;
- each recovery node whose presence is semantically observable by tooling or lowering.

Structural-only:

- root/list/logical-line/indent grouping nodes that exist only to preserve lossless hierarchy and ordering and do not represent an independently addressable typed construct.

Tokens do not receive syntax IDs. Token ranges are reached through their owning identity-bearing node. A token is never used as a substitute source-backed HIR key.

## 5. Parser ownership

| Module | Sole responsibility |
|---|---|
| `parser/declaration.rs` | outer prefixes, visibility, retained header, absolute/family ID validation, ordinary name, shared typed parameter/member utilities |
| `parser/character_grammar.rs` | Character alias/body/member grammar |
| `parser/view_grammar.rs` | fixed View signature, leading exports, one typed expression fragment |
| `parser/action_grammar.rs` | bodyless Action signature and trailing recovery |
| `parser/activity_grammar.rs` | abstract sections, ports, contracts, ordering/duplicates |
| `parser/signal_grammar.rs` | colon plus one typed observable type and no-policy tail |
| `parser/metric_grammar.rs` | closed kind, value type, body members, labels, buckets |
| `parser/layer_grammar.rs` | closed kind, typed singleton members, family-constrained references, policies |
| `parser/item.rs` / `parser/document.rs` | deterministic top-level classification and single dispatch transaction |
| common expression/type/pattern modules | all nested common typed children; family modules never duplicate these parsers |

No asset parser module is added.

## 6. Event/tree pipeline

```text
SourceDocument
  -> shared lexer
  -> ShadowDocumentParser + GrammarBudget
  -> SyntaxEvent stream (start/token/missing/diagnostic/finish)
  -> one lossless green tree + unattached typed index
  -> syntax database transaction and reconciliation
  -> immutable snapshot with exact SyntaxNodeId attachment
  -> public attached typed handles after the atomic switch
```

A family parser may only consume the shared document cursor and emit events. It must not invoke the legacy public parser, construct a detached public AST, retain source substrings, call a fragment parser on copied text, or allocate a public `SyntaxNodeId`.

## 7. Attachment invariants

- Attachment is event-path authoritative: exact parent, concrete kind, exact semantic role, and occurrence path select the Rowan node.
- Same-kind siblings on one physical line remain distinct through exact roles/ordinals and paths.
- Missing zero-width nodes at the same byte offset remain distinct through kind, role, and event path.
- Reconciled moves/copies follow the accepted syntax database rules. IDs are never forged, reused, or looked up by range.
- An `AstTag` or coarse family is navigation metadata only; construction of a concrete family node requires the concrete `SyntaxKind` predicate.
- Wrong database, lineage, snapshot, generation, or concrete kind returns a typed lookup/access error.

## 8. Losslessness

All original UTF-8 bytes, including LF/CRLF, whitespace, ordinary comments, docs, attributes, delimiters, malformed tokens, and trailing recovery, remain represented in the green tree. Typed nodes point into that tree; they do not normalize source spelling. Semantic normalization occurs only in owned typed values such as `PublicId`, closed policy enums, or decoded string literals, while source handles remain available for exact tooling display.
