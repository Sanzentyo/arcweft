# Request: AW-AH-007/008 typed RichText attribute validation

Date: 2026-07-14

## Request status and independence

This is a standalone design request for AW-AH-007 and AW-AH-008. The findings
are accepted as real; the assignee does not need the audit ZIP and must not
re-audit their existence. Evidence was recorded at revision
`4204d25965129ced50abe82cf5de67d528b483d0`; implementation targets the current
checkout, where line numbers may have moved.

The first implementation delivery and its `v2` re-delivery contained no
applicable implementation payload. Exact archive, manifest, and missing-content
evidence is recorded in the
[2026-07-14 delivery status](../../implementation/aw-ah-007-008-typed-rich-text-attribute-validation-delivery-status-2026-07-14.md).
This request therefore remains open and is still the only design source for a
future implementation package.

These findings must be designed together because duplicate/unknown-attribute
policy and value parsing share one typed attribute schema. Designing either in
isolation would create two incompatible validation boundaries.

## Findings and evidence

### AW-AH-007: parsed attributes are discarded and reparsed

P1/high-confidence `silent_fallback`.

- `crates/arcweft-lang-syntax/src/ast/dialogue.rs:340-520` retains ordered,
  ranged `DialogueTagArg` values.
- `crates/arcweft-runtime-plan/src/render_text/attrs.rs:71-105` reparses raw
  attribute text by splitting it into strings and collecting named pairs into
  a map.
- `crates/arcweft-runtime-plan/src/render_text/tag.rs:1-23` consumes that local
  parser instead of a checked HIR attribute sequence.

For `[.wave amp=2 stray amp=7]`, the raw parser can discard `stray`, overwrite
the first `amp`, and lose authored order and source ranges. Invalid authoring
therefore becomes a different valid effect.

### AW-AH-008: malformed values become valid defaults

P1/high-confidence `silent_fallback`.

- `crates/arcweft-render-text/src/rich_effects.rs:310-345` includes numeric
  parsing whose failure becomes `Milli::ZERO`.
- `crates/arcweft-runtime-plan/src/render_text/tag.rs:210-352` converts style,
  layout, transform, and enum inputs with default/identity paths.
- `crates/arcweft-runtime-plan/src/render_text/attrs.rs:1-170` conflates absent,
  unrecognized, and malformed values in several helper return types.

Examples include `opacity=oops` becoming zero, an invalid transform component
becoming identity, and an unknown writing-mode-like token becoming a default.
The renderer then sees a valid value and cannot recover the cause or range.

## Established substrate that must be preserved

- Syntax already owns `DialogueTagArg::{Positional, Named}` and
  `DialogueTagArgValue`, including exact source spelling, decoded value, key
  range, value range, and authored order. Extend or lower this structure; do
  not reopen raw tag text downstream.
- AW-AH-005/006 are implemented. `arcweft-dialogue::rich_text::BuiltinRichTextFx`
  is the closed owner of builtin selector, family, phase, and property-schema
  metadata. Attribute-free `[.sparkle]` already follows the typed Fx path.
  Do not rebuild those membership or phase tables.
- Renderer-neutral RichText/Fx/layout models should receive validated values.
  Native, Web, headless, and Agent paths converge downstream and must not gain
  backend-specific parsers.
- Existing typed wait/reveal controls and their structured errors are examples
  of missing/malformed distinction; they need not be redesigned unless the
  common attribute design intentionally subsumes them.
- Parser recovery remains lossless and source-ranged. Tooling's checked edit
  behavior from AW-AH-001/002/004 remains intact.

## Design objective

Define one ordered, ranged attribute IR and one schema-driven, fallible value
validation boundary from syntax/HIR into RichText runtime planning. Missing
values may receive documented defaults; malformed, duplicate, unsupported, or
out-of-range values must never become a different successful rendering.

## Required design decisions

1. Specify the complete tag-argument grammar: positional and named forms,
   whitespace and any comma rules, empty values, quoting, escape sequences,
   embedded `=`, Unicode, and the source ranges retained for recovery.
