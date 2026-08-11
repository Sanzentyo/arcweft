# Exact bounded work accounting

All arithmetic uses checked `u64`; a counter overflow is the same typed limit failure as one-over. A limit is checked before proportional allocation or externally visible work.

| Limit key | Maximum | Phase | Charge rule |
|---|---:|---|---|
| `semantic_modules` | 4096 | catalog build | charge each HIR module once before View enumeration |
| `semantic_definitions` | 65536 | catalog build | charge one accepted View definition shell |
| `semantic_parameters_total` | 262144 | catalog build | charge one parameter/default slot |
| `semantic_parameters_per_definition` | 4096 | catalog build | checked before parameter vector allocation |
| `semantic_nodes_total` | 262144 | catalog build/static proof | charge one reachable View/value ExprId once |
| `semantic_nodes_per_definition` | 65536 | catalog build | checked before traversal expansion |
| `dependency_edges_total` | 524288 | catalog/proof | charge one canonical dependency edge once |
| `dependency_edges_per_node` | 4096 | catalog/proof | checked before edge list allocation |
| `view_nesting_depth` | 64 | catalog/compiler/runtime | element/call/branch/match/repeat/await nesting |
| `match_arms_total` | 65536 | catalog/product/runtime | charge one typed match arm |
| `match_arms_per_match` | 4096 | catalog/product/runtime | checked before arm allocation |
| `resource_references` | 65536 | catalog/product | charge one exact accepted resource dependency |
| `exports_total` | 65536 | catalog/product | charge one exported part binding |
| `exports_per_definition` | 4096 | catalog/product | checked before export allocation |
| `awvp_transcript_bytes` | 16777216 | codec | entire canonical field-1 transcript, before JSON allocation |
| `value_programs` | 65536 | compiler/codec/runtime | charge one distinct View value semantic program |
| `generated_awbc_functions` | 65536 | compiler/cross-section | charge one generated synthetic function |
| `program_inputs_total` | 262144 | compiler/codec/runtime | charge one canonical input binding |
| `program_inputs_per_program` | 1024 | compiler/codec/runtime | checked before input vector allocation |
| `view_instructions` | 262144 | compiler/codec/runtime | charge one instruction record |
| `view_constants` | 65536 | compiler/codec/runtime | charge one interned constant |
| `view_constant_bytes` | 4194304 | compiler/codec/runtime | aggregate canonical constant payload |
| `static_fragments` | 65536 | compiler/codec/runtime | charge one certified fragment |
| `static_fragment_bytes` | 8388608 | compiler/codec/runtime | aggregate fragment payload within transcript cap |
| `static_certificates` | 65536 | compiler/codec/runtime | charge one certificate |
| `static_certificate_bytes` | 4194304 | compiler/codec/runtime | aggregate certificate payload within transcript cap |
| `certificate_dependencies` | 524288 | proof/codec/runtime | charge each dependency in a proof closure once |
| `runtime_instruction_ops_per_frame` | 65536 | runtime | one decoded/evaluated View instruction operation |
| `runtime_awbc_ops_per_invocation` | 65536 | runtime/AWBC | existing AWBC work counter scoped to one value call |
| `runtime_program_invocations_per_frame` | 65536 | runtime | one View value-program invocation |
| `runtime_repeat_items_per_repeat` | 4096 | runtime | one materialized repeat item/key |
| `runtime_call_depth` | 64 | runtime | nested View call stack |
| `runtime_handlers_per_frame` | 4096 | runtime | one registered handler/input binding |
| `runtime_resource_binds_per_frame` | 16384 | runtime | one validated resource acquisition/bind |
| `runtime_text_bytes_per_frame` | 16777216 | runtime | plain and canonical RichText output bytes |
| `runtime_scratch_bytes` | 67108864 | runtime | all frame candidate scratch before commit |

## Algorithms

- Catalog construction is `O(M + V + E)` in modules, reachable nodes, and dependency edges. Each node is interned once by `ExprId`; a repeated reachability edge reuses the node and charges only the edge.
- Dependency closure and static proof are one memoized tri-color DFS over the canonical dependency graph: `O(V + E + D)`. A cycle yields the typed recursive/dynamic disposition; it is not recursively expanded.
- Product generation is `O(V + E + P + I + C)` for catalog nodes, dependencies, value programs, instructions, and constants. Program semantic IDs deduplicate only exact semantic identity; source ranges never deduplicate programs.
- Strict codec validation is `O(bytes + records + references)` and rejects a transcript before constructing an accepted catalog. Canonical order/uniqueness checks use already sorted arrays or bounded maps.
- Runtime evaluation is bounded independently by frame instructions, program invocations, AWBC operations, repeat items, call depth, handlers, resources, text bytes, and scratch bytes. Any limit failure discards the candidate frame and all staged lifecycle mutations.

## First-error order

For simultaneous limit failures use: transcript bytes; top-level record count; per-record count; nesting/depth; dependency/reference count; program/function binding; instruction count; constant/fragment/certificate bytes; runtime call depth; frame operations; program invocation/AWBC operations; repeat/handler/resource/text/scratch. Earlier semantic/type/generation failures still outrank work limits.
