# Request: AW-AH-015 vertical-break quality policy

Date: 2026-07-14

## Request status and independence

This is a standalone design request. AW-AH-015 is accepted as a real P2
underspecification; the assignee does not need the audit ZIP and should not
re-audit whether the numeric literals exist. Evidence was recorded at revision
`4204d25965129ced50abe82cf5de67d528b483d0`; implementation targets the current
checkout.

The policy and evaluation corpus must be approved before production constants
are moved. Merely giving the existing literals names is not completion.

## Finding and evidence

AW-AH-015 is a medium-confidence `underspecified` finding:

- `crates/arcweft-text-layout/src/document_vertical.rs:148-182` computes a
  dynamic-programming column cost from raggedness `100 * cube`, effective
  overflow `10_000 * square`, allowed hanging overflow `50 * square`, a base
  break penalty `5`, and JLREQ pair adjustment.
- `crates/arcweft-text-layout/src/jlreq_punctuation.rs:1-500` supplies typed,
  versioned punctuation classes and pair adjustments.
- `docs/03-presentation/text-typesetting.md:1-500` documents vertical writing
  and JLREQ behavior but does not define the optimization units, normalization,
  provenance, or acceptance criteria for those weights.

Current tests pin representative break results, but a pinned result does not
explain why the objective is correct or how it should scale across font sizes,
metrics, and containers. The audit did not prove that current output is wrong;
it proved that its quality policy is not a reviewable contract.

## Established substrate that must be preserved

- `arcweft-text-layout` owns vertical planning and the deterministic DP.
- `JlreqStrictness` and the generated, versioned JLREQ punctuation/pair tables
  are established, tested substrate. Prohibited head/end breaks, pair
  keep-together decisions, compression, and hanging-cluster classification are
  not arbitrary literals and must not be redesigned here without a concrete
  defect.
- Vertical-left-to-right and vertical-right-to-left share an inline break plan.
  Backend rendering consumes the common layout result; do not introduce a
  Native/Web-specific quality policy.
- Document layout hashing already includes relevant authored layout policy such
  as JLREQ strictness. Any new externally selectable break policy must be
  included in deterministic cache identity.
- Existing text shaping, ruby semantics, writing-mode selection, and renderer
  parity are independent substrate.

## Design objective

Specify a named `VerticalBreakPolicy` (or equivalently owned policy type) with
documented normalization, units, hard constraints, soft penalties, tie-breaks,
numeric behavior, and quality goals. Establish a curated Japanese evaluation
corpus and approval method that justify the selected defaults across scale and
container changes.

## Required design decisions

1. State the product-quality goals in priority order: avoid illegal breaks,
   avoid non-hanging overflow, permit defined punctuation hanging, control
   raggedness, respect pair preference, and ensure deterministic tie-breaking.
2. Separate hard constraints from soft penalties. JLREQ-prohibited breaks and
   any required keep-together rule must not become merely a large finite cost
   unless the terminal overflow policy explicitly requires an escape.
3. Define normalization units for capacity, used advance, remaining space,
   overflow, and allowed overhang. Specify whether normalization uses em,
   representative cluster advance, column capacity, or another stable metric.
4. Define scale invariance. Multiplying font metrics and container dimensions
   by the same factor should preserve break decisions except at documented
   rounding boundaries.
5. Define each cost term mathematically, including exponent/curve, coefficient
   dimension, cap, and intended tradeoff. Explain how the coefficients were
   derived or calibrated rather than copying current values.
6. Define the final-column policy. State whether final raggedness and base break
   costs differ from intermediate columns and how short final columns are
   treated.
7. Define hanging punctuation interaction: eligible clusters, maximum
   overhang, whether allowed hanging has any penalty, and its ordering relative
   to an earlier legal break.
8. Define how `JlreqStrictness` pair adjustment enters the objective. Preserve
   the generated table as the source of punctuation facts; the new policy owns
   only how typed adjustments influence planning.
9. Define terminal overflow behavior when no legal fit exists. State when an
   overflowing column is permitted, how it is penalized, and how a diagnostic
   or trace distinguishes it from a normal optimum.
10. Define the tie-break as a total deterministic order. Avoid architecture-
    dependent floating epsilon comparisons; specify stable preference for
    earlier/later break, fewer columns, overflow, or source order.
11. Choose numeric representation and checked arithmetic. Specify finite
    inputs, normalization bounds, overflow behavior, and whether fixed-point,
    rational, ordered integer tuples, or bounded floating cost is appropriate.
12. Decide whether one internal default policy is sufficient or authored/host
    presets are required. If configurable, define closed preset identity,
    serialization, cache hash, validation, and unsupported-version behavior.
13. Define trace/explain data for reviewing a chosen break: normalized terms,
    rejected hard constraints, winning cost components, and tie-break reason.
    Keep this bounded and renderer-neutral.
14. Define the evaluation corpus, licensing/provenance, annotations, metrics,
    approval process, and change threshold. Include prose, dialogue, punctuation,
    ruby-adjacent text, mixed Latin/digits, narrow/wide columns, and pathological
    unbreakable sequences.