2. Decide which tag families permit positional arguments, their order and
   arity, and whether named and positional forms may be mixed. Do not leave a
   second positional scanner in runtime-plan.
3. Define the syntax-to-HIR attribute record. It must preserve ordered entries,
   key/value kind, exact ranges, and enough owner/tag identity to validate the
   correct schema.
4. Define the checked value algebra needed by builtins and generic RichText:
   boolean, integer, fixed decimal/milli, ratio/opacity, length plus unit,
   angle, duration, closed enum, selector/PublicId where applicable, text, and
   explicitly schema-owned plugin/custom values.
5. Assign schema ownership. Builtin Fx properties remain on
   `BuiltinRichTextFx`/its property owner; style, layout, transform, proxy, and
   custom/plugin tags need equally explicit owners instead of a global string
   map or scattered matches.
6. Decide the validation layer. Syntax may classify tokens, but HIR/sema or a
   dedicated checked lowering context must resolve names, schemas, units, and
   diagnostics before runtime-plan constructs renderer-neutral values.
7. Define duplicate-key policy. If duplicates are errors, specify primary and
   related ranges. If an intentionally repeatable property exists, represent
   repetition in its schema rather than applying generic last-wins behavior.
8. Define unknown-key and bare-token policy separately for closed builtins and
   registered custom/plugin tags. Unknown data must not disappear. Any
   extension point needs typed registration, limits, and deterministic errors.
9. Define missing versus malformed semantics per field. Defaults apply only to
   absence. Specify range, finiteness, overflow, sign, and unit constraints,
   including valid authored zero and identity values.
10. Define the compile/recovery policy. State whether an invalid tag prevents
    compilation, is omitted from a preview while text remains visible, or
    becomes a typed invalid node. It may not execute using a guessed value.
11. Define multi-error collection and deterministic ordering for one tag and
    nested tags, including how parser recovery errors interact with semantic
    attribute errors.
12. Define canonical formatter behavior for quotes, escapes, ordering, and
    positional/named forms. Formatting must round-trip meaning and retain an
    invalid spelling until a user accepts an explicit repair.
13. Identify every serialized boundary that carries checked RichText values
    (runtime plan, bundle/AWBC, cached artifact, or debug trace). Specify a
    canonical codec only where the data actually crosses one; do not invent a
    public wire format for an internal-only type.
14. Set hard limits for attributes per tag, key/value byte length, nesting, and
    numeric magnitude before allocation or evaluation.

## Ownership and layer constraints

- `arcweft-lang-syntax` owns lossless grammar tokens and ranges, not semantic
  defaults or renderer values.
- `arcweft-lang-hir` should carry the typed/ranged attribute sequence. Semantic
  checking or a dedicated lowering context resolves schema and produces
  diagnostics.
- `arcweft-dialogue` owns closed builtin RichText metadata already implemented.
- `arcweft-runtime-plan` consumes checked attributes and constructs typed
  renderer-neutral data; it must not split raw source or parse numbers.
- `arcweft-render-text`, text layout, and renderers consume validated values.
  They may validate runtime invariants but may not reinterpret authoring text.
- LSP/tooling consume the same diagnostics and ranges; they do not maintain a
  parallel schema.

## Non-goals

- Do not redesign builtin membership, phases, or `.sparkle` classification
  fixed by AW-AH-005/006.
- Do not redesign Ruby/JLREQ layout, the Fx graph, or backend renderers.
- Do not add raw-string compatibility readers, legacy attribute aliases, or
  permissive fallback modes for this unreleased surface.
- Do not declare all custom values to be unvalidated `Raw(String)`. An explicit
  extension contract is required if custom schemas are supported.
- Do not solve the issue by retaining the current map and adding warnings after
  information has already been lost.

## Migration order

1. Publish the normative grammar, schema-ownership table, value algebra, and
   missing/malformed/duplicate/unknown decision matrix.
