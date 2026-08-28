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
