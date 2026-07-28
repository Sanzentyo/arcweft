# Proof 01.1.1.4.1.1 source-owner consistency correction intake

Date: 2026-07-28

Status: `PARTIALLY_IMPLEMENTATION_READY`; full typed `SyntheticKey` and the
dependent public HIR switch remain `DESIGN_BLOCKED`

## Post-intake correction

A deeper predecessor-policy audit after the initial package integrity and
five-blocker review found one additional result-changing omission. The package
defines the final eight-variant `SyntheticOwner` and exact
`ElidedRegion = Type + ordinal 0`, but does not restate a complete
`SyntheticRole::accepts_owner(HirIdKind, u32)` policy:

- four inherited roles were owned by `SyntaxNodeId`, which the final enum no
  longer contains;
- `DesugaredTemporary` names only a generic "lowering owner"; and
- six source-ordered role families define generation order but not boolean
  admission for an arbitrary `u32`.

The stable-fingerprint paragraph likewise names ingredients without fixing
the typed encoder/API, discriminant values, integer widths/endianness, or
hash/transcript owner. Rust `Hash` output is explicitly not a substitute.

Therefore the package's full `READY_FOR_IMPLEMENTATION` claim is not accepted.
The final `SyntheticOwner` enum and `kind()` / `module()` projections are
concrete, and `ElidedRegion` itself is concrete, but implementing full
`SyntheticKey::try_new` would require guessing inherited behavior. The narrow,
independently throwable correction request is
[`Proof 01.1.1.4.1.1.1`](../reviews/requests/2026-07-28-seq-proof-01.1.1.4.1.1.1-synthetic-role-owner-admission-correction.md).

## Archive integrity

- Repository path:
  `docs/reviews/designs/proof-concurrency-v6.1.1.4.1.1/arcweft-proof-concurrency-v6.1.1.4.1.1-source-owner-and-semantic-consistency-correction-final-contract.zip`
- ZIP bytes: `91,023`
- ZIP SHA-256:
  `2bcd3f78efb76442c2698a24251c4d874f7a941c5a8985649ea157100908a72e`
- members: `24` unique entries
- manifest: `23` intentional non-self rows; every declared byte length and
  SHA-256 matches, with zero missing, extra, duplicate, or mismatched entries
- `FINAL_STATUS.md`: exactly `READY_FOR_IMPLEMENTATION` plus newline
- `OPEN_QUESTIONS.md`: exactly the four bytes `none`
- request copy SHA-256:
  `92e3affbc213e0a755685d6900e20d809611ca2bc07f83c82bbc484fef39db53`,
  byte-identical to the repository
  [01.1.1.4.1.1 request](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md)
- inspected baseline:
  `5018912852a45e96f48735767021bf858ffcd493`, exactly the intake `main`

All sidecars are inside the ZIP. There is no production patch, overlay,
adjacent status/hash file, compatibility implementation, or active workspace
crate.

The embedded `PRIMARY_REQUEST_COPY.md` is semantically complete and identical
to the parent package's embedded primary request. It differs from the current
repository primary request only by two indentation spaces (`22,460` versus
`22,458` bytes). The package does not claim byte identity for this historical
copy, and the difference changes no requirement or implementation result.

## Initial adjudication and retained ready portions

The return remains the standalone normative correction to the retained Proof
v6.1.1.4.1 package for the concrete areas below. It closes the originally
recorded blocker shapes, subject to the additional synthetic-role policy
omission above:

1. `HirModule::source_site(expected_source, HirSourceQuery)` is the sole typed
   Expr/Pattern/Type source query. The old Expr-only reader is deleted in the
   same public switch, not wrapped.
2. Pathless variants retain
   `HirVariantPatternHead::Unqualified(DotShorthand | BareExpectedType)`;
   qualified variants retain a non-empty root-preserving `HirPath`. No empty
   path or Option/Result name special case is authorized.
