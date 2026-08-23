# 01. Evidence basis

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

## Current-main owner anchors

| Concern | Current source anchor selected by symbol search | Status |
|---|---|---|
| runtime value/carrier owner | `crates/<runtime-owner>/src/value.rs (new module only if no owning enum exists)` | observed candidate or explicit proposed fallback |
| checked match executor/plan | `crates/<runtime-owner>/src/match_exec.rs` | observed candidate or explicit proposed fallback |
| checked type/nominal owner | `crates/<language-owner>/src/checked/type.rs` | observed candidate or explicit proposed fallback |
| snapshot/restore owner | `crates/<runtime-owner>/src/snapshot.rs` | observed candidate or explicit proposed fallback |
| task/Need/handle owner | `crates/<runtime-owner>/src/task.rs` | observed candidate or explicit proposed fallback |

The exact grep rows are retained in `evidence/source-search-results.md`; this table does not silently promote a proposed fallback into an observed path.

## Workspace packages observed

(cargo metadata unavailable; see validation logs)

## Request headings read

- Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1 — accepted structural nominal runtime carrier correction
-   Parent, split reason, and precedence
-   Mandatory redispatch inputs and repository preflight
-   Decisions required
-   Consumers to inventory
-   Non-goals
-   Required implementation order
-   Required tests
-   Required returned archive

## Evidence discipline

The request document is a requirement source, not evidence that current source already implements it. Source observations in this package come only from the checked-out SHA and command logs. Proposed API names are marked as such.
