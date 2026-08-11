# Searchable review package extraction migration — 2026-08-11

## Inspected state

- Baseline Git commit:
  `2585f527b02808305b3a8cab0442eb522e8d0352`.
- The working tree was clean and equal to `origin/main` before this migration.
- The validated working tree was dirty only with this documentation/package
  migration.

## Performed

- Moved 38 retained package ZIPs without changing their bytes from
  `docs/reviews/packages/` to `docs/reviews/packages/zips/`.
- Extracted their searchable contents to one sibling directory per ZIP basename.
  Fourteen archives had a single redundant top-level wrapper removed; the
  remaining member paths were retained.
- Moved five retained design ZIPs without changing their bytes into a `zips/`
  child of their existing sequence directory and extracted their searchable
  contents into that sequence directory. None required wrapper removal.
- Updated maintained review workflow documentation, current intake notes,
  requests, and links for moved retained archives.

## Passed

- Preflight: no absolute, rooted, drive-qualified, or parent-traversal member;
  no symlink/reparse member; and no case-insensitive member or destination
  collision across the 43 archives.
- Retained archive identity: every moved ZIP Git blob matches its pre-move
  `HEAD` blob.
- Extracted identity: all 855 files match their ZIP members by SHA-256.
  Packages account for 742 files and 7,149,693 uncompressed bytes; designs
  account for 113 files and 1,226,684 uncompressed bytes.
- Final longest absolute extraction path: 200 characters for packages and 120
  for designs. Repository-local `core.longpaths` remains unset.
- `git diff --check` passed for maintained documentation. A whole staged-tree
  check also reports producer-origin trailing whitespace in frozen extracted
  members; those bytes were intentionally preserved rather than reformatted.

## Preserved historical evidence

Extracted files are frozen mirrors and were not edited independently. Twenty-four
old `docs/reviews/packages/<archive>.zip` references remain inside returned
request-copy or ledger members: 18 in extracted design packages and six in an
extracted package. They describe the producer-time repository layout and remain
byte-identical to the retained ZIPs. One maintained request also names an older
archive that is not retained in this repository; its path was not rewritten to
a nonexistent `packages/zips/` target.

## Not run and non-goals

- Rust compilation, tests, lints, and structural audits were not run because no
  Rust, Cargo, generated product, or workspace structure changed.
- Archive contents were not semantically revised, reformatted, or regenerated.
- This migration does not change package readiness or implementation status.
