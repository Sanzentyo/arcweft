# Lang-01.3.1.2.3 final contract

This archive is the standalone, decision-complete design contract for the generic runtime ownership boundary required before Lang-01.3 P4+C1 can publish `RuntimeFunctionValue::ExternalStreamPartial` and `StreamHandle`.

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
CONTRACT_BASELINE=177ba1e61e43fb2da2149869ce35e165d1e93b66
PRODUCTION_PATCH_INCLUDED=NO
```

## Read order

1. `FINAL_STATUS.md`
2. `FINAL_CONTRACT.md`
3. `RUST_OWNERS_AND_APIS.md`
4. `STRUCTURED_RUNTIME_TRANSFER_SEMANTICS.md`
5. `AWBC_ABI2_OWNERSHIP_CONTRACT.md`
6. `SNAPSHOT_SAVE_RESTORE_CONTRACT.md`
7. `PLAN_HOST_REPLAY_PERSISTENCE.md`
8. `SUPERSESSION_DELTA.md`
9. `CONSUMER_AND_DELETION_INVENTORY.md`
10. `IMPLEMENTATION_ORDER.md`
11. `TEST_MATRIX.md`
12. `REQUIREMENTS_TRACEABILITY.md`
13. `REPOSITORY_EVIDENCE_AND_VERIFICATION_SCOPE.md`

`FINAL_CONTRACT.md` is normative. Rust snippets are exact target API shapes; responsibility-module splitting is allowed only when names, visibility, invariants, and dependency direction remain unchanged. The three predecessor Stream packages remain authoritative except for the explicit rows in `SUPERSESSION_DELTA.md`.

The selected result has one structural ownership lattice (`Unrestricted | Affine`), one opaque affine leaf token, exact typed capture and pattern-binding plans, checked copy plus consuming move/drop paths, one AWBC ownership state machine, dormant non-runnable snapshot evidence, and one non-Clone `RuntimePlan` shared as `Arc<RuntimePlan>` with an immutable block arena and one checked constant table. It adds no Stream-only runtime enum, ownership side table, copied environment, panic-on-`Clone`, compatibility reader, or source reconstruction path.

The package includes machine-readable contract/test inventories, all 803 predecessor test rows, an executable ownership-law reference model, and deterministic manifest/ZIP tools. It does not include a production patch and does not claim Cargo validation against a production checkout.
