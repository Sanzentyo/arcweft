# Canonical ordering, IDs, limits, and error precedence

## Runtime plan

- runtime locals: retain current canonical HIR-owner order and contiguous
  one-based `RuntimeLocalDeclarationId` issuance;
- plan types: retain first-seen canonical node traversal and contiguous
  one-based `RuntimePlanTypeId`; exact duplicate declaration reuses ID;
- typed node path: root `[0]`, canonical enum field/vector order, maximum depth
  64, checked `usize -> u32` conversion;
- typed sites: retain the parent closed site/slot/path grammars; BTreeMap order is
  the canonical encoded-site order; duplicate site is an error;
- type batch errors: path shape -> path set completeness -> declaration
  projection -> existing/intra-batch kind conflict -> ID exhaustion -> commit;
- builder collection IDs: checked zero-based `u32` ordinal in insertion order;
  overflow rejects before push.

## Pattern bindings

The current schema is retained exactly: schema byte `1`; local ID `u32_le`;
step count `u8`; tags Whole=0, TupleElement=1, RecordField=2,
SequenceElement=3, SequenceRest=4, VariantPayload=5, RecordRest=6. Maximum depth
64. Empty, too deep, whole-not-exclusive, duplicate rest, rest-not-terminal,
unknown local, unknown tag, truncation, and trailing bytes retain current
structured precedence.

## Generation facts

Each canonical table sorts by its typed key (semantic identity, producer ID,
nominal identity, field ID) after rejecting duplicate keys with unequal rows.
Root scalar values are derived before insertion. Work budget is 65,536 checked
nodes and maximum checked nesting is 64, reusing the accepted parent limits.
Catalog facts use their existing owner-provided canonical transcript/digest and
are compared, never recomputed from source names by core.

## AWBC nominal domains

Maximum rows: 262,144. Staging handles are first-seen opaque values only; final
IDs are zero-based encoded-row order. Same origin/same type is an exact
duplicate; same origin/different type is `ConflictingNominalRecordDomain` and
mutates nothing. Final rewrite errors precede program publication and leave no
partial `AwbcProgram`.

## Publication

Trust/authentication error precedes artifact correlation; correlation precedes
catalog/resources; those precede host binding; host binding precedes target
backend support; all precede atomic publication. Restore bundle/generation
failure precedes snapshot header/value/event decoding.
