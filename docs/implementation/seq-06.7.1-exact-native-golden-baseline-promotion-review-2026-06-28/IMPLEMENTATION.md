# Implementation

This package implements seq06.7.1 as a review/decision artifact, not as a Rust
production-code change.

## Design

The package uses a conservative promotion gate:

1. Treat the current policy and seq06.7 implementation note as source of truth.
2. Treat historical seq06.6 `baseline_drift` metrics as prior evidence only.
3. Require same-run pinned Windows artifacts before any PNG promotion decision.
4. When same-run artifacts are missing, produce a deferral packet with explicit
   blocker metadata instead of copying or fabricating PNG bytes.
5. Include no `promotion-overlay/` directory unless promotion is recommended.

## Implemented package contents

- Copied the request into `REQUEST.md`.
- Recorded current-main source inventory and connector SHAs in `SOURCE_INVENTORY.md`.
- Preserved current policy/drift/source snapshots under `evidence/source-snapshots/`.
- Added machine-readable not-run evidence for candidate, observe, `imq`, and environment gaps.
- Added command logs for local environment probes and source inspection.
- Added a PowerShell collector script for the next real pinned Windows run.
- Added `docs/implementation/seq-06.7.1-exact-native-golden-baseline-promotion-review-2026-06-28.md`.
- Added package verification instructions and checksums.

## What was not implemented

- No candidate PNG was generated.
- No PNG baseline was overwritten.
- No `promotion-overlay/` directory was added.
- No threshold was changed.
- No repository code was modified.

These omissions are intentional because the required pinned Windows run was not
available in the package-generation environment.
