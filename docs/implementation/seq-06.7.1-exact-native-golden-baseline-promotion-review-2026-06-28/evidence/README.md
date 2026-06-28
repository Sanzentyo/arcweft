# Evidence

This directory contains all evidence available to the package generator.

The real pinned Windows review artifacts are intentionally absent and represented
with `.MISSING.md` / `.not-run.json` files. Do not treat these files as a
candidate baseline. They exist so the deferral is auditable.

Subdirectories:

- `candidate/` — candidate PNG gap marker.
- `observe/` — observe JSON not-run marker.
- `imq/` — historical seq06.6 drift metadata plus not-run marker.
- `environment/` — local packaging probe and required pinned-run schemas.
- `command-logs/` — local environment/source-inspection logs.
- `source-snapshots/` — policy, drift, and fixture source snapshots used by the review.
