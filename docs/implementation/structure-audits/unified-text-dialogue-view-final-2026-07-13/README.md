# Unified text DialogueView final structural audit — 2026-07-13

Revision measured: Jujutsu change `oknwozsuyqqurprwpmuplplmmmxqprpt`, committed
as `29f0dded1039f0c7dc2fbbb041c449e400df2424` (`Finalize unified text and typed
dialogue Views`).

Canonical commands:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-dialogue-view-final-2026-07-13
```

The audit scanned 2,651 files, including 1,255 Rust files and 616,763 physical
Rust LOC across 91 package manifests. It reports **0 errors and 128 warnings**.
The warnings comprise 93 size warnings, 25 embedded-test warnings, 7 facade
warnings, and 3 tracked architecture warnings. No architecture warning points
at a manifest changed by the DialogueView integration.

The structured reports are:

- `file_metrics.csv`: exact current size and classification for every scanned
  file;
- `changed-rust-files.csv`: the 170 changed Rust paths, their owning crate,
  current exact metrics, embedded-test LOC, parent LOC, line delta, and any
  structural warning;
- `dependency_edges.csv`: normal, development, and build dependency edges;
- `public_type_duplicates.csv`: the audit's public-name collision inventory;
- `violations.md`: all 128 canonical warnings.

`changed-rust-files.csv` is evidence only. It is not a source gate and no
pass/fail behavior searches implementation spellings or file locations.

## Changed Rust surface

The cut has 158 modified, 9 added, and 3 deleted Rust paths. Of those, 167
exist in the measured checkout; together they contain 5,765,356 bytes and
168,028 physical LOC. Twenty-six changed production files have terminal
embedded test modules; their exact test-module LOC is recorded in
`changed-rust-files.csv`.

The four production files that grow by more than 300 physical LOC are the
mandatory decomposition-review set:

| Path | Crate | Bytes | Current LOC | Parent LOC | Delta | Embedded tests | Responsibility and result |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- |
| `crates/arcweft-bundle/src/standard_view.rs` | `arcweft-bundle` | 11,342 | 308 | 0 | +308 | 0 | Standard linked `std.view.dialogue` program, text resources, style resources, and deterministic merge operations; cohesive and in the preferred module range. |
| `crates/arcweft-lang-sema/src/dialogue_view.rs` | `arcweft-lang-sema` | 14,922 | 380 | 0 | +380 | Exact nominal dialogue-input inventory, projection typing, attributed-record validation, and 108 LOC of focused terminal tests; cohesive semantic ownership. |
| `crates/arcweft-player-scene/src/frame/view_text.rs` | `arcweft-player-scene` | 28,036 | 801 | 373 | +428 | Frame-local authored View text resolution, Fx binding, document preparation, target geometry, and dialogue prepared state; cohesive pipeline, but one LOC above the ordinary 300–800 preferred range and should not absorb another responsibility. |
| `crates/arcweft-runtime-driver/src/dialogue/store.rs` | `arcweft-runtime-driver` | 20,072 | 545 | 0 | +545 | Persistent presentation/occurrence store, operation application, stale-safe advance, snapshot invariant validation, and identity allocation; cohesive Sans I/O runtime state. |

Twenty-seven changed files carry 34 canonical warning occurrences: 26
`SIZE001`, 6 `TEST001`, and 2 `SIZE002`. Comparing current LOC with the parent
LOC derived from the same Git delta shows that every one of these files was
already beyond the applicable warning threshold in the parent. This cut does
not create a new warning- or error-level size crossing. In particular, the new
308/380/545 LOC responsibility modules remain within the preferred range, and
the 801 LOC frame preparer is the only growth-trigger module just above its
upper edge.

The broad cross-layer cut groups into these responsibility clusters; exact
per-file roles and sizes remain in `changed-rust-files.csv`:

| Crate/owner | Changed Rust paths | Major changed responsibilities |
| --- | ---: | --- |
| `arcweft-cli` | 24 | Bundle View lowering, authored geometry, Native observe/capture projection, parity checks, and CLI integration matrices. |
| `arcweft-lang-sema` | 19 | Exact DialogueView nominal inventory, standard prelude types, generic declaration checks, removed-language cleanup, and semantic tests. |
| `arcweft-lang-syntax` | 16 | Dialogue/View AST, generic entity-header grammar and recovery, CST classification, and parser behavior tests. |
| `arcweft-runtime-driver` | 15 | Persistent dialogue store, retained View evaluation, save/load correspondence validation, session orchestration, and tamper tests. |
| `arcweft-player-scene` | 14 | Authored View text/frame preparation, dialogue action routing, hit/input behavior, and scene integration tests. |
| `arcweft-lsp` | 10 | DialogueView metadata, diagnostics, completion, hover, actions, and session behavior. |
| `arcweft-runtime-plan` / `arcweft-presentation` | 8 each | Dialogue render-plan defaults and typed Fx/input/layer state. |
| `arcweft-render-text` / `arcweft-player-native` | 7 each | Resolved RichText/Fx playback and Native frame/capture adaptation. |
| `arcweft-bundle` | 6 | Standard View resources, serialized View schema/codec, and round-trip tests. |
| `arcweft-render-wgpu` | 5 | Prepared dialogue geometry, actions, transforms, and focused tests. |

## Ownership and dependency review

The final ownership direction is coherent:

- `arcweft-view` owns renderer-neutral dialogue presentation, entry, instance,
  stage, revision, and advance-target identities in `dialogue.rs`;
- `arcweft-lang-sema` owns the exact six-field nominal DSL contract and does
  not depend on bundle, runtime, renderer, or player crates;
- `arcweft-bundle` owns the serialized View resource contract and the linked
  standard View;
- `arcweft-runtime-driver` owns persistent dialogue occurrence state and feeds
  it through the ordinary retained View evaluator;
- `arcweft-player-scene` adapts retained View output to prepared renderer state;
  renderers consume the result without becoming owners of dialogue state.

Unique package fan-in/fan-out from `dependency_edges.csv`:

| Crate | Fan-in | Fan-out | Boundary observation |
| --- | ---: | ---: | --- |
| `arcweft-view` | 9 | 7 | Low renderer-neutral contract owner; no dependency on bundle/runtime/player. |
| `arcweft-bundle` | 10 | 22 | Serialized product boundary consumed by runtime and players. |
| `arcweft-lang-sema` | 8 | 7 | Depends only on language/data/source layers plus `thiserror`. |
| `arcweft-runtime-driver` | 6 | 13 | Depends downward on bundle/core/presentation/view contracts. |
| `arcweft-player-scene` | 3 | 18 | Application-facing scene adapter; depends on runtime and rendering layers. |
| `arcweft-render-wgpu` | 6 | 15 | Renderer consumes `arcweft-view`; no reverse renderer dependency is introduced. |

The only changed manifest is `arcweft-player-scene/Cargo.toml`. It promotes the
existing `arcweft-view` development dependency to a normal dependency because
production input/frame code now carries typed dialogue advance identities.
The set of unique dependencies is unchanged, and the promotion follows the
existing `player-scene -> view` direction.

The review initially found a non-isomorphic public-name collision: sema used
`DialogueViewProjection` for all six exact input fields, while bundle used the
same name for the two projections valid in a serialized text-source record.
The final cut resolves it directly by naming the bundle subset
`DialogueTextProjection`; CLI lowering retains an explicit semantic-to-text
subset match. No compatibility alias or dual spelling was added, and the final
`public_type_duplicates.csv` no longer reports that collision.

Two additional review-trigger files remain below warning thresholds. Generic
entity-header grammar and structured unexpected-tail recovery bring
`arcweft-lang-syntax/src/parser/headers.rs` from 966 to 1,067 LOC (+101); the
module remains cohesive around declaration headers. Atomic View/store/frame
save correspondence brings `arcweft-runtime-driver/src/view_runtime.rs` from
788 to 928 LOC (+140); it remains the retained View state owner. Both exceed
the ordinary preferred 800-LOC range but remain below the 1,200-LOC production
warning threshold, so further unrelated responsibilities should be extracted
instead of added there.

The save/load tamper matrix initially pushed
`arcweft-runtime-driver/tests/session.rs` over the 2,500-LOC integration-test
warning threshold. The final cut extracts the exact dialogue correspondence
cases to `tests/dialogue_restore/mod.rs` (4,432 bytes / 143 LOC); `session.rs`
is 88,673 bytes / 2,442 LOC. The final canonical report therefore introduces
no new warning crossing.

## Largest current workspace Rust files

The largest Rust file is generated Unicode data:
`crates/arcweft-text-layout/src/vertical_orientation.rs` is 357,456 bytes and
12,399 physical LOC. Its header records the Unicode 17.0.0 source and states
that the range data is generated and must not be edited by hand, so it is kept
separate from production hotspot rankings.

Largest non-generated production files:

| Path | Bytes | Physical LOC | Embedded test LOC | Major responsibility |
| --- | ---: | ---: | ---: | --- |
| `crates/arcweft-core/src/value.rs` | 84,017 | 2,500 | 0 | Runtime value, function, intrinsic, sequence, iterator, and value-error model. |
| `crates/arcweft-core/src/engine/eval/calls.rs` | 89,488 | 2,481 | 0 | Runtime intrinsic, method, trait, and backend call dispatch. |
| `crates/arcweft-lang-sema/src/checker/expr.rs` | 94,248 | 2,469 | 0 | Expression-kind dispatch, expected-type checking, and method/call inference. |
| `crates/arcweft-cli/src/toolchain_profile.rs` | 75,712 | 2,463 | 296 | Toolchain profile command selection, execution, sampling, and reports. |
| `crates/arcweft-lang-sema/src/checker.rs` | 85,689 | 2,459 | 0 | Type-judgment graph, borrow/effect state, checker orchestration, and diagnostics. |
| `crates/arcweft-core/src/awbc/product_step.rs` | 93,512 | 2,430 | 0 | Sans I/O Product AWBC execution and runtime-step adaptation. |
| `crates/arcweft-bundle/src/resource_codec/view/model.rs` | 76,343 | 2,420 | 42 | Serialized authored View program/style/text/control resource schema. |
| `crates/arcweft-runtime-driver/src/session.rs` | 93,251 | 2,415 | 0 | Bundle session lifecycle, stepping, pending actions, patching, and snapshots. |
| `crates/arcweft-bundle/src/container.rs` | 78,267 | 2,389 | 662 | AWFB container identities, section descriptors, validation, encoding, and decoding. |
| `crates/arcweft-cli/src/app/debug.rs` | 77,792 | 2,376 | 70 | Debug database, graph, timeline, RAG, script, and REPL command orchestration. |

Largest test files:

| Path | Bytes | Physical LOC | Classification and responsibility |
| --- | ---: | ---: | --- |
| `crates/arcweft-cli/tests/check/cli_runtime_bench.rs` | 255,217 | 7,935 | CLI runtime benchmark/check integration matrix. |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | 238,454 | 6,613 | Native vertical text observation and capture matrix. |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | Published JLREQ class-mix layout matrix. |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs` | 214,731 | 5,850 | Native samples, RichText effects, and capture behavior. |
| `crates/arcweft-compiler/src/tests.rs` | 179,339 | 5,350 | Compiler integration/unit behavior matrix. |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | 195,821 | 5,249 | Agent script/debug CLI behavior matrix. |

The error-level thresholds remain clear. The highest non-generated production
file is exactly 2,500 LOC and no production file exceeds the 2,500-LOC error
threshold. The largest integration-test file is 7,935 LOC, below the 8,000-LOC
error threshold. The canonical warnings still identify substantial existing
decomposition debt, especially facade files and terminal embedded tests, but
none is newly introduced by the DialogueView cut.
