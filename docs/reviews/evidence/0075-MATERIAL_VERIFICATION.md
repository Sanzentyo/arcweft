# Material verification ledger

| Material | Verification level | Notes |
|---|---|---|
| supplied request | direct, complete | exact bytes included as `REQUEST_SPEC.md` |
| supplied premise | direct, complete | digest and applied coordination recorded |
| supplied Rust skill | direct, complete through final line | digest/line count recorded |
| latest `AGENTS.md` | direct, complete through final line | exact revision/blob recorded |
| current repository Rust/docs | direct connector inspection | exact commit and blob inventory recorded |
| prior Lang-01.5.1.2 ZIP entry inventory | direct central-directory inspection | 26 entries observed |
| prior family/Rust/revision contract entries | direct recovery/inspection | used for normative delta |
| prior outer ZIP/internal hash verification | repository-recorded | intake ledger is the evidence; not falsely claimed as locally recomputed |
| current GitHub CI status | direct query | no status contexts reported |
| production Cargo/Tier 2 | not executed | no checkout and no production change |
| this final ZIP | direct deterministic build and independent extraction | full log and internal SHA-256 list included |
