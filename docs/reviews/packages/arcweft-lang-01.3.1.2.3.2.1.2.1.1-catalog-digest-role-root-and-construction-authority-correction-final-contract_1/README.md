# Lang-01.3.1.2.3.2.1.2.1.1 final contract

Status: `READY_FOR_IMPLEMENTATION`  
Open questions: `0`  
Repository evidence head: `36f83f8509417d1110a34f1b32aee6f4a113dcf3` (`main`)  
Delivery mode: design-only; no production source, patch, overlay, branch, PR, compatibility reader, or implementation artifact is present.

This replacement directly answers the maintained request's required exact decisions 1–15. `REQUEST_DECISION_MATRIX.csv` contains exactly fifteen rows and points each requirement to a concrete type/API/table/byte-grammar owner. The prior invalid return's generic authority graph is not retained. All Arcweft-owned version markers remain `1`, all integer grammar in this package is little-endian, and the accepted `.1.2.1` generation body remains the single generation authority.

## Reading order

1. `FINAL_CONTRACT.md`
2. `REQUEST_DECISION_MATRIX.md` and `.csv`
3. `decision-01-*.md` through `decision-15-*.md`
4. the plan/AWBC/role/error CSV tables
5. inventories, tests, implementation order, and validation evidence

## Repository access boundary

An exact `git clone --depth=1 --branch main` was attempted. The execution container could not resolve/connect to GitHub, so a `.git` checkout could not be materialized. The source investigation continued through commit-pinned GitHub raw/rendered source at `36f83f8509417d1110a34f1b32aee6f4a113dcf3`, which the project instructions explicitly permit. `REPOSITORY_ACCESS_AND_SOURCE_EVIDENCE.md`, `SOURCE_EVIDENCE.csv`, and `git-clone-current-main.log` record the attempt and the actual evidence used. The current head commit changes only review/implementation documentation and retained-package intake, so the inspected production source captures are not displaced by that commit.

## Package integrity

`MANIFEST.sha256` covers every member except itself. `VALIDATION_EVIDENCE.md` distinguishes package checks that were actually run from production checks that require a future implementation checkout.
