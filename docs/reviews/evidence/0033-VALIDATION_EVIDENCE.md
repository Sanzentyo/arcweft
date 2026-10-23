# Validation evidence and verification boundary

## 1. Material actually read

The design investigation read:

- `/mnt/data/Rust Skill.txt` in full;
- `/mnt/data/前提(Sanzentyo-arcweft).txt` in full;
- the supplied Lang-01.3.1.2.3.2.1.2.1 request in full;
- the supplied returned `.1.2` ZIP and its normative files;
- root `AGENTS.md`;
- scoped `crates/AGENTS.md`, `docs/AGENTS.md`,
  `docs/reviews/AGENTS.md`, and implementation guidance;
- docs/review indexes, crate map, and test-execution policy;
- exact pinned current source/trees and accepted G1 source evidence listed in
  `SOURCE_EVIDENCE.md`.

## 2. Parent artifact verification actually performed

Supplied parent:

```text
arcweft-lang-01.3.1.2.3.2.1.2-nominal-runtime-value-external-admission-and-dialogue-layout-authority-correction-final-contract.zip
```

Actual SHA-256 recomputed in this environment:

```text
7a7001cba41f312d428a88589877ce48eb3bb6734aff234b72601d7bfa6a9d70
```

Required SHA-256:

```text
7a7001cba41f312d428a88589877ce48eb3bb6734aff234b72601d7bfa6a9d70
```

Result: `PASS`.

The ZIP compressed-data test and its internal `MANIFEST.sha256` verification
also passed.

## 3. Repository evidence boundary

Exact immutable source was inspected through GitHub raw/tree retrieval at:

- `50771a19f57f86570837f616a66252be24e77e0c`;
- accepted G1 commit
  `1648894fbfc38ba623d1b01c6001fbd55b67b10b`;
- `.1.2` production parent
  `98ccafa5f0113a50f8a0f5e985df5f695c401588`.

This execution environment did not hold a local Git checkout. Therefore this
package does **not** claim to have run:

- `git status`/clean-tree checks;
- Cargo check/test;
- rustfmt;
- Clippy;
- nextest;
- structural audit;
- Miri;
- coverage;
- Tier 2 implementation gates.

Those commands are normative implementation acceptance requirements in
`IMPLEMENTATION_ORDER.md`; no result is represented as green here.

## 4. Design-only boundary

The output contains Markdown, CSV, JSON, text status files, the source request,
and a SHA-256 manifest only.

It contains no:

- `.rs`, `.toml`, `.patch`, `.diff`, `.rej`, or production build file;
- branch/PR metadata;
- implementation overlay;
- compatibility reader/writer;
- external sidecar.

## 5. Package checks performed by the builder

The final builder verifies:

1. `SOURCE_REQUEST.md` is byte-identical to the supplied request;
2. `OPEN_QUESTIONS.txt` is exactly `OPEN_QUESTIONS=0\n`;
3. parent ZIP SHA and internal manifest;
4. JSON parse and fixed metadata assertions;
5. CSV parse, unique IDs, and required row counts;
6. no forbidden production/patch extensions;
7. every Arcweft-owned version decision remains `1`;
8. sorted SHA-256 manifest generation for every file except the manifest;
9. deterministic ZIP entry order, timestamp, permissions, and compression;
10. extraction into a fresh directory;
11. extracted manifest verification;
12. second deterministic ZIP generation and byte comparison;
13. final archive SHA-256.

Concrete final results are appended after archive construction.

## 6. Executed package-builder results

- source request byte identity: `PASS`
- exact `OPEN_QUESTIONS=0`: `PASS`
- parent ZIP SHA-256: `PASS`
- parent compressed-data test: `PASS`
- parent internal manifest: `PASS`
- `contract.json` parse/metadata: `PASS`
- inventory CSV rows: `154` (`PASS`)
- test CSV rows: `320` (`PASS`)
- duplicate inventory/test IDs: none (`PASS`)
- Markdown code-fence balance: `PASS`
- production/patch extension scan: `PASS`
- version-1 decision assertions: `PASS`
- final file count including manifest: `28`
- manifest generation and fresh-extraction verification: `PASS`
- deterministic ZIP reproduction: `PASS`

The final archive SHA-256 is intentionally reported outside the archive, because
embedding an archive's own hash would change the archive bytes.
