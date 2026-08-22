# Decision register

| ID | Closed decision | Normative location |
|---:|---|---|
| D01 | Four child digests are opaque core-owned newtypes with owner-only hash construction and read-only bytes | `FINAL_CONTRACT.md` §2, `RUST_SCHEMAS.md` |
| D02 | Structured executable transcript has fifteen fixed table families and source-order roles | `TRANSCRIPTS.md` §3, `EXECUTABLE_TRANSCRIPT.md` |
| D03 | Task-plan base rows exclude map keys, completed/self/expected digests, and upper View payload | `TRANSCRIPTS.md` §7 |
| D04 | Final `RuntimeTaskPlan` field set is producer function, family, class, request template, control/effect ID, binding | `FINAL_CONTRACT.md` §4 |
| D05 | Binding tags are Ordinary=0, View=1, AwaitManyBase=2, AwaitManyChild=3, Timeout=4, Line=5 | `TRANSCRIPTS.md` §2 |
| D06 | Family/binding validation is an inherent exhaustive match on `NeedProducerFamily` | `schemas/final_contract.rs` |
| D07 | `RuntimeTaskPlanDigestBase` and View request are private-field, borrowed, non-Clone, nonserialized | `FINAL_CONTRACT.md` §5 |
| D08 | Build coordinates are owner-bound construction tokens; no caller raw constructor | `RUST_SCHEMAS.md` §2 |
| D09 | Existing `ViewTaskPlanAuthority` is the sole cross-layer protocol and returns completed typed digest | `FINAL_CONTRACT.md` §6 |
| D10 | Cross-crate digest finalization is one-use request-capability gated, not a public raw constructor/sink | `RUST_SCHEMAS.md` §1 |
| D11 | `ValidatedViewTaskPlanBinding` is owned by `ValidatedViewProgramResource` with actual View types | `FINAL_CONTRACT.md` §7 |
| D12 | Accepted View revision validates authority freshness but is excluded from digest | `TRANSCRIPTS.md` §8, `CYCLE_PROOF.md` §3 |
| D13 | Builder and private decoder invoke one common semantic encoder/sealer | `SEAL_STATE_MACHINES.md` |
| D14 | Ordinary-only `finish()` supplies no View authority and queries none | `FINAL_CONTRACT.md` §8 |
| D15 | Decoded expected keys are raw private assertions compared only after recomputation | `FINAL_CONTRACT.md` §8, `CYCLE_PROOF.md` §4 |
| D16 | Final table stores source-order sealed rows plus one global digest-to-index map | `RUST_SCHEMAS.md` §3 |
| D17 | Duplicate final digest rejects at the second source-order row across all families/bindings | `ERROR_PRECEDENCE_AND_LIMITS.md` §7 |
| D18 | Stale authority precedes missing binding; expected mismatch precedes duplicate on decode | `ERROR_PRECEDENCE_AND_LIMITS.md` §§5–6 |
| D19 | Exact default limits and atom accounting are fixed and not hashed | `ERROR_PRECEDENCE_AND_LIMITS.md` §§1–3 |
| D20 | Executable/task digest graph is acyclic and bounded | `CYCLE_PROOF.md` |
| D21 | Final row/table publication occurs only inside Cut 5 atomic switch | `COMPILE_CLEAN_SEQUENCE.md` |
| D22 | Every caller/self/raw-projection/parallel/generic-codec route is deleted, with no alias | `COMPILE_CLEAN_SEQUENCE.md` §4 |
| D23 | All Arcweft-owned version markers remain exactly one | all normative files |
| D24 | No production patch is contained in this archive | `README.md`, `SOURCE_INVENTORY.md` |
| D25 | `OPEN_QUESTIONS` is exactly `none`; status is implementation-ready | `FINAL_STATUS`, `OPEN_QUESTIONS` |
