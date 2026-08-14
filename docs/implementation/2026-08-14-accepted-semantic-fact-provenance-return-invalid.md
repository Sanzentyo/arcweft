# Accepted semantic-fact/provenance return invalid

Date: 2026-08-14

Continues:
`docs/implementation/2026-08-14-external-lowering-generation-admission-return-invalid.md`

Inspected Git baseline:
`a033f3c529f58b02cca3f03fd5239617ecba14a2` on `main`, equal to
`origin/main`, with a clean working tree before ZIP intake.

## Returned archive intake

The downloaded archive had a delivery-only `1-` prefix. Because the canonical
repository name was free, intake removed that prefix. The retained archive is:

`docs/reviews/packages/zips/arcweft-lang-01.3.1.2.3.2.1.2.1.1.1.1.1-accepted-semantic-fact-provenance-and-compile-clean-admission-order-correction-final-contract.zip`

SHA-256:
`f3b61c32591de484ac9cefa2db9197686a1815113f00fd152e1f2ef80e256f85`

The 18,794-byte flat ZIP contains 18 files. It has no unsafe/rooted/drive/
traversal path, symlink/reparse entry, or case-insensitive collision. All 18
extracted files match their ZIP-member SHA-256 values and all 17 internal
`MANIFEST.sha256` rows pass.

The package reports `READY_FOR_IMPLEMENTATION`, `OPEN_QUESTIONS=0`, design
commit `a033f3c529f58b02cca3f03fd5239617ecba14a2`, and Arcweft-owned versions
fixed at `1`. It contains no production patch.

## Request-copy failure

The package does not contain the required current `SOURCE_REQUEST.md` or its
hash. The maintained request is:

`docs/reviews/requests/2026-08-14-lang-01.3.1.2.3.2.1.2.1.1.1.1.1-accepted-semantic-fact-provenance-and-compile-clean-admission-order-correction.md`

Its SHA-256 is
`1b54121c38f7f957f9c168a02d25fef26ba21e7f50da9fc89e4b390ac9281c65`.
The package instead includes `PARENT_SOURCE_REQUEST.md`, byte-identical to the
previous request with SHA-256
`2498106d805515f2fba326ef55685a8699aec2ab1abb986e22bc2f0a1f984cc6`.
The returned package therefore cannot prove that its decisions answer the
current request.

## Readiness adjudication

Full-package/current-source inspection and an independent Sol max audit
classify this return as `INVALID_AS_DELIVERED`. These are unresolved decisions
and internal contradictions in the requested scope, not an external
authority, so `NOT_READY` is not appropriate.

1. The core-owned `RuntimePatternBindingCoordinate` directly contains
   `arcweft-lang-hir` `LocalId` and `CaptureId`. `arcweft-core` deliberately
   has no HIR dependency. Current core already owns
   `RuntimeLocalDeclarationId` and `RuntimeCaptureSlotId`, but the return does
   not define their accepted HIR-to-runtime allocation/projection or exact
   binding-coordinate use. The proposed owner cannot compile without reversing
   the documented layer direction. Its stated wire grammar also omits the
   identity bytes.
2. Only `ExprId -> RuntimeNormalizedType` is projected. No exhaustive
   `PatternId -> RuntimeNormalizedType` fact, constructor, accessor, or
   completeness rule is defined, although `FinalPatternLowerer` and
   `RuntimeTypedPattern` require exact accepted types for every pattern node.
3. The proposed higher-layer operational wrappers do not form a usable
   dependency boundary. `arcweft-runtime-driver` depends on
   `arcweft-bundle` but not `arcweft-compiler`, so it cannot own the suggested
   enum containing both `CompilerAcceptedRuntimeProduct` and
   `VerifiedBundleRuntimeProduct` without an unselected layer change. The
   returned APIs use a nonexistent `VerifiedBundle`, omit the retained runtime
   catalog inputs, and leave placeholder `/* accepted catalogs/lowering
   inputs */` arguments instead of exact Rust APIs.
4. `TYPE_MAPPING.csv` makes Tuple, Choice, Result, and Option unconditionally
   `Checked`. The accepted parent requires their exact closed projection only
   when every descendant is checkable; a Function/Range/other operational
   descendant must select the corresponding closed operational composite tag.
   The return therefore changes admissible expression facts and contradicts
   its retained substrate.
5. `SYNTHETIC_EXPR_TYPE_TABLE.csv` is not exhaustive for current lowering. It
   omits current shorthand-local, variant-call tuple, let/assignment-field,
   reduction empty-command, and agent-produced scalar/tuple/record nodes. The
   `AgentScaffold` slot also has no canonical slot grammar or current owner.
6. The AWBC nominal-domain interner does not define canonical ID assignment,
   ordering, or reference remapping when duplicate rows are interned. The
   public program builder therefore cannot be implemented deterministically
   from the stated grammar alone.
7. The claimed complete inventory has only 17 production rows and the test
   matrix has only 53 rows. They omit affected current consumers and include
   commands for nonexistent package `arcweft-aot`. The P10/P13 focused gates
   therefore cannot run as written and the matrices do not satisfy the
   requested exhaustive regeneration.

## Next action

Do not create another child correction. Re-submit the same maintained request
listed above. Require the return to include its exact copy/hash and to answer
every existing required decision against current `main`, including the
specific current-source conflicts in this note. Generic summaries and
placeholder arguments are not acceptable substitutes for final APIs, mapping
tables, inventory, or executable phase commands.

No production cut from this return is safe in isolation. Preserve the
previously accepted root projection, `OpaquePayload`, outer-shape, catalog
digest, atomic opaque, and effect-owned AudioCommand decisions while waiting
for a valid response.

## Validation performed

- source and retained ZIP SHA-256/byte equality: passed;
- unsafe path, traversal, symlink/reparse, and case-collision preflight: passed;
- ZIP member versus extracted file SHA-256 parity: 18/18 passed;
- internal `MANIFEST.sha256`: 17/17 passed;
- current request-copy/hash requirement: failed;
- all returned normative Markdown and CSV files were inspected;
- current core/runtime-plan/sema/compiler/bundle/runtime-driver Cargo and
  source owners were compared with the returned APIs; and
- no production code or test was changed.
