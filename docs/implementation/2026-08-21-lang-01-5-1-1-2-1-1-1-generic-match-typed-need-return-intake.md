# Lang-01.5.1.1.2.1.1.1 generic Match and typed Need return intake

Date: 2026-08-21
Inspected Git commit: `4bda1cdcea63fdf7aac32691d756c1c0e1fc693e`
Working tree before intake: clean; `main` matched `origin/main`
Supersedes current readiness in:
`2026-08-21-lang-01-5-1-1-2-1-1-1-generic-match-and-typed-need-producer-abi-blocker.md`

## Intake result

- Archive integrity: `PASS`
- Internal package validator: `PASS`
- Repository reconciliation: `FAIL`
- Classification: `DESIGN_NOT_READY`
- Production implementation: `BLOCKED_PENDING_CORRECTION`
- Open questions claimed by package: none
- Production code changed or validated by package: none

The return successfully selects a frame-safe single-Variant selector result,
core-independent View coordinates, explicit guard Branch lowering, a dedicated
typed Need runtime value, a resource-registry input route, and strict
version-1 deletion. It cannot be implemented as returned because its opcode
allocation conflicts with two accepted owners, its global task-plan identity
deletion has no non-View replacement, and its checked coverage/ownership/digest
owners remain underdefined.

## Retained archive

External source archive:

- path:
  `D:/sanze/Downloads/arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract.zip`
- byte length: 89,058
- SHA-256:
  `96F4F84BE1B7B2BBEC9D2BA564418F00F453870F4EA331566A1F51258CC1EF8D`

The unchanged byte authority is retained at
[`docs/reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract.zip`](../reviews/packages/zips/arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract.zip).
Its 36-file byte-identical frozen mirror is retained under
[`docs/reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract/`](../reviews/packages/arcweft-lang-01.5.1.1.2.1.1.1-generic-match-and-typed-need-producer-abi-correction-final-contract/README.md).

## Performed and passed

- Verified one redundant top-level directory, 36 safe file members, no
  absolute/drive/parent-traversal paths, and no duplicate names.
- Verified the retained ZIP is byte-identical to the external attachment.
- Verified all 36 extracted files are byte-identical to their ZIP members.
- Verified 33/33 `SHA256SUMS` payload rows and the separate manifest envelope.
- Verified `MANIFEST.json` SHA-256
  `2EF2A8402EFCC83C7D37CA1165B27237E4F23F73927766E45D956001F55DA9DE`
  equals `MANIFEST.sha256`.
- Verified `OPEN_QUESTIONS.md` is exactly `none` plus LF.
- Read the final contract, decision register, Rust schemas, selector, guard,
  typed producer, bundle, wire/digest, persistence, resource, owner/dependency,
  failure, sequence, evidence, traceability, consumer/deletion, tests,
  structural absence, validation, and validator contents.
- Re-ran the inspected stdlib-only validator with `uv run --no-project`; it
  reported 36 files, 148 tests, 48 evidence rows, and PASS.
- Confirmed opcode byte `0x1e` and function flag bit 4 are unused by current
  production, runtime type tag 19 is currently payloadless, NeedHandle is
  currently String-backed, and `RuntimeTypeShape::Need` is currently rejected
  from checked runtime projection.
- Obtained an independent Sol-max design audit; it confirmed the opcode,
  non-View task identity, coverage, ownership, persistent HIR identity, wire,
  and construction-API blockers without editing repository files.

## Failed repository reconciliation

### Opcode collision

The package allocates `MakeNeedHandle = 0x1e` because current Rust leaves that
slot open. Maintained
[`executable-runtime-core.md`](../02-runtime/executable-runtime-core.md)
already allocates `0x1e` to `NeedTimeout`. The accepted line-plan return
allocates `0x1e` to `ExecuteLineOperation` and `0x20` to
`CommitDialogueResult`. Empty current Rust is not the sole allocation
authority; all pending version-1 operations require one collision-free table.
The package's `NEED_PRODUCER` flag bit 4 also conflicts with the accepted
stream producer's `OWNS_STREAM_PRODUCER` bit 4.

### Missing non-View task identity replacement

The package deletes `AwbcTaskPlan.need_id` and changes `NeedId` to fixed bytes,
but defines derivation only for a flagged View producer with `many == None`.
Current ordinary task lifecycle, direct Await, AwaitMany indexed children and
in-flight snapshots, structured suspension, runtime-plan interning, AWBC
codec/verifier, bundle mapping, and product-step completion all consume the
task-plan Need identity. The package does not define their replacement.

