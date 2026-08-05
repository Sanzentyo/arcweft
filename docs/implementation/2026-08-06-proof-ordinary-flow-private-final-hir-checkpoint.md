# Proof ordinary Flow private final-HIR checkpoint

- Date: 2026-08-06
- Implementation base revision: `77b28b493ce70b8bd0c4637a2c375c6f71553574`
- Commit parent after the independent ownership-decision cut:
  `c4127d71e1d4b1f722a00a18cf4352c1ea0c7996`
- Contract intake:
  [Proof ordinary Flow redelivery intake](2026-08-03-proof-ordinary-flow-redelivery-intake.md)
- Status: `PRIVATE_CHECKPOINT_VALIDATED_PUBLIC_SWITCH_PENDING`

## Result

The attached ordinary-Flow syntax now lowers transactionally into the one
database-owned final HIR model. This checkpoint does not publish a second
Flow reader. The existing production clone-HIR Flow/Dialogue path remains
frozen and is the deletion inventory for the public authority switch; defects
in that path are not repaired here.

The private final model owns:

- the four accepted identity states without source-string reconstruction;
- shared generic parameters, fixed parameters, where predicates, and the
  omitted-Unit versus authored return distinction;
- callable, requires, ensures, and Flow body scopes;
- one optional `PostconditionResult` local in the ensures scope;
- all nine contract variants in heterogeneous source order;
- the shared statement-only `HirThreadBody` with all sixteen typed item
  variants and no tail expression; and
- one exact source manifest and typed poison record frozen against the same
  attached snapshot.

## Lossless recovery and source authority

Flow recovery no longer collapses a category to its first issue. Lowering and
source freeze retain and compare the complete typed sequence:

1. prefix;
2. identity, including Name primary plus PublicId related evidence for an
   ID/name mismatch;
3. generic and parameter component issues, every signature-recovery ordinal,
   authored return poison, and where-predicate poison;
4. every contract issue and poisoned operand in clause/operand order,
   including later-primary/first-related duplicate `decreases` evidence;
5. missing body, or every recovered body child followed by an unclosed body;
   and
6. every declaration-trailing recovery row.

Body-child poison uses the actual child owner and the central
`HirThreadFlowItemSourcePart::ChildWhole` query. Signature recovery and
declaration trailing recovery share one contiguous ordinal family. Source
queries validate semantic ordinals and applicability before expected source
identity. Omitted return, absent groups/visibility/public ID, default contract
mode, and delimiters that do not belong to an unbraced clause return the typed
role-not-applicable result rather than staging optional rows.

The return arrow is now parser-owned `ThinArrowNode` punctuation projected as
`AttachedRequiredPunctuation`. The former Rowan token-text search was deleted;
all callable producers emit the same typed punctuation owner.

## Accounting and transaction boundary

`HirThreadBody::try_new` enforces the shared inclusive
`HirLimit::ThreadFlowItems` maximum for Flow, ThreadExpression, and NestedScope
owners. Constructor evidence covers 65,536 accepted items and the typed
65,537-item rejection for every owner.

Whole-lowering exact/one-over rollback, deterministic retry, accepted-project
identity derivation, effect-catalog conflict checking, semantic callable
publication, and all downstream consumers belong to the following project and
public-switch cuts. This checkpoint does not claim those package rows complete.
No partial public reader, compatibility projection, source gate, or fallback
was added to bridge that boundary.

The loader/compiler/LSP ownership decision for that switch is recorded in
[Proof public-switch session ownership and publication decision](2026-08-06-proof-public-switch-session-ownership-decision.md).

## Validation

Validation recorded before commit:

- `cargo test -p arcweft-lang-syntax --all-features`: passed;
- `cargo test -p arcweft-lang-hir --all-features`: passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features`: passed with the
  pre-existing private-substrate warning inventory;
- `just test-workspace`: every recipe before the final CLI fixture binary
  passed. The final `arcw_fixtures_check_run` binary retained its established
  3-pass/2-fail public-switch baseline for
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`;
- direct execution of both failing fixtures reported
  `sema.nominal.unknown_type` for capability-owned `FsError`. The legacy
  public `ExternCapabilityItem` drops that member while the private final-HIR
  owner retains it. This checkpoint deliberately does not repair that reader,
  fabricate a global nominal, or add an alias/source gate; the deletion-driven
  public switch closes the frontier;
- Tier 2: not applicable to this private compiler-only checkpoint because no
  runtime, renderer, protocol, or Agent production authority changed;
- canonical structure audit: passed over 4,125 files, 2,250 Rust files,
  1,107,231 Rust physical LOC, and 95 manifests with 0 errors and 178
  repository-wide warnings; and
- `git diff --check`: passed.

Focused evidence additionally covers exact source-role applicability and
bounds, combined recovery ordinals, all retained poison relations, all sixteen
Flow body variants, shared body-owner limit parity, and parser-owned punctuation.

## Next deletion-driven cut

The next cut starts by deleting detached syntax entry points,
`lower_document_to_hir`, clone/linked-HIR accessors, and the selected legacy
SpeakerLine/ContentCall/HirDialogue authorities. Compiler errors are repaired
only toward retained `ParsedSource`, database-owned final HIR, the
module-preserving accepted project, and attached Dialogue application. No old
entry is restored to obtain an intermediate green build.