2. Add the ordered ranged HIR attribute representation and checked diagnostic
   types without changing renderer behavior.
3. Implement schema-driven validation and typed values for every currently
   supported tag family; add codec validation where an actual wire boundary
   exists.
4. Migrate runtime-plan consumers family by family while keeping one checked
   input contract. Intermediate compiling commits must not expose two accepted
   source semantics.
5. Migrate formatter/LSP diagnostics and canonical output to the same model.
6. Delete `parse_attrs`, `parse_typed_attrs`, positional raw scanners,
   `split_once` reparsing, parse-to-zero helpers, and malformed-to-default
   branches once their last caller is gone.
7. Run cross-backend and tamper validation, then document any deliberately
   unsupported custom-tag family as a non-goal.

No deprecated fields, dual readers, alias maps, or migration shims remain in
the final state.

## Diagnostics, errors, and codecs

The design must specify stable structured diagnostics for:

- duplicate key, including first and duplicate ranges;
- unknown key or forbidden positional argument;
- missing required value and unexpected value;
- invalid token kind, quote/escape, unit, enum value, or selector identity;
- non-finite, overflow, underflow, negative, or out-of-range numeric input;
- unavailable custom/plugin schema and resource-limit exhaustion.

Each diagnostic needs a tight primary range, optional related range, owner tag
and property identity, expected kind/range/unit, observed source token, and the
chosen compile/recovery effect. Error values may retain source spelling for
diagnostics; validated runtime models must not retain malformed text.

For each real codec, define canonical discriminants, integer/fixed-point
representation, maximum sizes, unknown-version/unknown-variant errors, and
decode-time revalidation. A decoder must reject tampered duplicate fields,
invalid enum tags, out-of-range values, dangling schema IDs, and noncanonical
ordering rather than applying authoring defaults.

## Required tests and corpus

- Named, positional, and chosen mixed forms round-trip with exact ranges.
- Duplicate keys, unknown keys, bare tokens, missing `=`, empty values, quoted
  spaces, escaped `=`, escaped quotes, CRLF, and Unicode ranges follow the
  documented policy.
- Missing optional values and malformed values produce different typed results.
- Valid zero, negative values where allowed, and transform identity remain
  distinguishable from omission.
- NaN-like spellings, infinities, decimal overflow, integer overflow, invalid
  units, and every closed-enum typo fail with tight diagnostics.
- Every builtin Fx variant is checked against its owner schema; generic style,
  layout, transform, proxy, and custom/plugin families have representative
  positive and negative cases.
- Nested/overlapping tags collect errors deterministically without corrupting
  unaffected text.
- Syntax to HIR to checked runtime-plan round-trip retains meaning and source
  provenance. Formatter output reparses to the same checked values.
- Native, Web, headless, and Agent-facing compilation observe the same error;
  no backend renders a default for an invalid value.
- Every actual codec has round-trip, unknown discriminant/version, oversized
  input, duplicate/tampered field, out-of-range numeric, and dangling-reference
  tests.

Use a small checked-in authoring corpus covering Japanese/Unicode content,
quotes/escapes, all builtin families, and malformed boundary cases. Corpus
tests must call parser/checker APIs; do not search source files for spellings.

## Expected output

- Normative grammar and duplicate/unknown/positional policy.
- Exact syntax, HIR, checked-value, schema, diagnostic, and optional codec
  types with dependency direction.
- Per-tag-family attribute/value/default table.
- Failure and recovery semantics for compiler, formatter, LSP, and preview.
- Compatibility-free migration and deletion plan.
- Unit, integration, corpus, tamper, and cross-backend test matrices.

## Acceptance criteria

The design is implementation-ready only when every currently accepted
RichText tag has one schema owner and one validation route; missing and
malformed input are unambiguously different; runtime-plan and renderers never
parse authored attribute strings; and all existing raw reparsers and silent
numeric/enum fallbacks have an explicit deletion point.
