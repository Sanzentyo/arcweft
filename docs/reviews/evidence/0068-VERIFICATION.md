# Verification record

## Artifact scope

This is a design-only package. It distinguishes:

- **actually verified now**: complete byte reads and hashes of the uploaded request, premise, and Rust Skill; request structure extraction; AGENTS reads when materialized; repository Git/source inventory when materialized; internal contract/test/mapping consistency; ZIP CRC and hashes;
- **specified for implementation admission**: production edits and repository-wide Rust command success. These are not represented as already executed by this design return.

## Input reads

| Input | Bytes | Lines | SHA-256 | Read through EOF |
|---|---:|---:|---|---|
| `2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.2-generic-match-complete-transcript-and-coverage-closure-correction(1).md` | 9780 | 178 | `981158fd20afcc41e737604f7c94ea2d56e455f7df2026d1a16a8c7994ac9628` | yes |
| `前提(Sanzentyo-arcweft).txt` | 250 | 1 | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | yes |
| `Rust Skill.txt` | 5045 | 57 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | yes |

The Rust Skill was read through its final line. Its full text is not duplicated into the deliverable; this table records the exact consumed bytes.

## Repository baseline

- Repository path: `/mnt/data/arcweft-current`
- Local Git worktree available: `False`
- Local source tree available: `False`
- `HEAD`: `not materialized`
- `origin/main`: `not materialized`
- Clean working tree: `False`
- Verification tier: `V3_REQUEST_AND_CONTRACT_ONLY`
- Commit metadata:

```text
not materialized
```

## AGENTS.md

Files read through EOF, root-to-leaf:

- No AGENTS.md file was locally materialized. Fetch attempts are preserved; the design does not falsely claim source verification.

The exact read text is preserved in `evidence/AGENTS-read-completely.md` when available.

## Request coverage checks

- Parsed headings: 8
- Parsed numbered items: 11
- Strict requirement sections preserved: 3
- Concrete decisions: 28
- Executable test rows: 40
- Every parsed numbered item has at least one decision: True
- Every parsed numbered item has at least one test row: True
- `OPEN_QUESTIONS=0`: yes

## Production verification boundary

No production code is included or modified. Therefore `cargo fmt`, `cargo check`, `cargo test`, and `cargo clippy` are normative implementation-admission commands, not claimed as completed production verification in this ZIP. The package instead provides exact APIs, state transitions, byte grammar, owner mapping, diagnostics, migration, performance constraints, and GM-T001–GM-T040 oracles needed to implement and verify the correction.
