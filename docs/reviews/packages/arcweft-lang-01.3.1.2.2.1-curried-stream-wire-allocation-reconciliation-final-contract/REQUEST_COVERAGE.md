# Request coverage

| Request item | Closed decision | Evidence in this package |
| --- | --- | --- |
| Required decision 1 | Non-conflicting opcodes | FINAL_CONTRACT.md §2; EXACT_WIRE_TABLE.md |
| Required decision 2 | Superseded rows and every removed opcode | NORMATIVE_DELTA.md; FINAL_CONTRACT.md §3 |
| Required decision 3 | Rust variants, fields, binary order, verifier, worked bytes | FINAL_CONTRACT.md §§4-6; EXACT_WIRE_TABLE.md; WORKED_BYTES.md |
| Required decision 4 | Parent identity mapping | FINAL_CONTRACT.md §7; RUST_SHAPED_OWNERS.md §1 |
| Required decision 5 | One callable owner and exact replacement | FINAL_CONTRACT.md §8; RUST_SHAPED_OWNERS.md §3 |
| Required decision 6 | Compile-clean P3-P8/C1-C6 interleave | FINAL_CONTRACT.md §10; IMPLEMENTATION_ORDER.md |
| Tests | All requested wire/verifier/limits/runtime/generator/identity/API cases | TEST_MATRIX.md/json/csv |
| Constraints | No redesign/compatibility/source gate/I/O inversion | FINAL_CONTRACT.md §§1,11; API tests |
| Expected output | Status, summary, manifest, delta, wire, Rust owners, order, tests, bytes | All present in archive |

`OPEN_QUESTIONS=0`; no fallback branch is used.
