# Repository and input evidence

## 1. Inspected repository state

```text
repository=Sanzentyo/arcweft
branch=main
inspected_commit=d8fbeaa5757fe5836fba17fca35fa104eeb72a1d
accepted_classifier_commit=b76465c128322be2d5e66398bc6c30794ca0276f
inspection_mode=GitHub web/raw read-only plus downloaded source snapshots
local_checkout_used=no
production_files_modified=no
```

At inspection time the repository and raw files were reachable without
authentication, although the supplied project premise describes the repository
as private. This package records the observed read-only state and does not infer
future visibility.

The inspected `main` is the documentation correction request commit directly
after the accepted classifier implementation in the relevant comparison. The
classifier commit remains the production baseline.

## 2. Governing inputs read completely

| Input | SHA-256 | Scope |
|---|---|---|
| source request | `dc9d39578e4706b7b518bc2cfdd37fda33d6be38352007c957e2360704afcf76` | complete 182-line correction request |
| project premise | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | Arcweft-first/latest-state instruction |
| Rust Skill | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | complete 56-line Rust guidance |
| root `AGENTS.md` | `90bae8bface6d390246538c60842da7d71d1ebd576ae3fa403019caa35a91498` | repository-wide authority |
| `crates/AGENTS.md` | `9dc887815ef05b1e7c2f63937926a908cdd7d9e36b916600fe9a278787966565` | crate/dependency/validation policy |
| `docs/AGENTS.md` | `40ef407718c7b6f44b1f79d8dcd92c562d3ff4649557ecfeff37a33d7fd86c2d` | docs/package policy |
| `docs/implementation/AGENTS.md` | `2874d75d223345eb8bbbf7d25b3474d4f8a9023ae7807acb7ed8ebbbe1b15da4` | implementation evidence policy |
| `docs/reviews/AGENTS.md` | `49d35db3276b2f2efe4d7e2343cf888455b610f5704a9607ec0acd93a3cb130a` | review/package policy |

Applied policy consequences:

- use one typed authority, not a wrapper/sidecar;
- replace obsolete paths directly;
- add behavior to the original enum/newtype through inherent implementations;
- preserve dependency direction;
- use typed/API/metadata evidence instead of source-text acceptance gates;
- record actual Git commit and validation limitations; and
- do not fabricate production test results.

## 3. Current production source snapshots inspected

| Path/area | Downloaded bytes SHA-256 | Relevant evidence |
|---|---|---|
| `crates/arcweft-core/src/value/ownership.rs` | `8eeffdec51690b65f386e9920006cae61d066c4689bbcae0e239ba468d536603` | shipped two-point classifier and exhaustive graph cases |
| `crates/arcweft-core/src/value.rs` | `64fefb678072e66af7294ecaec827b5ccaf72c6d32381da648520f1e4cba4386` | public name/value `RuntimeBinding`, record field, function captures, `RuntimeEnv` |
| `crates/arcweft-core/src/value/env.rs` | `775480ba479f0891035c08353aeb2b995b7abfe584be52b535ed18382d972b82` | nested name lookup, spare-scope reuse, clone/ref binding APIs |
| `crates/arcweft-core/src/value/nominal_record.rs` | `a41cdc9da415597461728be2b6a060c42c885e05a778b276fca37d94bddc7006` | schema-ordered value vector and current construction |
| `crates/arcweft-core/src/value/range.rs` | `b06843010877e1371813ed252f29a51dd5e013b2c0969b61c6b1d795af50a1d2` | iterator storage/index shapes |
| `crates/arcweft-core/src/runtime_id.rs` | `db8d082fec3ad3606f8960cf0f1925a4b99996f18fdb73386c79feeda7c15e21` | existing typed runtime-ID module/patterns |
| `crates/arcweft-core/src/awbc/schema.rs` | `3837106f5b571256ba56a040dbfd410a5cbcbff857b3d7c46df526c4620f1a09` | typed register IDs/current schema owner |
| `crates/arcweft-core/src/awbc/fiber.rs` | `868e911a0cd414e1212d0c71878d20f010a123acb18eafb68fe932b14e968c92` | current fiber/frame/register storage/snapshot surface |
| `crates/arcweft-lang-hir/src/scope.rs` | `c9722894187f8e6c137f2603e5d0fb2ed07f635daec47735939ac26c027cd1cd` | typed `LocalId`, `CaptureId`, generations, scope publication |

