# AW-AH-007/008 typed RichText validation public switch

- Date: 2026-08-09
- Git HEAD inspected: `80331c81e338d20e968a10947d5e848c39610384`
- Status: `COMPLETE_FOR_TYPED_VALIDATION_SLICE`

## Final authority

The private attached RichText grammar now lowers through the final HIR
dialogue-content inventory into one `arcweft-lang-sema::checked_rich_text`
authority. `FinalSemanticAnalysis` retains the checked report on the owning
dialogue-content application and rejects executable publication when the
report is invalid.

The checker consumes HIR-owned tag, argument, source-role, and paired-open
identities. It does not invoke the public dialogue parser, rescan a source
document, or publish a detached AST/side table. Domain inventories remain on
their owning dialogue/presentation enums and the neutral schema leaf carries
no duplicate spelling registry.

The checked result distinguishes:

- typed controls, markers, direct styles, style/layout/transform/object
  selectors, host events, and built-in Fx;
- authored versus absence-only defaulted fields;
- exact units, fixed values, ranges, enum variants, PublicIds, colors, vectors,
  durations, and seeds;
- invalid/recovered arguments, duplicates with related sites, unknown
  selectors/properties, conflicts, crossing spans, and unclosed spans; and
- exact close-to-open HIR identity rather than name-based stack recovery.

Malformed present input never selects a default or creates an executable open
action. Invalid selectors preserve their child text as typed invalid content
without inventing semantics.

## Deletion boundary

There is no compatibility alias, second public reader, source-document
fallback, CSS/Takumi route, or raw executable attribute map. The prior
permissive semantic success path is not repaired: final analysis admits only
the checked report attached to the final HIR owner.

The future `#expr`, `#call(...)`, and `#call(...)[content]` authoring surface is
not inferred by this slice. It remains a later grammar contract and will use
`#[...]` only for ordinary code attributes; this implementation does not add
`@static` syntax or revive `$(...)` interpolation.

## Validation

On the coherent public-switch copy:

- checked RichText tests passed 10/10;
- the presentation RichText/schema filter passed 6/6;
- syntax library passed 690/690 and HIR library passed 841/841 executed tests;
- sema library passed 163/163;
- workspace check and workspace strict Clippy passed for all targets/features;
- `git diff --check` passed; and
- both structure-audit gates passed with zero blocking violations.

Runtime display codecs and the future Typst-like `#...` content escape are not
claimed by this typed-validation slice.
