# Repository evidence

## 1. Exact identities

```text
REPOSITORY=Sanzentyo/arcweft
ACCESS=authenticated GitHub connector, read-only
REF=main
GIT_COMMIT=15cf571416245e1530c0d9902ab3ff6befbdb39e
GIT_SUBJECT=Publish accepted AW-AH-009.3 contract chain
GIT_COMMIT_TIME=2026-07-24T02:02:35Z
JUJUTSU_CHANGE_ID_EVIDENCE=zzrlxnsunyxl
LOCAL_GIT_WORKTREE=NO
LOCAL_JJ_STORE=NO
```

The exact Jujutsu ID above is the repository-authored protected integration
change cited by current-main intake evidence. It is the Jujutsu identity used
as repository evidence in this assignment. No claim is made that it is the
Jujutsu change ID currently mapped to Git commit `15cf571416245e1530c0d9902ab3ff6befbdb39e`, because the GitHub
connector exposes Git objects and no local `.jj` store was available.

## 2. Supplied evidence hashes

| Input | SHA-256 | Verification |
|---|---|---|
| sole correction request | `3f37ef7f45dd69cfe7ed70470e943a33ae4824569158243cba1cead38ba65e5e` | Read completely; sole request authority. |
| accepted AW-AH-009.4.1.2 ZIP | `cb087cc2e4e137edde1732c11df579a1c71371769633bfdcf807fd367b30fdc1` | Opened, ZIP-tested, internal manifest checked. |
| Rust skill | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | Read completely through final line. |
| Arcweft premise | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | Read completely. |

The prerequisite hash equals the hash recorded by
`docs/implementation/2026-07-20-aw-ah-009-4-1-2-tts-provider-intake.md` at the
inspected main commit. All 16 ZIP entries were readable. The 15 non-manifest
content members matched exact declared byte lengths and SHA-256 values.

## 3. Applicable instruction evidence

| Path/evidence | Identity | Finding |
|---|---|---|
| `AGENTS.md` | blob `e91f99213dde67953beda6aa078c370a8dc4541d` at current main | Core runtime/data-only; lower-to-higher edges; typed APIs; inherent owner behavior; no ad hoc wrappers/extension traits; direct replacement; no source gates; Tier 2 for broad runtime. |
| `crates/AGENTS.md` | absent at current main | Root `AGENTS.md` is applicable. |
| `crates/arcweft-core/AGENTS.md` | absent at current main | Root `AGENTS.md` is applicable. |
| supplied Rust skill | SHA above | Read completely; production Rust rules used for design. |

## 4. Repository owner evidence ledger

All paths are pinned to Git commit `15cf571416245e1530c0d9902ab3ff6befbdb39e`.

