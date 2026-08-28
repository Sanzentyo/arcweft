# Checked statement transcript prerequisites

Date: 2026-08-28

Status: accepted implementation direction; implementation in progress

## Evidence at the decision point

The accepted generic-Match C3 contract requires statement and body semantic
digests, but `CheckedStatement` at Git
`b502aa4bcc3c194166e285540d07d46748d8f44e` retains only `effects` plus the
sparse lowering-oriented `CheckedStatementRole`. Its `Ordinary` branch merges
statement meaning that the transcript must distinguish. In addition,
`CheckedEffectField` retains a `String` name and fabricates positional
`arg{ordinal}` names from HIR call syntax.

The current facts therefore cannot be hashed as final semantic authority.
Doing so would require source-name reconstruction, raw-ID hashing, or an
unsupported/`Any` success branch. All three are rejected.

## Required implementation order

1. Move evaluated-effect operands onto callable-owned open-slot identities and
   sealed execution sources. Delete field-name reconstruction.
2. Add one HIR-owned control-transfer index to the existing root-local
   semantic topology. It resolves successful `out`, `break`, and `continue`
   statements to typed targets before sema publication.
3. Replace the sparse statement role with one final checked non-child payload
   authority. It retains only semantics not already owned by typed HIR
   children and body projections.
4. Seal evaluated effects, control targets, Include Flow targets, locale,
   unsafe-audit, trigger, Select, and named-scope facts after accepted roots,
   structural edges, and final call applications exist.
5. Build one atomic semantic-transcript catalog over expressions, patterns,
   statements, and bodies. The lazy Match-only builder is not the final
   publication authority.

This ordering is a prerequisite relationship, not a request to preserve an
intermediate compatibility model.

## HIR control-transfer authority

The topology owns root-local rows equivalent to:

```text
statement + Out      -> dialogue line-plan output application
statement + Break    -> Loop/While/WhileLet/For body owner
statement + Continue -> Loop/While/WhileLet/For body owner
```

Scope parentage is validation and target-selection evidence. It is not copied
into sema, and sema must not repeat label or scope resolution.

Output continuations are conceptual line-plan owners rather than executable
body rows, so they receive their own accepted-rooted output coordinate. Loop
targets join the existing `HirSemanticBodyOwner` and
`StableCheckedBodyCoordinate` authority.

Current final HIR retains label uses on control-transfer statements but has no
produced loop-target label declaration, and production line-plan lowering does
not currently produce a label. A labeled transfer therefore rejects as an
unresolved typed target. The implementation must not hash the label use or
silently treat it as the nearest unlabeled target.

## Final checked statement payload

`HirStmtKind::semantic_transcript_tag()` remains the exhaustive 35-shape
authority. The checked payload is grouped by non-child meaning rather than
copying the HIR statement algebra. Required payload families are:

- structural;
- assignment and assertion;
- defer outcome;
- evaluated effect;
- checked iteration;
- output and loop-control targets;
- trigger and Select branch semantics;
- unsafe audit;
- source locale and named/anonymous scope;
- Include Flow target;
- suspension and yield.

Expression, pattern, type, local, statement, and body children continue to
come from HIR typed edge/projection authorities. No copied statement AST or
parallel body model is introduced.

## Transcript boundary

The final transcript cut introduces private, non-Serde version-one
`CheckedStatementSemanticDigest` and `CheckedBodySemanticDigest` newtypes.
One memoized, cycle-checked graph constructs expression, pattern, statement,
and body digests after all checked rows are complete. A failure publishes no
partial transcript catalog or `CheckedMatch` row.

Statement and body records start with accepted-rooted stable coordinates.
Body traversal consumes `HirSemanticBodyRow` in source order; it never infers a
body from path prefixes or fabricates a `BodyId`. Heterogeneous Thread children
retain an explicit expression-versus-statement child-kind tag.

There is no transcript `Any`, wildcard identity, source-spelling fallback, or
`UnsupportedIdentity` success path. Coverage's typed open-domain `Other`
constructor remains coverage-only and is unrelated to transcript identity.

## Implemented prerequisite cuts

Evidence was refreshed on 2026-08-29 at Git
`fe7cbcc5cfa2f737ee0ac10e5c0c7dbebcf18f0a` with unrelated user WIP and the
current four-file sema coordinate cut present in the working tree.

- HIR now resolves each accepted `out`, `break`, and `continue` statement to
  one root-local typed target. Labeled transfers reject because no target-label
  declaration authority currently exists.
- evaluated-effect open fields now retain callable-owned `OpenArgumentId`
  identities. Positional names are not fabricated and runtime spelling is
  projected only at the compiler boundary.
- sema now issues affine control-transfer evidence from the HIR topology and
  accepted root catalog. Line-plan output coordinates append the disjoint
  `output=0x01, dialogue-line-plan=0x00` suffix; loop transfers retain both the
  exact `HirLoopTargetFamily` and the existing checked body coordinate.

These are prerequisites only. The sparse statement role and the lazy
Match-only transcript builder remain scheduled for deletion by the following
statement-seal and atomic transcript cuts.

## Structural review

The canonical audit scanned 2,262 files and 95 workspace packages, reported
273 review triggers, and found zero blocking violations. Both the screening
and `--fail-on-blocking` gate passed.

`semantic_coordinate.rs` remains the cohesive owner of accepted-rooted checked
coordinate types and their canonical byte grammar; issuance and topology joins
remain separated in `semantic_coordinate/catalog.rs`. The cut adds only
crate-private control-target coordinates and does not widen a public API, copy
HIR scope state, or invert the HIR-to-sema dependency. Splitting these
coordinate types by statement family would scatter one canonical grammar, so
the existing owner is retained.

`final_analysis/tests.rs` remains the integration-fixture owner for private
final-analysis publication behavior. The two added tests exercise the same
accepted-root/topology/report fixture authority as the surrounding coordinate
tests. Moving only these rows would duplicate the private fixture harness or
introduce a test-only access seam, so the cohesive integration owner is
retained despite the upper LOC review trigger.

## Validation of the checked control-target coordinate cut

Passed:

- both focused output-target and four-loop-family tests (one test each);
- `cargo test -p arcweft-lang-sema --tests`: 542 library tests, 12 compile-API
  tests, and 4 integration tests;
- `cargo check --workspace --all-targets --all-features`;
- `cargo clippy --workspace --all-targets --all-features`, with existing
  warnings and no command failure;
- package formatting and repository diff checks; and
- `just structure-audit` plus `just structure-audit-gate`.

Blocked environment validation:

- the first `just test-workspace` attempt exhausted the build volume after the
  workspace `target` cache had grown to 296,527,421,327 bytes;
- `cargo clean` removed 146,998 regenerated files (276.2 GiB), without touching
  tracked files or user WIP; and
- clean and cached retries both reached Windows error 1455 while concurrently
  linking the broad workspace test inventory: the configured paging file was
  too small to map the generated `rlib` metadata. No Arcweft assertion or
  compile diagnostic preceded those environment failures.

Tier 2 was not run because this sema-private coordinate cut does not change a
runtime, renderer, Agent, MCP, capture, protocol, or persisted public contract.
