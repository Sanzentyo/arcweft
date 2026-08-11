# Requirements traceability

| Requirement | Contract sections | Tests |
|---|---|---|
| ABI fixed at 1 | C3, `ABI1_OWNERSHIP_WIRE.md` | ABI-* |
| copied snapshot cannot activate twice in one domain | C4, activation document | ACT-* |
| allocator continues exactly after restore | C5 | ALC-* |
| prepared drop tied to exact value/slot | C6 | DRP-* |
| snapshot Eq conflict closed | C7 | SNP-* |
| View affine boundary decided per role | C8-C10 | VOW-* |
| View save uses dormant snapshots | C10 | SAV-* |
| authored static requirement wire authority | C11 | REQ-* |
| deterministic fragment overlap/dispatch | C12 | FRG-* |
| failure atomicity | C13 | ATM-* |
| deletion/no compatibility | C14 | DEL-* |
| full parent integration | implementation order | INT-*, T2-* |
