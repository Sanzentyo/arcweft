# Proof 01.1.1.4.1 READY-claim redelivery intake

Date: 2026-07-27

Status: `PARTIALLY_IMPLEMENTATION_READY`; public source/arena switch remains
`DESIGN_BLOCKED`

## Archive integrity

- Repository path:
  `docs/reviews/designs/proof-concurrency-v6.1.1.4.1/arcweft-proof-concurrency-v6.1.1.4.1-final-hir-semantic-leaf-expression-payload-correction-final-contract.zip`
- ZIP bytes: `64,523`
- ZIP SHA-256:
  `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708`
- members: `20`
- manifest rows: `19` non-self rows; every declared byte length and SHA-256 is
  exact; no member is missing or extra
- `FINAL_STATUS.md`: exactly `READY_FOR_IMPLEMENTATION`
- `OPEN_QUESTIONS.md`: exactly `none`
- declared baseline: `ac9ce44fe9423efd85280e26832dd30c725b3b34`, an ancestor of the intake
  `main`

The prior 1,305-byte `NOT_READY` placeholder at the same canonical path was
replaced rather than retained as an active parallel package. Its hash remains
in the historical intake note and Git history. All sidecars are inside this
ZIP; no adjacent summary/status/hash file is required.

The archive contains 82 lowering rows (35 expressions, 12 patterns, and 35
components), 99 test rows, 52 API-surface rows, and 24 closed traceability
rows. The manifest and row counts are mechanically sound.

## Accepted implementation authority

The following decisions are concrete and may drive private compiling slices:

- `arcweft_lang_hir::expr` is the final expression owner, with one qualified
  ExprId arena and no detached/raw parallel expression authority;
- the closed 35-expression and 12-pattern families, typed known-family poison,
  and generic `Error` fallback;
- separate type-region and runtime lifetime-registry payloads;
- root-preserving typed paths and snapshot/scope-bound resolution;
- arbitrary-precision integers, canonical decimals, deterministic checked
  float bits, whole-nanosecond Duration payloads, and compact numeric
  sequences;
- value-first/nominal-second ordinary and associated-type calls feeding the
  existing shared resolver, including bare-`Vec` arity failure;
- Thread ownership of an ordered typed FlowItem body with no invented block
  ExprId or tail;
- same-arena Dialogue/RichText expressions, accepted AW-AH-009.4.2 outer IDs,
  typed recovery, and exact/one-over transactional accounting; and
- deletion-driven migration through sema, verifier, runtime-plan, compiler,
  runtime, LSP, formatter, Agent/debug, cache, and project publication, ending
  in one public compiling switch that deletes old readers and variants.

No old `DialogueCall`, `MemoBlock`, `Raw`, string-path reader, source reparse,
compatibility alias, wrapper, dual reader, source gate, or removed-syntax-only
diagnostic is authorized.

## READY-claim rejection

The full package is not decision-complete in four result-changing areas.

### 1. Pattern and type source owners cannot use the declared query

`SOURCE_ROLE_AND_QUERY_CONTRACT.md` defines only:

```rust
HirModule::expr_source_site(ExprId, &HirExprSourceRole)
```

and says there is no parallel query enum or reader. Its own lowering/test rows
nevertheless require the same query behavior for:

- P01-P12, whose owners are PatternId;
- C08-C10 pattern fields, including the otherwise orphaned
  `HirPatternFieldSourcePart`;
- C11-C12 type regions owned by TypeId; and
- C13 path segments owned by ExprId, PatternId, or TypeId.

This is not a spelling-only defect. It leaves the source-table key, typed role
enum, public query signature, foreign/stale behavior, and accepted
`expr_source_site` precedence undecided. Implementing any choice would invent a
public authority prohibited by the same package.

### 2. Pathless variant patterns have no lossless final payload

Current syntax intentionally represents `.Foo` and the expected-type families
`Some`, `None`, `Ok`, and `Err` as `Pattern::Variant { path: None, .. }`.
The returned `HirPatternKind::Variant` instead requires `path: HirPath`, while
`HirPath::try_new` rejects an empty segment list and no short-variant pattern
alternative exists. The lowerer would have to fabricate a path, resolve
semantics early, or incorrectly lower a known family to `Error`.

### 3. Duration comparison contracts conflict

