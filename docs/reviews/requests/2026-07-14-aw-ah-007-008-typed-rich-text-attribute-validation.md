# Request: AW-AH-007/008 typed RichText attribute validation

Date: 2026-07-14
Last revised: 2026-07-16

## Request status and independence

This is a standalone design request for AW-AH-007 and AW-AH-008. The findings
are accepted as real; the assignee does not need the audit ZIP and must not
re-audit their existence. Evidence was recorded at revision
`4204d25965129ced50abe82cf5de67d528b483d0`; the contract targets the current
checkout, where line numbers may have moved.

The first implementation delivery and its `v2` re-delivery contained no
applicable implementation payload. Exact archive, manifest, and missing-content
evidence is recorded in the
[2026-07-14 delivery status](../../implementation/aw-ah-007-008-typed-rich-text-attribute-validation-delivery-status-2026-07-14.md).
This request therefore remains open and is still the only source for the final
design contract that must precede implementation.

These findings must be designed together because duplicate/unknown-attribute
policy and value parsing share one typed attribute schema. Designing either in
isolation would create two incompatible validation boundaries.

## Dispatch contract

This is a **design-contract task**, not an implementation task. Send this file
by itself to exactly one assignee with access to the current Arcweft checkout.
Do not attach either failed delivery ZIP and do not ask the assignee to confirm
whether the findings exist. The findings above are accepted input.

Sequence position:

1. AW-AH-005/006 builtin Fx ownership and typed attribute-free selector work is
   already implemented and remains fixed substrate.
2. This AW-AH-007/008 task publishes one final attribute grammar, schema,
   checked-lowering, diagnostic, recovery, codec, and migration contract.
3. Only after that final contract is accepted should a separate implementation
   task change Rust, tests, schemas, or fixtures.

The assignee must answer the required decisions normatively. `MUST`, `MUST
NOT`, exact type shapes, owner crates, deterministic tables, and explicit
unsupported cases are acceptable. `TBD`, alternatives without a selected
winner, “implementation may decide”, and prose that merely repeats the problem
are not acceptable. If a current surface should be removed instead of
supported, the contract must name that surface, give the removal diagnostic,
and place removal before checked lowering; it must not silently omit the
surface from the schema table.

The design may inspect the current checkout to inventory consumers, but it must
not redesign already implemented substrate unless it identifies a concrete
contradiction with an accepted AW-AH-007/008 invariant. Any such contradiction
must be isolated as a separately named blocker rather than folded into a broad
rewrite.

## Required checkout inputs

Use the current checkout, with these files as the minimum ownership map:

- `crates/arcweft-lang-syntax/src/ast/dialogue.rs` and
  `crates/arcweft-lang-syntax/src/text.rs` for the existing ordered, ranged
  `DialogueTagArg` surface;
- `crates/arcweft-lang-hir/src/model.rs` for the current retained dialogue
  content boundary;
- `crates/arcweft-lang-sema/src/checker/line_plan.rs` and
  `crates/arcweft-lang-sema/src/checker/fx.rs` for semantic checking and Fx
  inventory integration;
- `crates/arcweft-presentation/src/rich_text.rs` for the closed builtin Fx family,
  phase, and property inventory;
- `crates/arcweft-runtime-plan/src/render_text/attrs.rs`, `tag.rs`,
  `contributions.rs`, and `fx/builtins.rs` for every raw reparse and current
  consumer;
- `crates/arcweft-render-text/src/rich_effects.rs`, `rich_text.rs`, and
  `style.rs` for renderer-neutral value consumers; and
- the formatter/LSP, bundle/AWBC, cached-artifact, and debug-trace call sites
  found from the typed boundary, but only when they actually carry this data.

The output must record the exact repository revision it inspected. Line numbers
in the findings are orientation only; symbol and responsibility ownership is
the contract.

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
- AW-AH-005/006 are implemented. `arcweft-presentation::rich_text::BuiltinRichTextFx`
  is the closed owner of builtin selector, family, phase, and property-schema
  metadata. Attribute-free `[.sparkle]` already follows the typed Fx path.
  Do not rebuild those membership or phase tables.
- Renderer-neutral RichText/Fx/layout models should receive validated values.
  Native, Web, headless, and Agent paths converge downstream and must not gain
  backend-specific parsers.
- Existing typed wait/reveal controls and their structured errors are examples
  of missing/malformed distinction; they need not be redesigned unless the
  common attribute design intentionally subsumes them.
- The CSS/Takumi authoring and rendering path has been deleted from current
  main. It is not a compatibility target and must not be reintroduced. Any Web
  validation in this request refers only to a surviving non-CSS Web consumer
  of the shared checked runtime-plan boundary.
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
- Do not restore CSS declarations, Takumi integration, a CSS compatibility
  reader, or a CSS-specific attribute schema. Native-only Style ownership and
  any surviving non-CSS Web rendering path must consume the same checked
  RichText values.

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
- Native, headless, Agent-facing, and any surviving non-CSS Web compilation
  observe the same error; no backend renders a default for an invalid value.
- Every actual codec has round-trip, unknown discriminant/version, oversized
  input, duplicate/tampered field, out-of-range numeric, and dangling-reference
  tests.