15. Define how quality changes are reviewed. Automated metrics and deterministic
    goldens may catch drift, but must not blindly promote a new default without
    explicit review of corpus deltas.

## Ownership and layer constraints

- `arcweft-text-layout` owns policy, checked cost evaluation, tie-break, and
  bounded explain data.
- `jlreq_punctuation` owns punctuation classes and pair facts. Do not duplicate
  or hand-edit generated tables in the policy.
- `docs/03-presentation/text-typesetting.md` records the stable design once
  approved; transient measurements and corpus review belong under
  `docs/implementation/` or fixtures.
- Renderers consume planned columns and do not rescore breaks.
- If policy is authored or serialized, the owning typed config/codec supplies
  inherent validation and deterministic hashing; adapters do not parse names.

## Non-goals

- Do not redesign JLREQ punctuation data, ruby layout, shaping, horizontal line
  breaking, glyph rasterization, or renderer composition.
- Do not claim quality solely because current unit tests pass.
- Do not replace the literals with constants without a normative objective and
  corpus evidence.
- Do not add per-backend weights or font-specific hard-coded exceptions.
- Do not expose arbitrary user-provided floating weights in the first contract
  unless the design proves a stable, validated use case.

## Migration order

1. Check in the policy specification, mathematical term table, corpus manifest,
   annotations, and baseline evaluation report before changing production
   planning.
2. Decide the default policy through documented corpus review and record known
   tradeoffs/non-goals.
3. Add the final typed policy, validation, checked numeric evaluation, total
   tie-break, and bounded explain result.
4. Include policy identity in document/cache hashes and add a codec only if the
   policy crosses a real persisted boundary.
5. Replace the local cost literals with the approved policy in one owner; keep
   the existing JLREQ hard facts connected through typed APIs.
6. Run scale/property/corpus/backend validation and update stable design docs.
7. Delete the old unowned `column_cost` coefficients and any temporary
   comparison path. Do not retain selectable legacy weights.

## Diagnostics, errors, and codecs

Internal fixed defaults do not need authoring diagnostics, but checked planning
must report invalid/non-finite metrics, arithmetic overflow, impossible policy
parameters, resource-limit exhaustion, and terminal forced overflow through
typed errors or bounded trace status. It must not map invalid cost to zero,
infinity, or an arbitrary break.

If policy is configurable or serialized, define required fields, closed preset
IDs/version, canonical numeric representation, limits, and errors for missing,
malformed, out-of-range, unknown-version, and noncanonical values. Decode must
revalidate mathematical invariants. No compatibility reader is required for
the current unversioned local literals.

Corpus records must include stable case ID, source text or licensed reference,
writing direction, font/metric fixture identity, container dimensions,
strictness, expected invariants, and reviewed preferred/acceptable breaks.
They must not encode absolute host paths.

## Required tests and corpus evaluation

- Uniform scale changes across representative font sizes and containers
  preserve break offsets modulo documented rounding cases.
- Narrow, exact-fit, wide, and unavoidable-overflow columns exercise every cost
  term and terminal escape.
- Head/end prohibition, keep-together pairs, strictness levels, compression,
  and hanging punctuation retain their typed JLREQ behavior.
- A legal earlier break competes with hanging and non-hanging overflow according
  to the documented priority.
- Exact and near ties follow the total deterministic order across repeated
  runs, architectures, and Native/Web/headless consumers.
- Zero/tiny/huge/non-finite metrics and arithmetic-boundary inputs return typed
  results without panic, NaN ordering, saturation to an unrelated optimum, or
  unbounded work.
- Vertical-lr and vertical-rl retain the same inline break plan where expected.
- Corpus includes Japanese prose/dialogue, opening/closing punctuation,
  consecutive punctuation, leaders/dashes, ruby-adjacent clusters, mixed
  Latin/digits, emoji/grapheme clusters, and unbreakable sequences.
- Evaluation reports quality deltas per case and aggregate invariants; reviewed
  exceptions are explicit, not silently accepted by regenerating goldens.
- If a codec exists, round-trip and tampered missing/malformed/unknown-version,
  extreme coefficient, noncanonical numeric, and oversized corpus/policy cases
  are rejected.
- Common layout output is parity-tested through Native, Web, and headless paths
  without backend-local rescoring.

Tests must call layout/policy/codec APIs. Do not add source gates that search
for numeric literals, function names, or file locations.

## Expected output

- Normative `VerticalBreakPolicy` goals, term equations, units, normalization,
  hard/soft ordering, numeric representation, and total tie-break.
- Default-policy derivation and approval rationale.
- Versioned corpus manifest, annotations, provenance, baseline report, and
  review thresholds.
- Exact owner/config/hash/optional-codec/error/explain contracts.
- Compatibility-free migration and deletion order.
- Unit, property, corpus, tamper, and cross-backend validation matrix.

## Acceptance criteria

The design is implementation-ready only when every cost term has a documented
meaning and unit; hard JLREQ facts remain separate; scale, overflow, and ties
have deterministic rules; a reviewed corpus justifies the default; any real
codec/cache boundary is specified; and implementation can replace the literals
without preserving legacy weights or guessing aesthetic policy.
