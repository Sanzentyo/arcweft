# Requirement traceability

| Requirement | Decision owner(s) | Acceptance tests | Validator blocker(s) |
|---|---|---|---|
| `M1` — constructible reusable Join handle | FINAL_CONTRACT.md §1–2; NEED_HANDLE_AND_AWAIT_MANY.md; machine/contract.json need_handle; schemas/final_contract.rs | `H01–H15, S11` | `B01` |
| `M2` — rederivable AwaitMany request | FINAL_CONTRACT.md §3; NEED_HANDLE_AND_AWAIT_MANY.md; machine/contract.json await_many | `A01–A06` | `B02` |
| `M3` — one whole child-launch batch | FINAL_CONTRACT.md §4; BATCH_OBSERVER_AND_CANCEL.md; machine/contract.json batch | `B01–B08` | `B03` |
| `M4` — persistent observer allocator | FINAL_CONTRACT.md §5; BATCH_OBSERVER_AND_CANCEL.md; machine/contract.json observer_allocator | `O01–O08` | `B04` |
| `M5` — complete Host cancellation transaction | FINAL_CONTRACT.md §6; BATCH_OBSERVER_AND_CANCEL.md; machine/adapter_protocol.json | `C05–C10` | `B05` |
| `M6` — Sans-I/O dependency direction | HOST_ADAPTER_AND_LAYERING.md; SOURCE_DELETION_AND_CUTS.md; machine/adapter_protocol.json | `C01–C04,C11–C12` | `B06` |
| `M7` — snapshot schemas isomorphic to live carriers | SNAPSHOT_ISOMORPHISM.md; PROJECTION_REGISTRY.md; machine/live_snapshot_inventory.json; schemas/final_contract.rs | `S01–S14` | `B07,B08` |
| `M8` — constructible Match callable and role transcripts | MATCH_ROLE_TAG_CALLABLE.md; machine/match_roles.json | `M01–M12` | `B09` |
| `M9` — exact ownership carrier projection | OWNERSHIP_PROJECTION_MATRIX.md; machine/ownership_matrix.json | `P01–P12` | `B10` |
| `M10` — event ordering and restartable snapshot policy | EVENT_ORDER_AND_SNAPSHOT_POLICY.md; machine/contract.json event_order/snapshot_policy | `E01–E03,R01–R06` | `B11,B12` |
| `Cuts` — compile-clean five-cut sequence | SOURCE_DELETION_AND_CUTS.md; machine/compile_cuts.json | `K01–K04` | `—` |
| `Package` — source/deletion inventory, tests, validator | SOURCE_EVIDENCE.md; TEST_MATRIX.md; tools/validate_package.py; VALIDATION_OUTPUT.txt | `all rows; 12 validator self-tests` | `B01–B12` |

No requirement is closed by a bare `CLOSED` marker. Each row points to a concrete live/schema owner, machine contract, and test or validator failure.
