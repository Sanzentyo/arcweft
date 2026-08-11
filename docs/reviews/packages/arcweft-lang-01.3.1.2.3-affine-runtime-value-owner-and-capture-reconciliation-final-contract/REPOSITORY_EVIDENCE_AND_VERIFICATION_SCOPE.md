# Repository evidence and verification scope

## 1. Commit-pinned and supplied evidence

- Complete Lang-01.3.1.2.3 request bytes.
- Complete supplied `Rust Skill.txt` and project premise.
- Complete predecessor ZIP bytes for Lang-01.3.1.2.1, .2, and .2.1.
- Request-recorded clean production baseline `177ba1e61e43fb2da2149869ce35e165d1e93b66`.
- Predecessor repository evidence, exact Rust-shaped owners, wire allocations, implementation orders, supersession deltas, and test matrices.
- Current root/scoped AGENTS byte identities listed in `AGENTS_AND_RUST_POLICY.md`.

The governing request records unconditional `Clone` across the executable runtime-value graph, clone-based partial application and closure capture, clone-based iterator advancement, and 322 `arcweft-core` compile errors from the diagnostic removal experiment. That experiment remains request evidence; this package does not claim to have rerun it.

All three supplied predecessor ZIPs passed `unzip -t`. Their 17, 40, and 23 internal `MANIFEST.sha256` entries matched the extracted bytes. Every predecessor JSON test row is retained with source identities in `PARENT_TEST_MATRIX_INDEX.json`.

## 2. Current raw-main observations

The following exact files were retrieved from the public raw `main` endpoint on 2026-08-10 (Asia/Tokyo). These bytes confirm that the request's missing-owner premise remains materially present. Because the transport did not expose the full branch head SHA and no Git checkout was mounted, this table is deliberately labelled moving raw-main evidence rather than a commit pin.

| Current raw path | Lines | Bytes | SHA-256 | Observed production fact |
|---|---:|---:|---|---|
| `crates/arcweft-core/src/value.rs` | 2,490 | 84,272 | `0e91906e017777bbd00b07159bcc3f9b52ab2bd497d3f7080436344b41e0cf49` | `RuntimeBinding`, function bodies/values, `RuntimePayload`, `RuntimeValue`, aggregate/sequence carriers, and `RuntimeExpr` still derive `Clone`; `partially_apply` clones captures, arguments, and body; `RuntimeExpr::Value(RuntimeValue)` remains. |
| `crates/arcweft-core/src/value/env.rs` | 480 | 14,573 | `775480ba479f0891035c08353aeb2b995b7abfe584be52b535ed18382d972b82` | `RuntimeEnv: Clone`; `bindings_snapshot()` clones all visible bindings; reference-based setters and root replacement clone values. |
| `crates/arcweft-core/src/value/range.rs` | 339 | 10,033 | `b06843010877e1371813ed252f29a51dd5e013b2c0969b61c6b1d795af50a1d2` | `RuntimeIterator` derives `Clone`; `Values::next` executes `items.get(*index).cloned()?`. |
| `crates/arcweft-core/src/value/sequence_impls.rs` | 1,580 | 52,736 | `5387b150d01fdd2827e617dde8c3b64e67ecfac4f8db1444eefacf0baab2ba89` | non-consuming row/materialization/index/slice paths construct values by child duplication; current sequence owner has no affine extraction protocol. |
| `crates/arcweft-core/src/pattern.rs` | 509 | 17,122 | `e8d7feae80e062cf55eef50ad1125c1d0c8aab70a41d74d6e1993b8d4cf532ae` | the existing `RuntimePattern::Literal(RuntimeValue)` embeds a live value and matching/rest binding code clones values; this is why the correction changes that original variant to `RuntimeConstantId` and attaches the binding plan directly. |
| `crates/arcweft-core/src/plan.rs` | 747 | 22,549 | `c74e05bc68f4dbd675b11d6b8e67bb8ba380d70abadf2387beb4f9e84f7946e8` | `RuntimePlan` and related plan carriers still derive `Clone`/Serde and embed `RuntimeExpr`/patterns; a direct executable-value literal and direct plan codec therefore cannot survive executable `Clone` removal. |
| `crates/arcweft-core/src/engine.rs` + `engine/flow.rs` | parsed raw-main view | byte hash not retained | moving raw-main corroboration only | `Engine`/`FlowFiber` derive `Clone`; `pending_ops` stores cloned `FlowOp`; control frames clone `Arc<[FlowOp]>`; `FlowOp::Bind` owns live bindings and `ForNext` owns `RuntimeIterator`. This concrete seam is why the final contract normalizes the original flow arena and moves live continuation state to the original control-frame enum. |

Current maintained documentation still describes one Sans-I/O runtime authority, VM/AOT/product parity, typed thread-capture obligations, explicit drop obligations, and Stream suspension/replay boundaries. No maintained document supplied an already-implemented generic affine value owner that would supersede this request.

These observations are corroboration only. The normative implementation base remains the request's clean commit until Stage 0 records a new full Git SHA and proves no result-changing conflict.

## 3. Package verification actually performed

- complete input reads, SHA-256 recording, ZIP CRC tests, and predecessor internal-manifest verification;
- UTF-8/LF member checks, safe-path/member checks, and exact `OPEN_QUESTIONS.md == "none\n"`;
- JSON/CSV parsing, unique test IDs, requirement/area coverage, exact original-owner assertions for `RuntimeSeq`/`RuntimeEnv`/`RuntimePattern`/`RuntimePayload`, and preservation of all 803 parent rows;
- executable Python reference tests for classification, duplicate rejection, capture failure atomicity, repeat, iterator movement, slot copy/move, and dormant snapshot uniqueness;
- sorted SHA-256 member manifest with the 64-zero self-entry rule;
- deterministic fixed-timestamp ZIP construction, extraction/CRC verification, and byte-identical rebuild.

## 4. Explicitly not performed

- local Git checkout or full current-main Git SHA/dirty-state capture;
- production edit, patch, worktree, branch, PR, commit, or push;
- Rust compilation, Cargo tests/check/Clippy, Tier 2, Cargo metadata, or canonical structure audit;
- execution of the proposed Rust APIs, AWBC codec, save codec, or restore protocol against production.

The design is implementation-ready at the request baseline. The principal integration risk is mechanical breadth: the request exposed 322 core compile errors before downstream crates. G1/G2/G3 intentionally convert that breadth into compiling migrations without leaving semantic choices to compiler-error-driven local judgment.