### Caller-supplied Match coverage

Normative `CheckedMatch::try_from_hir` receives a `CheckedMatchCoverage`
argument. No current checked coverage owner exists, and the package does not
define a full pattern coverage/reachability algorithm or corresponding test
matrix. A caller could therefore fabricate exhaustiveness/unreachable facts.

### Incomplete ownership and digest authority

`RegisteredSemanticWorld::checked_ownership(&TypeKind)` is named without an
exhaustive mapping or sufficient inputs for project/resource/opaque and
producer-argument-dependent cases. The checked-Match digest also relies on
unspecified canonical bytes for HIR IDs. The accepted final-HIR View parent
explicitly keeps those IDs session-only and excludes them from persistent
identity, so the returned content-root/replacement digest violates its parent.

### Wire and construction API mismatch

The package states that all `u32` IDs/counts are fixed little-endian, while the
current AWBC wire owner uses canonical unsigned base-128 varints for ordinary
`u32` and vector lengths. Its normative schemas also name
`RuntimeSemanticFactInput` and `AwbcVm`, but current production owns
`RuntimePlanSemanticFactInput` and functional VM entry points. The return does
not define a migration to the invented owners.

### Request-copy discrepancy

The ZIP's `INPUT_REQUEST.md` is 14,785 bytes; the repository request is 15,712
bytes. It omits the later-added current-source guard-defect section and guard
decision, then independently restores that issue as package decision D17.
Thus design coverage is not missing solely because of this copy difference,
but the README claim that the complete current request is retained and the
validator's request-file check are inaccurate. The next validator must compare
exact request hashes.

## Blocking request and next action

Implementation remains blocked by
[`Lang-01.5.1.1.2.1.1.1.1`](../reviews/requests/2026-08-21-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction.md).
It requires one collision-free global opcode table, complete task/Need identity
for View and non-View consumers, an inherent full-pattern coverage owner, a
total ownership classifier, parent-compatible product identity, canonical wire
primitives, and constructible current APIs.

No Rust, Cargo manifest, production test, fixture, generated artifact, stable
chapter, build, Clippy, platform, AOT, or runtime validation was changed or run
for this intake cut.

## 2026-08-21 user-directed allocation preselection

Decision audit baseline:
`c49099fb154d9e3dbb587e1bcd7ee243214da0c4`, clean and equal to
`origin/main` before this documentation update.

After this intake, the user directed that opcode, function-kind, function-flag,
and `u32` wire collisions be selected before redelivery rather than left to the
next reviewer. A Sol-max read-only audit selected and the maintained request now
fixes:

- one full version-1 semantic renumber with value construction/projection in
  `0x00..=0x14`, call/effect/task/Need in `0x20..=0x29`, Stream/line in
  `0x30..=0x36`, ownership/lifecycle in `0x40..=0x48`, and terminators grouped
  by CFG, suspension, Stream, presentation, and fault families;
- `Flow, Ordinary, PureHelper, TraitMethod, Synthetic, GeneratorProducer,
  StreamTransform, LineActivation, LineTask` at dense function-kind tags 0..8;
- Need producer flag bit 4 and Stream producer flag bit 5 under one typed flag
  enum/validated bitset;
- `#[repr(u8)]` opcode/kind/flag enums as the only numeric declarations, with
  direct numeric Serde/private Wire and allocation-free scalar decode;
- one shortest canonical base-128 varint owner for all ordinary `u32` values,
  including tensor dimensions; and
- one final encoder buffer and direct borrowed-reader decode, with allocations
  only for final owned Vec/String values.

These replace the earlier current-value-preserving preselection; no removed
tombstone, old numeric reader, or historical alias remains. The updated
[`Lang-01.5.1.1.2.1.1.1.1` request](../reviews/requests/2026-08-21-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction.md)
makes these selections mandatory. The remaining design blockers are complete
runtime Need/task identity, checked Match coverage, ownership admission, and
parent-compatible stable Match product identity.

Future returned archives remain byte-preserved frozen evidence. If an otherwise
complete return repeats an allocation from before this semantic reorder, intake
records that numeric row as superseded and projects the selected enum authority
locally. Allocation spelling alone does not require another design round. The
frozen ZIP/mirror is never rewritten, and no compatibility reader or old-to-new
runtime translation table is added.
