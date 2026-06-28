# Seq06.7.1 exact native golden baseline promotion review

Date: 2026-06-28
Repository: `Sanzentyo/arcweft`
Inspected source revision: `b0b45b44b2dd34573d991839d950b58091c314b4`
Decision: **DEFER; do not promote `vertical_tutr_golden.png` from this package.**

This package is a concrete seq06.7.1 review packet. It intentionally contains no
`promotion-overlay/` directory because promotion is not recommended. The required
pinned Windows evidence was not produced in the package-generation environment:
there is no same-run candidate PNG, observe JSON, `imq` metrics JSON, or pinned
Windows environment fingerprint for a reviewed replacement baseline.

The package therefore documents the blocker rather than fabricating a candidate
or hiding the decision inside a blind PNG overwrite. It preserves the seq06.6
historical drift evidence, current policy, source fixture snapshot, command-log
probes, and an implementation note that tells the next maintainer exactly what
Windows run must be performed.

## Contents

- `REQUEST.md` — original seq06.7.1 request.
- `SOURCE_INVENTORY.md` — inspected repository inputs and hashes/SHAs where available.
- `REVIEW.md` — answers the required review decisions.
- `DECISION.md` — explicit defer decision and blocker.
- `IMPLEMENTATION.md` — package design and implementation notes.
- `evidence/` — available evidence, not-run markers, source snapshots, environment probes, and command logs.
- `scripts/collect-pinned-windows-review-evidence.ps1` — concrete collector for the next pinned Windows run.
- `docs/implementation/seq-06.7.1-exact-native-golden-baseline-promotion-review-2026-06-28.md` — repository-ready implementation note.
- `verification/VALIDATION.md` — actual validation performed and required next validation.
- `SHA256SUMS.txt` — checksums for all package files except the checksum file itself.

## Important limitation

This ZIP is not a promotion packet. It is a deferral packet with actionable next
steps. The missing same-run candidate PNG is represented as a gap under
`evidence/candidate/vertical_tutr_golden.candidate.png.MISSING.md` rather than as
a fake or copied image.
