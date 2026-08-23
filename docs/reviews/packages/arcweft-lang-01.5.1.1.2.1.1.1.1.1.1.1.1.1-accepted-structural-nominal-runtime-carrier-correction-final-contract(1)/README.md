# Accepted structural/nominal runtime carrier — final design contract

This ZIP is a **design-only** return for `2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction(1).md`. It contains no production overlay and makes no commit to the repository.

## Fixed evidence basis

- Repository: `Sanzentyo/arcweft`
- Basis ref: `origin/main`
- Complete Git SHA actually used: `UNAVAILABLE`
- Git decorations: `UNAVAILABLE`
- Working tree status after checkout: `(clean/no status output)`
- Repository acquired successfully: `false`
- Root/latest-main AGENTS files read in full: (none found / repository unavailable)
- Request SHA-256: `e9ead183b2bfd4d3019e8c3e51da79136bdae64d38aa5fe63ec4c92c1c948269`
- Premise SHA-256: `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1`
- Rust Skill SHA-256: `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`


## Normative result

The design introduces one runtime authority, `AcceptedRuntimeCarrier`, represented by an enum owned by the crate that already owns the runtime value/carrier enum. If current source already has that enum under another name, the implementation **extends that enum and its inherent `impl`**; it must not add an ad-hoc wrapper, extension trait, or side table merely to avoid editing the owner.

The carrier has exactly two semantic classes:

1. `Structural`: carries the canonical structural shape identity required by checked matching.
2. `Nominal`: carries the canonical nominal instance identity **and** its validated structural representation identity.

Nominal identity is never reconstructed from layout. Structural access to nominal representation is allowed only when checked lowering emitted an explicit projection witness. The same checked constraint is consumed by runtime execution, coverage closure, transcript production, persistence, and restore.

## Package map

- `01-evidence-basis.md` — current-main SHA, AGENTS scope, source anchors, and input hashes.
- `02-normative-decisions.md` — decisions D1–D24 and rejection of competing designs.
- `03-rust-api-and-owner-map.md` — concrete Rust types, inherent methods, owner/module map, and error taxonomy.
- `04-match-admission-and-coverage.md` — complete structural/nominal matrix and transcript/coverage closure.
- `05-persistence-byte-grammar-and-restore.md` — canonical grammar and two-phase restore.
- `06-runtime-task-awbc-integration.md` — task, Need, handle-batch, and allocation integration.
- `07-test-matrix.md` — executable test rows T1–T32.
- `08-implementation-sequence.md` — file-level change order and admission gates.
- `09-requirement-traceability.md` — request rows mapped 1:1 to concrete decisions/tests.
- `10-verification-boundary.md` — what was and was not actually run.
- `api-sketches/*.rs.txt` — non-production API sketches.
- `evidence/` — source search results, AGENTS copies, acquisition log, and validation logs.

## Closure state

`OPEN_QUESTIONS = 0`. Any name whose exact current-main spelling could not be proven is explicitly labeled **proposed**, while its semantic owner and migration rule are fixed. There are no generic `CLOSED` placeholders.