| Path | Blob/evidence | Finding used |
|---|---|---|
| `Cargo.toml` and relevant crate manifests | connector read at commit | Current workspace owners/dependency direction; no TTS runtime bridge crate. |
| `crates/arcweft-core/src/task.rs` | `b2d7442b7e867ef6cf65450881b285dd48d01fdd` | Provisional `TtsRequest`, TTS request variant, TaskClass, policy, string error; direct deletion/correction target. |
| `crates/arcweft-core/src/engine/suspend.rs` | `bd0ec4fe1d14391c3bec5abe900bb9d1d80d9073` | Hard-coded `synthesize|synthesis`/`voice` branch and current Await error collapse. |
| `crates/arcweft-core/src/value.rs` | `25ee59e63f9354d357d283f067ab1123804b0d89` | `RuntimePayload`, `RuntimeValue::NominalRecord`, exact Variant shape. |
| `crates/arcweft-core/src/value/nominal_record.rs` | `00437bc01ae1d9e4c21a1afa909f3542da4f3bd4` | Type/layout/ordinal carrier and public unchecked constructor to tighten. |
| `crates/arcweft-core/src/entry/schema.rs` | `2a46608aa13a41926733839f23124efbb07330d5` | Exact layout hash algorithm, canonical encoder, tags 14/15, current limits. |
| `crates/arcweft-core/src/step.rs` | `92a34b177b7474ccaafaaa8922b6d332fb535c95` | HostRequestBatch/tasks and typed RuntimePayload host-call surfaces. |
| `crates/arcweft-core/src/awbc/schema.rs`, codec, VM, product mapping | connector read at commit | AWBC ABI 1/codec 7, string task plan, `MakeRecord` ignoring nominal type, exact direct-replacement seam. |
| `crates/arcweft-runtime-plan/src/host_request.rs` and AWBC lowering | connector read at commit | Existing task-template lowering owner. |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `484dc6ad790a1a194cc91293700799024adc411b` | Shared callable identity enum and correct owner for TTS IDs. |
| `crates/arcweft-runtime-driver/src/session.rs` | connector read at commit | `BundleSession::dispatch_requested_tasks` current publication seam. |
| `crates/arcweft-runtime-driver/src/task.rs` | `2a53ed2265a5b224e79d7ca140059d67cdb50a4c` | Registry/dispatch/generation/sequence owners and current string `failed` helper. |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | `31ea6d77f232f42fb8c158a3eb7900b8a758536c` | Sole scheduler, key join, joined fan-out, current scope cancellation. |
| `crates/arcweft-host-adapter/src/lib.rs` | connector read at commit | Current registry ownership seam; string operation branch must not be restored for TTS. |
| `crates/arcweft-runtime-host/src/native_task.rs` | connector read at commit | Privileged provider-I/O pump owner. |
| `crates/arcweft-runtime-driver/src/session_save.rs` | connector read at commit | Existing `HostTasks` and `TaskGenerationPins` blockers. |
| `crates/arcweft-runtime-driver/src/session/replay/model.rs` | `ec1d3f81532c2f0a5a16c1dc0224191d54929f59` | Replay schema 1, one external outcomes vector, string failure defect. |
| `crates/arcweft-bundle/src/container/identity.rs` | `743944deedc33037fa115137a48aec3eb0890349` | Artifact identity owner used by accepted lower model. |
| `docs/01-language/await-need-result.md` | `cc9c66524c162d62a1a8a173dd0419deeeeb614f` | Need state and ordinary await/Result/Try semantics. |
| accepted AW-AH-009.4.1.2 package | supplied ZIP/hash above | Lower identities, selectors, catalogs, selected request, fingerprint, provider protocol, limits, errors, save/replay/reload acceptance rows. |

## 5. Current-main reconciliation findings

Verified current substrate:

- core has no audio Cargo dependency but contains provisional stringly TTS data
  and branch logic;
- core generic task and host errors are still stringly at the affected points;
- nominal RuntimePayload and layout identity already exist;
- AWBC currently cannot preserve a public nominal record through `MakeRecord`;
- runtime-driver publishes generation/sequence/registry after core tasks but has
  no typed preparation stage;
- one generic scheduler already owns joining and event fan-out;
- save blockers and replay schema-1 external outcomes already exist;
- shared callable identity/resolver owner exists;
- accepted lower TTS implementation is not present on current main and must be
  implemented before/with this reconciliation.

Selected design decisions, not current-production claims:

- new bridge crate and its layout hashes;
- generic core intent/outcome carriers and codec decoder;
- AWBC codec 8;
- atomic TTS preparation transaction;
- typed registration/host dispatch;
- joined-observer cancellation correction;
- schema-1 typed task external outcomes;
- all new diagnostics and test rows.

## 6. Work not performed and validation boundary

This design-only assignment changed no repository file. No local checkout was
mounted, so no Cargo, `just`, native/Web/headless/Agent, code-generation,
fixture, or Tier 2 command was run. No production test result is claimed.

Actually verified here:

```text
request read/hash = yes
Rust skill read/hash = yes
root AGENTS read at exact Git commit = yes
accepted prerequisite ZIP open/test = yes
accepted manifest member hashes/sizes = 15/15 pass
current main repository owner inspection = yes, read-only connector
layout hashes = independently recomputed from the inspected core canonical
                schema algorithm using a local pure BLAKE3 implementation
output archive member hashes/sizes/ZIP integrity = performed after generation
```

Implementation commands and behavioral results remain future implementation
evidence and are explicitly listed in the handoff/matrix rather than fabricated.
