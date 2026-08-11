# Ordered implementation plan and atomic merge ownership

## 1. Global merge rule

Cuts are compile-clean review points in the exact order below. Cuts 6, 7, and 8 form one
**protected version-migration merge group**: they may be separate review commits on one
branch, but none is independently mergeable, releasable, or cherry-pickable. The group is
merged only when ABI 2/codec 8, the shared host boundary, bundle schema 6/`awbc_v2`, save
schema 2, restore/fingerprint/hot-reload, and every hard-rejection test are complete.
There is never a product revision that writes a new version with an incomplete schema or
accepts both old and new formats.

No cut uses a feature flag, source gate, compatibility shim, dual reader/writer, serde
alias, endpoint DTO, Source compatibility module, CSS dependency, or Takumi dependency.

## 2. Cut sequence

| Cut | Named owner(s) | Exact work | Compile-clean exit gate | Forbidden interim state |
| ---: | --- | --- | --- | --- |
| 1 | Contract package / reviewers | Freeze the callable projection, sole instance/tombstone table, replay store, effect table, reused support owners, typed profile, exhaustion split, dropped-consumer cleanup, versions/tags | Package validation; `OPEN_QUESTIONS=0`; no product diff | Any unresolved public/wire choice |
| 2 | `arcweft-lang-syntax`, `arcweft-lang-hir`, `arcweft-lang-sema`, existing direct-call/CFG owners | Land or consume Lang-01.1.1 codec-stable ordinary-function direct suspension, typed await/ranges, own-scope-yield generator classification evidence, direct frames/CFG/safe-point substrate | Focused parser/HIR/sema/runtime-plan tests; workspace check/clippy; current product writer still writes only its current unrelated format | Provisional StreamPlan/handle/state/event/table/opcode; `stream fn`; Source workaround |
| 3 | `arcweft-lang-sema::callable`, external-binding publication owner | Consume Lang-01.3.1.1 final external Stream callable evidence through the shared catalog/resolver; expose exact accepted parameter/effect/result/source evidence | Callable resolver positive/negative/query-budget tests; no second argument binder | Source-text parameter inference or external-only resolver |
| 4 | `arcweft-manifest-model`, `arcweft-launch`, `arcweft-compiler::ProjectCompilationContext`, `arcweft-runtime-plan::stream_profile`, `arcweft-core::entry`, `arcweft-core::stream`, `arcweft-core::engine::stream` | Add the strict authored profile field/source-map entries, explicit target and sole compiler projection, accepted profile resolver/evidence, corrected identities, generic resolved arguments, affine handle, typed policy/lifecycle, sole table, replay, requests/events/outcomes/observations, staging, deterministic unit tests | `cargo test -p arcweft-core`; compile-fail affine API tests; check/clippy | RuntimeStep/AWBC/save version activation; runtime-plan→launch or launch→core dependency; sidecar authority; facade rebuild |
| 5 | `arcweft-runtime-plan`, owning `arcweft-core::plan` types | Replace RuntimePlan Source/old Stream fields with effect-set table and sole corrected Stream definition/reference model; reuse existing CFG/frame/source types | RuntimePlan canonicalization/lowering/tamper tests; workspace check/clippy | Translation to old Source/Stream table; duplicate Stream-local support types |
| 6 | **Version owner:** `arcweft-core::awbc`; producer `arcweft-runtime-plan::awbc_lower`; wrapper integration staged in `arcweft-bundle::product_awbc` | In the protected group, change ABI 1→2 and codec 7→8 once; update signature parameters, StreamHandle, sole definition table, functions/flags, opcodes/terminators/safe points, verifier, VM, compiled-region exchange; remove old Source/Stream tags/readers/writers | Full AWBC canonical round-trip/tamper/VM parity suite; standalone codec 7/ABI1 direct rejection; workspace check/clippy | Any codec8 writer lacking all tables/fields; legacy dispatch; two opcode/table paths |
| 7 | `arcweft-core::step`, `arcweft-runtime-host`, native host, web host, Agent runner | In the protected group, replace RuntimeStep/adapter ingress and egress with the exact one shared Stream JSON schema and byte codec | Native/web/Agent golden-byte parity; no endpoint DTO; all integer/strict JSON tests | Source events/close fields, old Stream event egress, adapter-local schema |
| 8 | **Bundle version owner:** `arcweft-bundle`; **save version owner:** `arcweft-runtime-driver::session_save` with `arcweft-save`; hot-swap owner `arcweft-runtime-driver::swap` | Complete protected group: bundle 5→6 and `awbc_v1`→`awbc_v2`; save 1→2; sole table snapshot; blockers; replay/tombstones; fingerprints; generation pins; atomic restore/hot reload; hard old-version rejection | Bundle/save golden bytes, restore atomicity/tamper, hot-reload classifications, full protected-group workspace check/clippy | Bundle5 carrying codec8; schema2 without full state; migration registry; old readers |
| 9 | All affected crate owners; final audit owner | Delete remaining Source and provisional Stream product modules/types/fields/tests after direct corrected evidence exists; retain `arcweft-source` and debug maps; run final workspace validation and structural audit | `cargo fmt --all -- --check`; focused tests; `cargo check --workspace --all-targets`; `cargo clippy --workspace --all-targets --all-features -- -D warnings`; full tests required by matrix; `cargo +nightly -Zscript tools/structure-audit.rs --root .` | Source/provisional product symbol reachable; source-text grep as acceptance evidence; CSS/Takumi touch |

## 3. Cut-6/7/8 version constant authority

- `arcweft-core::awbc` is the only owner that changes `AWBC_ABI_VERSION` to 2 and
  `AWBC_CODEC_VERSION` to 8.
- `arcweft-bundle` is the only owner that changes outer bundle schema to 6 and the sole
  executable discriminator to `awbc_v2`.
- `arcweft-runtime-driver::session_save` is the only owner that changes
  `BUNDLE_SESSION_SAVE_SCHEMA_VERSION` to 2; `arcweft-save` enforces the envelope.
- The protected group contains one coordinated test-vector update. No other crate defines
  a shadow constant or accepts a range of versions.

## 4. Reviewable-cut validation commands

Run the smallest focused crate tests after each owner change, then at every reviewable cut:

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run all matrix-selected focused tests before advancing. After Cut 9 run the structured
command exactly:

```text
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

The audit and Cargo metadata may prove module/dependency ownership; raw repository
source-text searches are not acceptance tests.

## 5. Rollback boundary

Before the protected migration group merges, rollback is the prior branch head. After it
merges, rollback is the complete merge group/artifact, never a mixed-version subset. No
runtime migration code is introduced for rollback.
