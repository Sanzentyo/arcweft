# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2 accepted design

Status: `READY_FOR_IMPLEMENTATION`

Inspected production commit:
`9168c8ac7285c6b44f29018626a0e7c1b0059796` (`main == origin/main`; the
working tree contained unrelated untracked review intake and package files,
which this design preserved).

This directory resolves the focused
[`current host scheduler and Sans-I/O restore transaction correction`](../../requests/2026-08-23-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2-current-host-scheduler-and-sans-io-restore-transaction-correction.md).
It rejects the returned coordinator package's durable WAL and global
coordinator model. It retains the accepted parent design's core journal,
concrete adapter token, and last-fallible-apply ordering.

The normative documents are:

- [REQUEST.md](REQUEST.md) — byte-identical maintained request mirror;
- [FINAL_DESIGN.md](FINAL_DESIGN.md) — selected owners and behavior;
- [SCHEMAS.md](SCHEMAS.md) — exact Rust-shaped public and private seams;
- [TRANSACTION_AND_STATE_PROJECTION.md](TRANSACTION_AND_STATE_PROJECTION.md) —
  operation state machine, event/outcome projection, and error precedence;
- [DEPENDENCIES.md](DEPENDENCIES.md) — crate direction and predecessor gates;
- [CUTS_TESTS_AND_DELETION.md](CUTS_TESTS_AND_DELETION.md) — compile-clean
  implementation order, tests, and deletion inventory;
- [SOURCE_EVIDENCE.md](SOURCE_EVIDENCE.md) — current source and accepted parent
  evidence;
- [DECISION_REGISTER.md](DECISION_REGISTER.md) — closed decision ledger;
- [machine/final_contract.json](machine/final_contract.json) — machine mirror;
- [MANIFEST.sha256](MANIFEST.sha256) — package-member hashes;
- [VALIDATION_REPORT.md](VALIDATION_REPORT.md) — performed validation;
- [FINAL_STATUS.md](FINAL_STATUS.md) and [OPEN_QUESTIONS.md](OPEN_QUESTIONS.md)
  — readiness authority.

The package validators are read-only with respect to the repository and design
folder:

```powershell
cargo +nightly -Zscript tools/validate_design.rs --repository-root ../../../..
cargo +nightly -Zscript tools/negative_self_tests.rs
```

`READY_FOR_IMPLEMENTATION` means every result-changing choice in this
correction is closed. Production implementation remains ordered after the
accepted task-plan/View/Match/nominal predecessors listed in
[DEPENDENCIES.md](DEPENDENCIES.md). Those dependencies are consumed only
through `TaskValidationAuthority`, `RuntimeSnapshotAuthority`, and
`ViewTaskPlanAuthority`; this design creates no placeholder upper-layer type.

No production source, test, Cargo manifest, branch, worktree, commit, or push
is part of this design cut.
