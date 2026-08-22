# Current source evidence

## 1. Repository identity

| Field | Observation |
|---|---|
| Repository | `Sanzentyo/arcweft` |
| Exact inspected Git SHA | `UNAVAILABLE (authenticated clone was not available in the execution container)` |
| Checkout form | `UNAVAILABLE` |
| Worktree state at capture | `UNAVAILABLE` |
| Evidence level | request-only design; current private source was not locally materialized |

No request statement is treated as proof that current source still has the same shape. Rows below are direct mechanical anchors from the inspected worktree when one was available. An anchor proves that text exists at that revision; it does not by itself prove semantic ownership, which must be confirmed by reading the enclosing Rust item before implementation.

## 2. `AGENTS.md` authority read

| Path | Lines | SHA-256 | Read status |
|---|---:|---|---|
| — | — | — | authenticated working tree unavailable; no local AGENTS.md claim made |

## 3. Likely current owner files

| Source path | Search score | Why it is in the inspection set |
|---|---:|---|
| — | — | no local repository materialization |

## 4. Symbol/line anchors

| Current-main anchor | Search key | Source line excerpt | Interpretation limit |
|---|---|---|---|
| — | — | No current-source anchor was locally available. | package does not claim source-level verification |

## 5. Normative owner placement selected by this design

These are the concrete implementation destinations. Where a current owner file already contains the relevant project-owned enum/type, extend that original definition and `impl` rather than creating an extension trait or side helper.

| Concern | Existing/source-derived anchor candidate | Normative placement rule |
|---|---|---|
| Coordinator and publication root | `crates/arcweft-runtime/src/task/coordinator.rs` | Add `RuntimeTaskCoordinator` restore ownership in the runtime task owner module; split a sibling `restore.rs` only if module size demands it. |
| Persistence journal and snapshot decode | `crates/arcweft-runtime/src/task/persistence.rs` | Extend the existing persistence/snapshot codec and version enum in place. |
| Runtime handle batch | `crates/arcweft-runtime/src/task/handle.rs` | Add prepared→published transition to the existing handle batch owner and enum `impl`. |
| Task plan seal/semantic children | `crates/arcweft-runtime/src/task/plan.rs` | Reuse the existing canonical semantic child encoder and seal verification API. |
| Generic match substrate/transcript | `crates/arcweft-runtime/src/task/match_substrate.rs` | Restore into a detached builder, seal it, and move it into the published root as one value. |

## 6. Source facts versus design decisions

- A **source fact** in this package is limited to the exact SHA, file hash, symbol/line anchor, or command output recorded in `10-verification.md`.
- A **normative design decision** is authoritative for the requested correction even when the exact destination filename must be reconciled with a source module discovered at implementation time.
- No production code was edited, staged, or committed while producing this package.