3. `HirDurationValue` has structural identity including `authored_unit`, while
   `HirDurationSemanticValue` is the exact unit-insensitive checker/cache/
   runtime/verifier value identity.
4. float width overflow and Duration runtime-range overflow are checker-owned
   typed rejections. Their provisional HIR issue variants are deleted, and no
   default, truncating, infinite, or saturated value is published.
5. elided regions use `SyntheticOwner::Type(TypeId)` and ordinal-zero
   `SyntheticRole::ElidedRegion`; the raw-ID owner is deleted rather than
   adapted.
6. source, decoded-string, name, path, registry, numeric, decimal, sequence,
   and Thread limits have one exact `HirLimit` owner, inclusive maximum,
   charge phase, and atomic one-over result.
7. all corrected lowering and test rows are standalone and traceable.

The package contains `82` lowering rows (`35` expression, `12` pattern, `35`
component), `106` top-level test rows, and `164` unique named subtests. There
are no duplicate row IDs. Every referenced `T-Q-*`, `T-RB-*`, `T-PQ-*`,
`T-PRB-*`, `T-CQ-*`, and `T-CRB-*` ID resolves exactly once; every named
subtest has a known parent and lowering row.

## Precedence and retained owners

This correction is authoritative only for source ownership, pathless variant
payloads, Duration identity, checker overflow ownership, typed synthetic
owners, HIR limits, and their traceability. It retains without redesign:

- the parent 35-expression and 12-pattern inventories;
- AW-AH-009.4.2 Dialogue/ID outer records and candidate ordinals;
- the AW-AH-009.3 shared resolver and accounting authority;
- the ordered Thread FlowItem body; and
- same-arena Dialogue/RichText children and checked-value ownership.

The parent Dialogue/RichText contract is byte-identical in this return. The
retained parent ZIP SHA-256 is
`61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708`.

## Deletion-driven implementation boundary

The correction releases the independently concrete private substrates, but
does not yet release the full public HIR switch that depends on a complete
synthetic-key policy. The intended implementation order remains:

1. typed `SyntheticOwner` projection and qualified arena identity;
2. after 01.1.1.4.1.1.1 returns, typed `SyntheticKey` admission/fingerprint;
3. one typed source index and source query;
4. type region and pathless variant payloads;
5. literal, Duration, limit, call, Thread, Dialogue, and RichText records;
6. attached-syntax lowering plus sema/checker products;
7. consumer migration; and
8. one compiling public authority switch that deletes all old readers and
   provisional variants.

Private compiling substrate may land before the public switch, but no
provisional public reader, raw owner, wrapper, alias, dual map, second call
resolver, source-string reparse, source gate, CSS/Takumi path, or
removed-syntax-specific final diagnostic may exist. Current
`SpeakerLine`/`ContentCall`/stringly `HirDialogue` and detached syntax readers
remain frozen until their final replacement consumer can delete them in the
same authority switch; their defects are not repaired.

## Intake validation

- all 24 members were opened;
- every manifest row was recomputed from member bytes;
- request and parent hashes, status, questions, baseline, and member inventory
  were checked directly;
- the complete normative schemas and source/path/literal/limit contracts were
  read and compared with the correction request and retained parent;
- lowering, test, and subtest tables were parsed as TSV and checked for row
  counts, duplicate IDs, missing references, unknown parents, and unknown
  covered rows;
- retained-package ledger: `31` archives, zero unrecorded hashes, and zero ZIPs
  left in the `docs/reviews/` root inbox;
- `git diff --check`: passed; and
- canonical structural audit: `3,807` files, `1,965` Rust files, `906,111`
  physical Rust LOC, and `95` manifests; zero errors and `146` pre-existing
  warnings.

This is a design-package intake cut. It changes no Rust, Cargo, runtime,
render, Agent, MCP, persistence, or codec behavior, so Rust tests and Tier 2
are not applicable.
