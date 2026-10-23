# Validation evidence

## Performed and passed

- exact request copy/hash: **passed** — 11995 bytes,
  SHA-256 `2498106d805515f2fba326ef55685a8699aec2ab1abb986e22bc2f0a1f984cc6`;
- current-main evidence: **passed** — full Git SHA `80348beed0efa72db07f712122217b4e679e0a97`, exact commit
  patch reconstruction, and commit-pinned raw source captures;
- decision closure: **passed** — decisions 1–12 each have a named owner/API,
  normative mapping output, and D1–D12 test coverage;
- raw-construction/authority scans: **passed** — legitimate external lowerer
  builders are public and checked, final fields/wire DTOs are private, and raw
  plan/AWBC self-admission is absent;
- mapping-table reconciliation: **passed** — 35 audio fields,
  35 effect-owned AudioCommand rows, no
  `EffectPlan::AudioValue`, and 47 exhaustive expression
  variant/edge rows with root `[0]`;
- test matrix: **passed** — 2021 rows (1878
  retained-and-corrected plus 143 focused correction rows), unique
  IDs, and all contradiction scans passed;
- producer/consumer/deletion inventory: **passed** — 836 rows
  with core, runtime-plan, runtime-driver, compiler, verifier, VM, AOT,
  snapshot, restore/replay, dialogue, View, tests, and maintained documentation;
- implementation order: **passed** — 14 compile-clean phases,
  each with a same-phase deletion column and an explicit compile/test exit;
- design-only surface: **passed** — no production `.rs`, patch, overlay,
  nested archive, executable/library, unsafe path, symlink, or case-colliding
  member.

## Not run

Production `cargo check`, Clippy, unit/integration tests, restore/replay runs,
and structure audit were **not run** because this deliverable intentionally
contains no production checkout or patch. `ACCEPTANCE_COMMANDS.md` and
`TEST_MATRIX.csv` specify those future executable implementation gates; this
file does not claim them as passed.

## Evidence boundary

The direct container `git clone` attempt failed before checkout because DNS
could not resolve `github.com`. The package therefore makes no invented clean
working-tree claim. It records the actually performed full-SHA GitHub commit
patch reconstruction and exact raw-file hashes in `GIT_EVIDENCE.md` and
`SOURCE_EVIDENCE.csv`.
