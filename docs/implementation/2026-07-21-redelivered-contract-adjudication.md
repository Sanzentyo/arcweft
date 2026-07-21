# 2026-07-21 redelivered contract adjudication

## Purpose

Several returned packages describe the same sequence positions with materially
different contracts. This note records the implementation authority selected
for the active goal. The packages are not combined, and an older rule is not
kept as a compatibility path merely because it appeared in an earlier ZIP.

Selection uses the later same-request audit when it targets the newer `main`,
explicitly supersedes the earlier result, preserves typed ownership, and gives
deterministic acceptance criteria. A concrete crate-dependency cycle may move a
reusable boundary type downward without changing the selected semantic
contract.

## Selected authorities

### Lang-01.1.1.1

Selected:

`arcweft-lang-01.1.1.1-final-contract-c957a61e4a0b.zip`

SHA-256:

`024A13F98A7F46764A79CCBBD8F7ED317C30A4F5E24332E6AE1E2FF7B2A7E18C`

This package replaces the earlier implementation-ready ZIP. It owns one
shared `TryExprSource`, keeps authored operator spelling separate, uses
`SourceSpan` consistently, and adds only the four required type-check
diagnostics. References to removed `task fn`, `dialogue fn`, or `stream fn`
surface forms are interpreted through the already-selected ordinary-function
role and generator model; they do not authorize restoring those declarations.

The follow-up
`docs/reviews/requests/2026-07-20-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation.md`
has now been rewritten against this selected archive. It preserves
`TryExprSource`, `PropagationBoundaryEvidence`, and
`CheckedReturnType::{Known, Unconstrained}` and asks the returned contract to
define project-aware type-failure/poison evidence without restoring the older
`CheckedReturnTarget` model. It is independently throwable in that corrected
form.

### Lang-01.4.1

Selected:

`arcweft-lang-01.4.1-resource-reference-and-retained-identity-schema-contract-correction-final-contract-main-a8361377.zip`

SHA-256:

`D0839B2ECAFD7D77F033100C620FC1459966060AD18ADA3087A71307F48C8881`

This is the authority already recorded in
`2026-07-20-lang-01-4-1-retained-identity-wip.md`. Target, View, and containing
scroll-region references resolve independently. The older proposal's
cross-field owner inference, equality rule, and precedence rule are not
implemented. Existing `RetainedIdentityRef<K>`, the project symbol table, and
the external owner registry remain the single typed substrate.

### Lang-01.5.1.1

Selected:

`arcweft-lang-01.5.1.1-dialogue-profile-presentation-owner-contract-correction-final-contract(1).zip`

SHA-256:

`58BCC3A8B03414E7CCA2B08CDD3770517A22B7B09B568A1F34FA2DC34956D506`

This is the authority already recorded in
`2026-07-20-lang-01-5-1-1-dialogue-profile-presentation-owner-intake.md`.
Profile admission belongs to the compiler transaction that owns linked HIR and
the accepted View product. CLI source reparse, a second catalog, and a second
manifest decoder are prohibited. Existing `inline-failure`, nested fallback,
and `InlineFailurePolicy` wire names remain canonical.

The semantic revision tuple remains the selected six-field contract. Its
reusable `DialogueProfileRevision` type must live in a lower cycle-free crate
shared by compiler and runtime-plan; keeping that type physically inside the
compiler would introduce a dependency cycle. `CheckedDialogueProfile` and
admission logic remain compiler-owned. This is an ownership correction, not a
second model or compatibility shim.

## Completion boundary

This note decides which contracts drive implementation. It does not by itself
mark any sequence complete. Each selected package still requires its stated
production paths, negative tests, Tier 2 coverage where applicable, structural
audit, and deletion of superseded code before the active goal can close.
