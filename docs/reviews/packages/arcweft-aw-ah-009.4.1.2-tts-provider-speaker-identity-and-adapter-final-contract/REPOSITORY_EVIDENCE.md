# Repository evidence

## 1. Inspection identity

```text
repository=Sanzentyo/arcweft
ref=main
commit=e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139
inspection_mode=authenticated GitHub connector, read-only
local_checkout=none
production_changes=none
cargo_or_just_execution=not run
```

`main` advanced during the audit. The inspection was repinned after each
advance and reconciled against the final tip above. A final read-only recheck
resolved `main` to the same commit; that pin is the sole repository basis of
this archive.

## 2. Supplied inputs

| Input | SHA-256 | Inspection |
|---|---|---|
| `2026-07-20-aw-ah-009.4.1.2-tts-provider-speaker-identity-and-adapter-contract.md` | `4cd740e664528ac2a033f02245e6e0c5f4d887fdfdbbb877584b9fe742727b99` | Read completely; sole request authority. |
| `Rust Skill.txt` | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | Read completely through final line. |
| `前提(Sanzentyo-arcweft).txt` | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | Read completely. |

## 3. Repository evidence ledger

All paths below were read at `e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139` unless noted. A blob SHA is included
where the connector returned it directly.

| Path | Blob SHA / evidence | Finding used by this contract |
|---|---|---|
| `AGENTS.md` | `ea4a46132ff8cd004f860c89c854e4cbfe807d86` | Layering, Sans-I/O, direct replacement of unreleased APIs/wires, typed structural evidence, no source gates, and owning-type behavior. |
| `Cargo.toml` | `1e2cf9174ed3f6a3b82ff41351f7759f8b66e58e` | Workspace has audio core/codec/mixer/device but no `arcweft-audio-tts`; Rust 1.96, edition 2024. |
| `crates/arcweft-id/src/lib.rs` | `1853c6d02de44b7e5fc8c4e763dbdd000f777f19` | `PublicId` provides generic checks but no TTS family or size bound; nominal wrappers must validate their own families. |
| `crates/arcweft-character/src/id.rs` | `fefbd574e637a631ea0785e14649596e106a516c` | `CharacterId` is exact `character.*` narrative identity and must remain distinct. |
| `crates/arcweft-audio-core/src/graph.rs` | `8ea9e88d545d9db9bfa009bc446fdf9f0bf8c688` | Existing `AudioFormat`, graph, asset, bus/effect boundaries; no TTS provider model. |
| `crates/arcweft-audio-core/src/prepared.rs` | inspected at commit | Complete decoded-audio/prepared-command boundary is suitable downstream substrate. |
| `crates/arcweft-audio-codec/src/lib.rs` | `a59dcd423fb8225526cf9521f0984b2e1b9d3210` | Complete encoded bytes are decoded off callback; mono/stereo and duration limits are validated. |
| `crates/arcweft-audio-mixer/src/lib.rs` | `24cd988a73a21b3a28aca4edfa3570b1accf4f8c` | Mixer consumes `DecodedAudio` and owns no decoder/device; no need to redesign for TTS. |
| `crates/arcweft-audio-device-cpal/*` | inspected at commit | Physical output is a platform adapter and is not provider-speaker identity. |
| `crates/arcweft-interaction-model/src/audio.rs` | inspected at commit | Audio resource/playback voice/bus IDs are already nominally separate. |
| `crates/arcweft-core/src/task.rs` | `b2d7442b7e867ef6cf65450881b285dd48d01fdd` | Current provisional `TtsRequest { voice, text }`, `TaskClass::TtsSynthesis`, host call `tts.synthesize`, and string task error. |
| `crates/arcweft-core/src/engine/suspend.rs` | `bd0ec4fe1d14391c3bec5abe900bb9d1d80d9073` | Current hard-coded TTS branch accepts `synthesize|synthesis` and `voice`; direct deletion target. |
| `crates/arcweft-core/src/engine.rs` | `804ee532d20192bbed5855084221e0f4bcf62ace` | Existing ordinary flow/suspension/task spine is preserved. |
| `crates/arcweft-core/src/value.rs` | `25ee59e63f9354d357d283f067ab1123804b0d89` | Runtime supports nominal records and dense byte values for typed results/errors. |
| `crates/arcweft-core/src/value/nominal_record.rs` | `00437bc01ae1d9e4c21a1afa909f3542da4f3bd4` | Nominal identity/layout/ordinal validation supports exact TTS runtime values. |
| `crates/arcweft-need/src/lib.rs` | inspected at commit | Existing typed Need states include ready/error/cancelled and are reused. |
| `crates/arcweft-host-adapter/src/lib.rs` | `5772c82828e3e9457519b28ef31b1a8de5c8617c` | Existing manifest-derived policy, unique host-call ownership, pending completion, cancellation, and testable dispatch are preserved. |
| `crates/arcweft-runtime-scheduler/src/lib.rs` | inspected at commit | Existing deterministic TaskKey/join/event ordering/cancellation substrate is reused. |
| `crates/arcweft-runtime-host/src/native_task.rs` | inspected at commit | Host worker/main-thread and adapter pumping are the I/O owner. |
| `crates/arcweft-runtime-driver/src/session_save.rs` | `c5fd9c392092d47a29fee26c3ea9545dce95cb04` | `HostTasks` and `TaskGenerationPins` already block nonquiescent save; no TTS active-state wire is needed. |
| runtime-driver lifecycle/hot-swap/task paths | inspected at commit | Existing task generation pins and atomic candidate publication support active/queued reload rules. |
| `crates/arcweft-runtime-driver/src/session/replay/model.rs` | `ec1d3f81532c2f0a5a16c1dc0224191d54929f59` | External outcomes are recorded/injected, but failure is stringly; direct schema-1 correction is required. |
| `crates/arcweft-manifest-model/src/schema.rs` | `7145b7647137a0ff93e0740c0e758eac59a85d77` | Manifest schema 1 is final; module import already owns artifact/ABI coordinates. |
| `crates/arcweft-manifest-model/src/identity.rs` | `ea9937...` (connector inspection) | Existing manifest nominal strings are the accepted lower metadata substrate. |
| `crates/arcweft-adapter-metadata/src/model.rs` | `1076d9...` (connector inspection) | Generated schema-1 metadata is the correct provider capability/export owner. |
| `crates/arcweft-adapter-metadata/src/codec.rs` | `79d8e4...` (connector inspection) | Strict JSON, ordering, duplicate, payload and ABI hash substrate exists; extend directly. |
| `crates/arcweft-bundle/src/container.rs` | `4ddc43ededd8fd121983bb81c4510c1fa964af71` | AWFB v1 has section kinds 1–21; exact next codes 22/23 are available. |
| `crates/arcweft-bundle/src/lib.rs` | `da3384a5a7a2be89a0737a787f0c8aa3ede9356f` | Current structured bundle has audio graph and adapter manifests but no TTS catalog. |
| `crates/arcweft-lang-syntax/src/parser/headers.rs` | `2e6871abaaf9b7649d1e2bab01fda02c020f85f8` | Current generic entity grammar still recognizes `voice profile`/`voice`; Lang-01.4 final `res` cut must delete it directly. |
| Lang-01.4 request and production reconciliation | inspected at commit | Private `res` shadow exists; public typed-resource/reference owner remains prerequisite and must be consumed. |
| Lang-01.5/01.5.1 request and implementation docs | inspected at commit | Sole manifest decoder/generated metadata are the mandated ownership path; no second parser. |
| `crates/arcweft-dialogue/src/character_dialogue/identity.rs` | inspected at commit | Bounded canonical dialogue locale and presentation-only `voice.*` reference exist; TTS does not infer from them. |
| `docs/03-presentation/audio.md` | `64140d...` (connector inspection) | TTS crate/voice-profile/provider sketches are aspirational and not production evidence. |
| `docs/implementation/2026-07-20-aw-ah-009.4.1-intake-reconciliation.md` | `fb5b279239f2ee1be9a9c2e411e3a96bbf1af65c` | Original package SHA/integrity were independently checked by repository intake; TTS is a new independent subsystem and non-blocking. |
| latest main correction docs through commit | commit-bound | Dialogue profile/projection owners remain separate; this contract does not create a second dialogue registry. |

