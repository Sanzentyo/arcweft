# Production reconciliation

## Baseline

```text
repository: Sanzentyo/arcweft
main: 27227bbc8e1d5c78d7b35c2865bad8fb6d00fca9
inspection: read-only GitHub connector plus supplied local contracts
production edits: none
```

## Preserved production substrate

The contract preserves the implemented CharacterDialogue Cut 1 domain, the
AW-AH-009.3.1 parser-owned ordinary call surface, the AW-AH-009.4.2 typed source
application contract, proof HIR IDs/snapshots/scopes, the unified callable
identity, revision-bound SourceSpan, and AW-AH-009.3.2 accepted-project
lifecycle.

Current `HirProjectModule::try_new` already verifies that a lowered module's
retained source identity exactly matches its supplied `SourceDocumentIdentity`.
Current `AcceptedProjectSnapshot::try_new` already validates one `Arc<HirProject>`
against the symbol world and accepted source registry and leaves the previous
accepted generation intact on failure. These owners are extended, not replaced
with parallel models.

## Current contradictions

### Package identity arrives too late

Current module lowering constructs HIR without `CallablePackageId`; package
identity first enters `HirProject::new`. Callable IDs are consequently finalized
at project assembly. The proof model requires package-aware module identity.
Line prefixes cannot be final or cache-safe while package qualification is
optional during lowering.

### Current line generation is speaker-derived and mutating

`dialogue_identity.rs` owns `DialogueSpeakerSlug`, strips `.say`, unwraps entity
spelling, recognizes narrator aliases, and extracts a final callee segment.
`lower_ids.rs` combines flow slug, that speaker string, scope strings, and a
mutable `HashMap<String, usize>` counter. This directly violates the frozen
identity rules.

### Current HIR errors cannot carry the required evidence

`HirLowerError` is `{ message: String, range: Option<TextRange> }`. It cannot
retain AW-CD-013/AW-CD-020 identity, two SourceSpans, cross-document evidence,
or deterministic related labels.

### Current project has no accepted line product

`HirProject` retains modules, sources, and callable signature sources. It has no
candidate owner, collision transaction, accepted immutable line inventory, or
lookup by source ExprId. LSP currently derives other accepted references during
`AcceptedProjectSnapshot` construction; doing that for lines would create the
forbidden LSP-only inventory.

### Current generated IDs can never prove project collision atomicity

Counters advance during module lowering and current code has no project-wide
namespace. An implementer could be tempted to skip an occupied generated ID or
reserve scratch IDs across failed builds. This contract forbids both.

## Selected reconciliation

```text
package-aware LoweringRequest
  -> immutable HirModule snapshot
       + bounded unaccepted HirDialogueLineCandidates
  -> one HirProjectBuilder BTreeMap
  -> one project-wide line acceptance transaction
  -> immutable HirProject + AcceptedDialogueLineInventory
  -> existing AcceptedProjectSnapshot / accepted generation
```

Module lowering owns source correctness and candidate validity. Project
construction alone owns namespace collision acceptance. Accepted lifecycle
alone owns publication. Sema/tooling/runtime consumers borrow the resulting
project; none rebuild it.

## Concrete lower-owner correction

The exact 256-byte rule is currently exposed inside
`arcweft-dialogue::CharacterDialogueLimits`, but HIR cannot depend upward on
`arcweft-dialogue`. The clean dependency-safe correction is to place durable
`DialogueLineId`, `DialogueTextKey`, and the shared constant in the existing
lower `arcweft-id` crate. Cut 1 keeps the same field and value. This is a
concrete dependency-owner defect, not a redesign of runtime behavior.

## Rejected alternatives

| Alternative | Rejection |
|---|---|
| Materialize line IDs only in sema | HIR/source rename and project collision facts would be absent or duplicated. |
| Build an LSP line inventory | Violates the single accepted `Arc<HirProject>` lifecycle. |
| Keep speaker slug until runtime migration | Creates a successful compatibility identity path and lets Character rename change IDs. |
| Let generated IDs probe for a free value | Violates the frozen no-skip rule and makes source order/history observable. |
| Add package later during project assembly | Module candidate/cache identity would be incomplete and callable prefix construction would be duplicated. |
| Put `CharacterDialogueLimits` dependency in HIR | Reverses the required language/runtime dependency direction. |
| Make text keys globally unique | Prevents intentional localization-key sharing and is not required by the governing contract. |
| Accept relative `text_key` | Requires new unfrozen owner-relative semantics and would preserve the old speaker-derived path. |
| Store a dotted callable owner string | Duplicates `CallableDeclarationId` and weakens family/path typing. |
| Wrap `HirLowerError` with more text | Still loses structured code, identity, and cross-document SourceSpans. |

## No production claim

No Rust, Cargo, schema, fixture, source, generated artifact, or repository
document was changed. Proposed shapes have not been compiled. Implementation
validation is specified in `IMPLEMENTATION_HANDOFF.md`.
