# Applicable AGENTS and Rust policy

## Inspected policy identities

The supplied Rust Skill was read completely through its final line before this contract was written.

| Policy input | Lines | Bytes | SHA-256 | Inspection |
|---|---:|---:|---|---|
| supplied `Rust Skill.txt` | 56 | 5,045 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | complete |
| current raw `AGENTS.md` | 106 | 5,412 | `90bae8bface6d390246538c60842da7d71d1ebd576ae3fa403019caa35a91498` | complete |
| current raw `crates/AGENTS.md` | 134 | 6,943 | `9dc887815ef05b1e7c2f63937926a908cdd7d9e36b916600fe9a278787966565` | complete |
| current raw `docs/AGENTS.md` | 51 | 2,247 | `40ef407718c7b6f44b1f79d8dcd92c562d3ff4649557ecfeff37a33d7fd86c2d` | complete |
| current raw `docs/reviews/AGENTS.md` | 65 | 3,569 | `49d35db3276b2f2efe4d7e2343cf888455b610f5704a9607ec0acd93a3cb130a` | complete |
| current raw `docs/implementation/AGENTS.md` | 31 | 1,866 | `2874d75d223345eb8bbbf7d25b3474d4f8a9023ae7807acb7ed8ebbbe1b15da4` | complete |

The current raw files were retrieved from `main` on 2026-08-10 (Asia/Tokyo). The transport exposed their exact bytes but not the full Git commit to which the moving branch resolved. Therefore their byte identities are evidence of the policy actually read, while implementation Stage 0 still obtains and records one full current-main Git SHA.

The current `docs/README.md`, `docs/reviews/README.md`, crate map, test-execution policy, structural-audit policy, maintained ownership/proof/runtime/Stream chapters, and targeted runtime-value production files were also inspected. `REPOSITORY_EVIDENCE_AND_VERIFICATION_SCOPE.md` separates commit-pinned request evidence from moving raw-main observations.

## Applied rules

The contract follows the Rust Skill by selecting strong typed ownership, narrow visibility, explicit `Result` failures, deterministic transitions, small compile-clean increments, no `unsafe`, no panic-based ordinary failure, direct behavior tests, and exact fmt/check/Clippy/test evidence. Proposed Rust snippets are target contracts, not claims that production compiled.

The current repository policies require all of the following, and this package applies them directly:

- preserve layer direction and keep core/data-format owners Sans I/O;
- use one final typed authority and directly replace unreleased internal contracts;
- delete obsolete success paths instead of repairing them or retaining compatibility surfaces;
- add missing domain behavior to the Arcweft-owned enum/boundary type or its legitimate context through inherent methods;
- do not add scattered match helpers, extension traits, source-string reconstruction, copied side tables, fallback resolvers, parallel environments, or Stream-only value/register models;
- treat typed behavior, codecs, compile checks, lints, deterministic artifacts, Cargo metadata, and structured dependency evidence as acceptance evidence rather than source spelling;
- use Git as the sole VCS authority and record a full Git SHA, not a Jujutsu identity;
- distinguish passed, failed, blocked, and not-run validation, and never promote a planned command into evidence;
- use the canonical `just structure-audit` and `just structure-audit-gate` commands when public contracts, duplicated boundaries, or crate ownership change; and
- run the matching Tier 2 matrix for this broad multi-crate runtime/public-contract cut.

Accordingly this package extends the original `RuntimeValue`, `RuntimeFunctionValue`, sequence, `StreamHandle`, RuntimePlan, AWBC, fiber, and save owners in place. It defines no ad hoc ownership helper layer, extension trait, compatibility reader, or parallel Stream-specific runtime value model.

## Remaining intake duty

No production checkout was mounted and no production file was edited. Stage 0 must fetch current `main`, record its full Git SHA and dirty state, re-read the applicable policy files from that checkout, and reconcile path movement. This is an evidence/placement gate, not an unresolved ownership decision. A result is reopened only for a concrete accepted-production conflict that changes observable behavior.