Repository trees and current driver/runtime-plan sources were also inspected for
producer/consumer placement, construction, persistence, replay, and dependency
direction.

## 4. Current-value graph evidence

The preserved classifier traverses:

- tuple values in stored vector order;
- ordinary sequence values in stored order;
- dense sequences as unrestricted leaves;
- tuple columns in stored column order recursively;
- record columns in stored field order recursively;
- anonymous records in field vector order;
- nominal records in accepted schema/value-vector order;
- function captures in capture vector order;
- variant payload;
- iterator `Values` only from current index to end;
- iterator witness state; and
- scalar/range leaves without children.

The final path contract mirrors this list and adds typed IDs/order; it does not
change current ownership outcomes.

## 5. Current identity gaps confirmed

The source request's exhaustive search result was rechecked against current
source/package evidence:

- no exact `ExecutionInstanceId` owner shared by core/driver;
- no typed `RuntimeRecordFieldId`;
- no stable dynamic `RuntimeLocalSlotId` or slot revision;
- no complete diagnostic slot union across structured/AWBC/mailbox/transfer/
  cleanup;
- no exact G1.2 transaction/evidence/limit/error owner set;
- no shared domain activation authority; and
- no defined affine-owner cursor continuation after restore.

Current names, vector positions, and AWBC register IDs are insufficient to fill
those gaps without the language/persistence decisions in this package.

## 6. Parent package evidence

Parent archive identities are supplied by the request. The separate
`arcweft-contract-audit-2026-08-10.md` in the user's file library reports that
the affine and View archives passed ZIP CRC/path/member, manifest, JSON/CSV, and
package model checks, while also identifying the activation, cursor,
prepared-Drop, and floating-snapshot defects corrected here.

The binary parent ZIPs themselves were not mounted in `/mnt/data` during this
artifact build. Therefore:

- this archive does not claim to have re-extracted or rehashed their members;
- parent SHA-256 identities are treated as authoritative request inputs;
- exact retained parent payload variants are referenced rather than
  reconstructed from memory; and
- the correction is narrow to the rows the request explicitly reopens.

This limitation does not leave a result-changing identity/slot decision open,
because the request and audit enumerate the unresolved symbols and preserved
parent decisions.

## 7. Current version evidence

Current inspected production contains typed AWBC register IDs and a production
ABI/codec owner. The request explicitly retains the parent affine/View ABI 1 /
codec 8 result, while the inspected production may contain unrelated later
codec evolution. This correction assigns no AWBC numeric value and records the
required implementation-time rebase rather than fabricating a downgrade or
dual format.

## 8. Evidence classification

| Claim class | Evidence available |
|---|---|
| request requirements/parent hashes | exact uploaded request bytes |
| repository policy | full downloaded AGENTS files |
| current value/env/HIR/AWBC structure | downloaded current-main source bytes |
| latest commit/classifier relationship | GitHub commit/compare inspection |
| parent mechanical archive checks | uploaded audit report |
| parent ZIP member-by-member inspection in this build | not performed |
| production Cargo/test/Clippy/Tier-2 | not performed; design-only |
| package mechanical/semantic checks | performed and recorded in `VALIDATION.md` |

## 9. No fabricated evidence

This file does not claim:

- a local Arcweft build;
- Rust compilation of the target declarations;
- production implementation;
- a branch, patch, or PR;
- parent ZIP extraction in this artifact environment;
- test pass counts from current production; or
- G1.2 runtime behavior already present.

Those are implementation-time gates in `IMPLEMENTATION_ORDER.md`.