Ellipsized blob prefixes above are used only where the connector inspection was
commit-bound but the full blob value was not retained in this artifact build.
The commit pin, path, and finding remain the evidence identity; no guessed full
SHA is asserted.

## 4. Original AW-AH-009.4.1 package evidence

The original archive was not supplied to this session and was not independently
opened here. Current repository intake records that it had SHA-256:

```text
ebb7e5a1914a1ab7dd56c12719871fe8c54591d77152315d86ec2ea2b8ff2604
```

and that its 24 manifest entries, sizes, and hashes were verified. This contract
uses that intake record only as historical evidence. It does not assume any
sketched crate or API exists in production.

## 5. Verified substrate versus selected design

### Verified current substrate

- no production `arcweft-audio-tts` crate;
- provisional stringly core TTS request only;
- typed Task/Need/scheduler/cancellation and host-adapter registry;
- audio codec/mixer/device separation;
- schema-1 manifest/generated metadata foundations;
- AWFB v1 section registry through code 21;
- save blockers, generation pinning, and external-outcome replay;
- exact Character identity and independent dialogue presentation voice reference.

### New contract decisions, not current production claims

- new `arcweft-audio-tts` crate and all its nominal types;
- typed resource/functions;
- provider/profile catalogs and section kinds 22/23;
- generated `tts_providers` extension;
- AWTP 1;
- host TTS adapter/executor/secret/queue/retry model;
- typed task-error direct correction;
- all selected limits, diagnostics, privacy projections, and test rows.

## 6. Work not performed

No implementation file was edited. No local clone or worktree was available.
The following were not run and are therefore not claimed as evidence:

- Cargo format/check/Clippy/tests;
- repository `just` gates;
- code generation or fixture regeneration;
- Tier 2, native/Web/headless parity, protocol simulation, or structural audit;
- dependency graph extraction from a local `cargo metadata` run;
- original AW-AH-009.4.1 ZIP re-verification in this session.

Those operations belong to the implementation cuts and are fully specified in
`TEST_MATRIX.md`.
