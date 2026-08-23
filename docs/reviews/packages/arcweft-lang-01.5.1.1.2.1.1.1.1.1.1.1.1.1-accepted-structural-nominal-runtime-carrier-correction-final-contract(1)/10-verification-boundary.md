# 10. Verification boundary

## Actually verified

- All three supplied inputs were read byte-for-byte in full and their SHA-256 values are recorded.
- Repository acquisition/update commands and complete current-main SHA are recorded when access succeeded.
- Every `AGENTS.md` found in that checkout was read in full and copied under `evidence/AGENTS/`.
- Current source was searched for carrier, structural/nominal, match/coverage, snapshot/restore, task/Need/handle, catalog/digest, and AWBC anchors; exact grep rows are retained.
- The design package contains no production source overlay or repository mutation.
- ZIP contents, internal file hashes, and archive readability are verified by the package builder.

## Baseline commands run against unmodified current main

| Command | Exit | Runtime | Log |
|---|---:|---:|---|
| `(repository acquisition)` | 255 | 0 s | `validation/repository_acquisition.log` |

An exit other than zero is not hidden or relabeled. Read the corresponding log before attributing the result. These are **baseline/current-main checks**, not proof of an unimplemented design.

## Not claimed as verified

- The proposed production APIs have not been compiled because this return is design-only and deliberately contains no patch.
- T1–T32 are specified executable test rows, not represented as passing before implementation.
- Performance bounds are architectural (interned key comparisons/no hot-path allocation); no benchmark of unimplemented code is claimed.
- Cross-version compatibility is fixed by the grammar/version rules but requires implementation golden vectors before release.

## Validation classification

- **Source evidence:** verified only when repository acquisition succeeded and a path/line appears in `evidence/source-search-results.md`.
- **Design decision:** normative for the requested correction, but not current implementation evidence.
- **Proposed spelling/path:** may be renamed to current owner conventions; semantic ownership and invariants are not optional.
- **Future gate:** must pass after production implementation.