The schema-wide trait rule requires owned payloads/enums to derive structural
`Eq`, `Hash`, and `Ord`, so `authored_unit` participates. The numeric contract
instead requires Duration equality and ordering to compare only whole
nanoseconds and says the authored unit does not affect value equality. Unlike
integer literals, no separate Duration value-comparison API is named.

### 4. Checker-only failures and exact limits remain ambiguous

`HirFloatIssue::WidthOverflow` and
`HirDurationIssue::RuntimeRangeOverflow` are HIR-invalid variants, while the
normative prose says they arise only after a valid canonical HIR value reaches
the checker. No exact checker error owner or construction prohibition closes
the conflict. `DecodedByteLimitExceeded` and matrix charges such as
segment/source bytes also lack an exact owning limit and exact/one-over
boundary.

The matrix also cites 164 `T-Q-*`, `T-RB-*`, `T-PQ-*`, `T-PRB-*`, `T-CQ-*`,
and `T-CRB-*` identifiers absent as rows from `TEST_MATRIX.tsv`. The main rows
contain source/rollback assertions, so their behavior is readable, but the
package's claim that every referenced test ID is closed is not exact.

### 5. Elided-region synthetic ownership is not representable

The returned schema requires `HirElidedRegion` to carry a `SyntheticKey`
owned by TypeId with role `ElidedRegion` and ordinal zero. The accepted Proof
`SyntheticOwner` inventory has no Type owner, while the current private
substrate stores only a raw HIR ID and cannot prove the owner's kind. The
package does not state the exact owner-enum extension, constructor, validation,
or migration. Adding one locally would change qualified synthetic identity.

## Implementation boundary

The contradictions stop only their affected source/public switch. They do not
justify preserving or repairing an old production reader.

Permitted before the follow-up return:

1. private canonical scalar/name/path and runtime-registry owners, excluding
   type-region/elision ownership;
2. deletion of duplicate private provisional owners in the same compiling
   slice;
3. private unambiguous numeric leaf records other than the conflicting
   Duration/checker boundary; and
4. focused invariant, identity, rollback, and compile-fail tests that do not
   publish a second reader.

Blocked pending the narrow correction:

- Pattern arena publication involving pathless variants;
- PatternId/TypeId component source-table publication;
- Type-region/elision publication and its SyntheticOwner extension;
- Duration equality/ordering and checker-failure publication;
- matrix-complete exact/one-over limits affected by the missing budgets; and
- the final public authority switch and old-reader deletion.

The independently throwable correction request is
[`2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md`](../reviews/requests/2026-07-27-seq-proof-01.1.1.4.1.1-source-owner-and-semantic-consistency-correction.md).

## Current-main reconciliation

The package baseline is an ancestor of this intake. Intervening commits remove
unused syntax/HIR forwarding and zero-consumer compiler/project-loader
facades. They do not change the package's core identity, Dialogue application,
resolver, or RichText evidence and they reduce the eventual deletion surface.

The final implementation must not repair active `SpeakerLine`, `ContentCall`,
stringly `HirDialogue`, `typed_tree()`, linked-HIR, or raw tooling readers.
Those active authorities remain frozen until their replacement consumer is
ready for the same public compiling deletion cut.

## Intake validation

- the external and retained ZIP both have 64,523 bytes and SHA-256
  `61e2ee166bff158fe83dcf1484b7b9380a81f60d865377503400d27d238cc708`;
- all 20 members were opened; all 19 non-self manifest rows matched exact byte
  length and SHA-256; `FINAL_STATUS` and the four-byte `OPEN_QUESTIONS` payload
  were checked directly;
- every lowering and test row was read and compared with the exact Rust/source
  contracts and current syntax/HIR owners;
- the final package ledger found 30 retained archives, zero root-inbox ZIPs,
  and zero archive hashes missing from package-specific implementation notes;
- `git diff --check` passed; and
- the canonical structural audit dry-run scanned 3,766 files, 1,954 Rust
  files, 906,287 physical Rust LOC, and 95 manifests with zero errors and 146
  pre-existing warnings.

This is a documentation/package intake cut. It changes no Rust, Cargo,
runtime, render, Agent, MCP, capture, persistence, or codec behavior; focused
Rust tests and Tier 2 are not applicable.
