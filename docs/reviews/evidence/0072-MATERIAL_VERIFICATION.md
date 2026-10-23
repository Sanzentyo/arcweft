# Package material verification declaration

| Material | What was actually verified |
|---|---|
| `REQUEST_SPEC.md` | Byte-for-byte copied from the uploaded request; SHA-256 `5a318c3499ef3082aff829eafc00e9259b37bc200beb273ffa3c143dcb618065`. It is the sole normative task specification. |
| Current `main` | GitHub connector resolved and pinned `Sanzentyo/arcweft` `main` at `23ed5d93824630d8ead9092d32f7fc70f0a8f314` before inspection. |
| Repository policy | Complete `AGENTS.md` read at blob `ea4a46132ff8cd004f860c89c854e4cbfe807d86`. |
| Rust skill | Complete uploaded skill read; SHA-256 `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665`. |
| Repository source | The paths in `evidence/SOURCE_INVENTORY.csv` were inspected at the pinned revision. Findings are source-inspection evidence, not a Cargo build claim. |
| Prior Lang-01.4.2 ZIP | Archive and internal fallback notice inspected; SHA-256 `01f308c08fe818e247e41e94278eb2d69d5a12ac597794a9109390840c0d95d3`. Used for coordination only. |
| Contract decisions | Cross-checked against all seven required decisions, all eight required implementation phases, every explicit test family, and every constraint in `REQUEST_SPEC.md`. |
| Test matrix | Contractual/planned only. No production implementation exists in this package, so rows were not executed. |
| Repository commands | No local checkout was available/used; Cargo, `just`, and structure-audit commands were not executed. |
| ZIP integrity | Every internal file SHA-256 is recorded; archive CRC and internal hashes are verified during package construction. |
| Source modification | None. No repository file, branch, commit, issue, or pull request was created. |

No material in this ZIP should be interpreted as verified beyond the explicit boundary above.