Use a small checked-in authoring corpus covering Japanese/Unicode content,
quotes/escapes, all builtin families, and malformed boundary cases. Corpus
tests must call parser/checker APIs; do not search source files for spellings.
Automated source gates are prohibited: no test, script, CI check, or audit rule
may read checked-in implementation or documentation and pass/fail by searching
for symbols, spellings, snippets, module paths, or file locations.

## Expected output

- `DESIGN.md`: normative grammar and duplicate/unknown/positional policy; exact
  syntax, HIR, checked-value, schema, diagnostic, and real-codec type shapes;
  dependency direction; compiler/formatter/LSP/preview recovery semantics; and
  the compatibility-free migration/deletion order.
- `TAG_SCHEMA_MATRIX.md`: one exhaustive row for every currently accepted
  control, direct span, style, layout, transform, object/proxy, builtin Fx,
  host-event, marker, and registered custom/plugin surface. Each row names the
  owner, accepted positional/named form, value kind, required/default status,
  range/unit/limit, duplicate/unknown behavior, checked output, and whether the
  surface is retained or explicitly removed.
- `DIAGNOSTIC_AND_RECOVERY_MATRIX.md`: stable diagnostic identity and fields,
  primary/related range selection, deterministic ordering, compile effect,
  preview effect, and formatter behavior for every failure class.
- `MIGRATION_AND_TEST_PLAN.md`: compiling implementation order, exact raw parser
  and fallback deletion points, and unit/integration/corpus/tamper/cross-backend
  tests mapped to each migration step.
- `REQUIREMENTS_TRACEABILITY.md`: every numbered decision, required test, and
  acceptance criterion in this request mapped to one normative section and at
  least one planned behavioral test.
- `FINAL_STATUS.md`: inspected revision, complete/incomplete result, and any
  genuine external blocker. An incomplete result must not be labeled final or
  implementation-ready.
- OPEN_QUESTIONS.md: exactly the single lowercase line “none” for an
  implementation-ready result.
- REPOSITORY_EVIDENCE.md: exact Git/Jujutsu identities and inspected current
  owner/consumer inventory.
- MANIFEST.txt: sorted lowercase SHA-256 integrity entries for every archive
  member, using the self-entry rule below.

Deliver those files in
`arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-final-contract.zip`.
The archive may also include this request as input provenance, but must not
include `.git`, `target`, caches, credentials, generated build output, or a
speculative Rust patch. This design stage does not require production code or
test logs; it requires a decision-complete contract whose planned verification
is exact enough for the following implementation task.

## Integrity and outside status artifacts

MANIFEST.txt must contain one line per archive member, sorted by relative path:

~~~text
<64-lowercase-sha256>  <relative/path>
~~~

The MANIFEST.txt self-entry uses 64 ASCII zeroes. Every other digest must match
the exact archived bytes.

Return these three files next to the ZIP:

- arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-summary.md
- arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-status.txt
- arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-final-contract.zip.sha256

The .sha256 file contains lowercase digest, two ASCII spaces, the exact ZIP
filename, and LF. The status file uses exactly:

~~~text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_RESULT_CHANGING_DECISIONS=0
REPOSITORY_GIT_COMMIT=<40-lowercase-hex>
REPOSITORY_JJ_CHANGE=<change-id-or-unavailable>
ARCHIVE=arcweft-aw-ah-007-008-typed-rich-text-attribute-validation-final-contract.zip
ARCHIVE_SHA256=<64-lowercase-hex>
~~~

If any required decision, schema row, real-codec inventory, migration deletion
point, or planned test remains unresolved, use STATUS=NOT_READY, explain the
blocker, and do not return a ZIP or .sha256 that claims readiness.

## Design completion gate

Before returning the package, the assignee must verify all of the following:

- every numbered required decision has one selected answer and no unresolved
  branch;
- every currently accepted tag surface appears in the schema matrix, even if
  the selected answer is explicit removal;
- every default is tied only to absence and every malformed spelling has a
  deterministic diagnostic and recovery effect;
- the syntax-to-HIR-to-checked-to-runtime-plan type flow contains no raw-string
  reparse boundary;
- custom/plugin behavior is either a fully typed registration contract with
  limits or explicitly unsupported with a diagnostic;
- formatter and LSP behavior use the same ranges, schema, and diagnostics as
  compilation;
- every actual serialized carrier is named and specified, while internal-only
  values are explicitly identified as having no invented wire format; and
- migration ends with deletion of all raw reparsers and malformed-to-default
  branches, without aliases, dual readers, or compatibility shims.
- CSS/Takumi is not restored, Web evidence is limited to surviving non-CSS
  paths, OPEN_QUESTIONS.md is exactly “none”, and all archive/status hashes
  agree.

Failure of any item keeps AW-AH-007/008 in design, not implementation.

## Acceptance criteria

The design is implementation-ready only when every currently accepted
RichText tag has one schema owner and one validation route; missing and
malformed input are unambiguously different; runtime-plan and renderers never
parse authored attribute strings; and all existing raw reparsers and silent
numeric/enum fallbacks have an explicit deletion point.
