# Proof revision-bound attached-syntax substrate cut

Date: 2026-08-03

Status: `PRIVATE_SUBSTRATE_VALIDATED_PUBLIC_SWITCH_PENDING`

Inspected baseline: `9591e1f7db4884cb796cf098ab027c9bd3155cf2`, which was
both local `main` and `origin/main` before this cut.

The main checkout contained a protected dirty Proof integration working set.
The files in this cut were copied byte-for-byte into a detached validation
worktree at the same baseline. Validation did not use the uncommitted final-HIR
or public-consumer changes that remain in the main working tree.

## Scope released by this cut

This cut establishes the private, revision-bound syntax authority required by
the Proof-concurrency public switch:

- one `ParsedSource`-bound attachment graph with database, lineage, snapshot,
  and node identity instead of detached source-string reconstruction;
- typed attached owners for declarations, expressions, patterns, statements,
  Flow/Thread bodies, Dialogue/RichText source components, and recovery;
- a final literal source owner shared by syntax and the minimal semantic,
  runtime-plan, and compiler literal consumers;
- one ordinary Pratt producer for Choice expressions, used by both direct
  Choice statements and `let ... = choice ...`;
- a flat, source-ordered Choice `if`/`else if` branch inventory, including a
  512-branch stack-safety fixture;
- bounded delimiter scans and a one-pass body-suffix classifier that preserves
  record-expression heads before required Choice bodies;
- forward-progress recovery for malformed Select branch terminators; and
- responsibility modules for expression projection, pending call/dialogue/
  record validation, and control forms, removing the error-level size of the
  former single expression-owner file.

The maintained grammar chapter and the ordinary-Flow redelivery intake record
the repository-local Choice owner decision. The returned ordinary-Flow archive
is retained with SHA-256
`BDC55671E7D4F8CDB3D07D8EC004672C90E14DEA88A47E63D8189E585BB3E4DF`.
The repository adjudication remains authoritative where it differs from that
archive, including the inclusive `HirLimit::ThreadFlowItems = 65,536` limit per
`HirThreadBody` and whole-transaction rollback at attempt `65,537`.

## Deletion-driven migration

The old syntax-side Dialogue application modules and opaque dialogue-text
reader are deleted in this cut. Their behavior is represented by the typed
attached source/component owners; no alias, wrapper, dual reader, source-text
reparse, compatibility spelling, source gate, CSS path, or Takumi path was
introduced.

The remaining production Flow/Choice/HIR readers are not repaired or extended.
They stay frozen only until the final-HIR/project replacement can carry every
consumer. At that public switch, the obsolete entry points and carrier types
are removed first and compilation failures are used as the migration inventory.
Deleted readers must not be restored to make an intermediate build green.

## Validation evidence

Passed in the detached exact-cut worktree:

- `cargo fmt --all -- --check`;
- `cargo check -p arcweft-lang-syntax --all-targets --all-features`;
- `cargo test -p arcweft-lang-syntax --lib expressions::projection_tests --all-features`
  (`1` passed);
- `cargo test -p arcweft-lang-syntax --lib attachment::expression --all-features`
  (`50` passed);
- `cargo test -p arcweft-lang-syntax --lib parser::expression --all-features`
  (`16` passed);
- `cargo test -p arcweft-lang-syntax choice --all-features` (`33` passed);
- focused malformed-Select and same-line Choice-boundary regressions;
- `cargo test -p arcweft-lang-syntax --lib --all-features` (`815` passed);
- `cargo test -p arcweft-lang-syntax --all-features`, including all unit,
  integration, `13` trybuild UI, and documentation tests;
- `cargo check --workspace --all-targets --all-features`; and
- `cargo clippy --workspace --all-targets --all-features` with exit status
  zero. The configured pedantic warnings remain visible; no `allow` was added
  to hide them.

An early full syntax-unit run was manually terminated after a malformed Select
branch failed to consume its semicolon and repeatedly emitted recovery nodes,
growing the process to approximately 22 GiB. The parser now consumes that
terminator after preserving its diagnostic range; the complete `815`-test
library run and full syntax-crate run passed afterward.

Push-cut validation used the same detached worktree and target directory:

- `just test-workspace` advanced through the workspace suites and stopped at
  the two previously recorded CLI capability fixtures
  `spec_should_pass/check/010_capability_fs_read.arcw` and
  `spec_should_pass/run/002_file_read_task.arcw`;
- an exact `cargo test -p arcweft-cli --test arcw_fixtures_check_run --
  --nocapture` reproduction yielded `3` passed and those same `2` failures;
  both still report `sema.nominal.unknown_type` for the capability-owned
  `FsError`, because the frozen public `ExternCapabilityItem` drops associated
  types while the private final attached owner already preserves them;
- no fixture rewrite, fabricated global type, final-HIR-to-old-HIR adapter, or
  repaired legacy reader was introduced to hide that public-switch frontier;
  therefore `just test-workspace` is explicitly not claimed as passing; and
- `just test-tier2` passed, including the MCP stdio, native observe/capture,
  object-ID/mask, animation, typewriter, ruby/text-combine, and exact visual
  golden suites.

The final staged-path review admitted the exact `241`-path validated snapshot
plus this implementation note and the repository-local Choice adjudication in
the ordinary-Flow intake. It excluded the later final-HIR/Thread-control and
public-consumer work that remains in the protected main working tree.

## Structural audit

The canonical command

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

scanned `3,927` files, `2,063` Rust files, `976,644` physical Rust lines, and
`95` manifests. It reported `0` errors and `161` warnings. Generated evidence
is under
`docs/implementation/structure-audits/proof-attached-syntax-substrate-2026-08-03/`.

The previous error-level `expressions.rs` hotspot is now `1,194` lines. This is
not claimed to be the final small-facade shape. In particular,
`expressions/dialogue.rs` remains a `1,216`-line warning and the attachment,
parser, and grammar owners listed by the audit remain explicit structural
follow-ups.

## Explicit non-goals and next authority boundary

This cut does not publish final HIR or switch compiler/LSP/runtime authority.
The next coherent slice is:

1. add the final `HirChoiceExpr` payload and
   `HirExprKind::Choice(HirChoiceExpr)`;
2. exhaustively lower direct and binding Choice through one `ExprId` owner;
3. complete shared Flow/Thread bodies, source rows, scopes, recovery, poison,
   deterministic accounting, and transactional project freeze;
4. make deeply nested Choice bodies iterative before the public switch; and
5. switch all project/compiler/sema/verifier/runtime-plan/formatter/LSP/CLI/
   Agent/cache/persistence consumers in one workspace-compiling authority cut,
   deleting the detached Flow/Choice AST, old `HirChoice`, legacy
   `SpeakerLine`/`ContentCall`/`HirDialogue`, linked-HIR readers, and other
   obsolete production entry points in that same cut.

Event-payload compaction and the remaining Dialogue candidate-search hotspot
are structural follow-ups, not completion claims for this private substrate.
