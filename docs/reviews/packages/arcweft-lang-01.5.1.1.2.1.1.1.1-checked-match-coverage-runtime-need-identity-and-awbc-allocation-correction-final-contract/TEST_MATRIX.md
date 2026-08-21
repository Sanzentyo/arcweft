# Positive, negative, tamper, property, differential, limit, rollback, and structural tests

| ID | Area | Kind | Case | Expected | Tier |
|---|---|---|---|---|---|
| `AWBC-001` | opcode | exhaustive | all 256 input bytes | 64 allocated bytes decode exactly; all others reject | Tier-1 |
| `AWBC-002` | opcode | roundtrip | AwbcOpcode ALL inherent/Serde/Wire | numeric roundtrip equality | Tier-1 |
| `AWBC-003` | opcode | negative | 2f/7f/91/ff | unknown/reserved rejection | Tier-1 |
| `AWBC-004` | opcode | negative | instruction decoder receives 80 | wrong-class rejection | Tier-1 |
| `AWBC-005` | opcode | negative | terminator decoder receives 29 | wrong-class rejection | Tier-1 |
| `AWBC-G-1E` | opcode | golden | NeedTimeout first byte | exact 1e and exact operand grammar | Tier-1 |
| `AWBC-G-20` | opcode | golden | CommitDialogueResult first byte | exact 20 and exact operand grammar | Tier-1 |
| `AWBC-G-29` | opcode | golden | MakeNeedHandle first byte | exact 29 and exact operand grammar | Tier-1 |
| `AWBC-G-2A` | opcode | golden | CopyValue first byte | exact 2a and exact operand grammar | Tier-1 |
| `AWBC-G-2B` | opcode | golden | ExecuteLineOperation first byte | exact 2b and exact operand grammar | Tier-1 |
| `AWBC-G-2C` | opcode | golden | OpenStream first byte | exact 2c and exact operand grammar | Tier-1 |
| `AWBC-G-2D` | opcode | golden | FinishStream first byte | exact 2d and exact operand grammar | Tier-1 |
| `AWBC-G-2E` | opcode | golden | ApplyExternalStreamGroup first byte | exact 2e and exact operand grammar | Tier-1 |
| `AWBC-G-8F` | opcode | golden | NextStream first byte | exact 8f and exact operand grammar | Tier-1 |
| `AWBC-G-90` | opcode | golden | YieldStream first byte | exact 90 and exact operand grammar | Tier-1 |
| `VAR-0` | varint | golden | 0 | 00 | Tier-1 |
| `VAR-1` | varint | golden | 1 | 01 | Tier-1 |
| `VAR-127` | varint | golden | 127 | 7f | Tier-1 |
| `VAR-128` | varint | golden | 128 | 80 01 | Tier-1 |
| `VAR-4294967295` | varint | golden | 4294967295 | ff ff ff ff 0f | Tier-1 |
| `VAR-NC-1` | varint | negative | 80 00 | NonCanonicalVarint | Tier-1 |
| `VAR-NC-2` | varint | negative | 81 00 | NonCanonicalVarint | Tier-1 |
| `VAR-OV-1` | varint | negative | ff ff ff ff 1f | overflow | Tier-1 |
| `VAR-OV-2` | varint | negative | sixth continuation byte | sixth-byte rejection | Tier-1 |
| `VAR-TR-1` | varint | negative | 80 | truncated/unterminated rejection | Tier-1 |
| `TENSOR-001` | wire | golden | Tensor shape [1,128] | 02 01 80 01 before fixed-bit elements | Tier-1 |
| `WIRE-001` | wire | rollback | payload write fails after envelope reserve | final Vec truncated to original length | Tier-1 |
| `WIRE-002` | wire | property | canonical decode then encode | byte-for-byte identity | Tier-2 |
| `WIRE-003` | wire | fuzz | arbitrary byte slices | no panic; only canonical values accepted | Tier-2 |
| `WIRE-004` | wire | structural | codec source architecture | one opcode decoder and no raw DTO | Tier-1 |
| `WIRE-005` | wire | structural | usize | no usize Wire implementation or field | Tier-1 |
| `FLAG-001` | flags | exhaustive | bits 0..31 | 0..5 unique; 6..31 reject | Tier-1 |
| `FLAG-002` | flags | negative | bits 4+5 | verification failure | Tier-1 |
| `FLAG-003` | flags | negative | NeedProducer + MaySuspend | verification failure | Tier-1 |
| `FLAG-004` | flags | positive | Synthetic NeedProducer + Deterministic + MayAllocate | accepted | Tier-1 |
| `FLAG-005` | flags | positive | Synthetic selector without producer bit | accepted | Tier-1 |
| `KIND-001` | kind | exhaustive | 0..255 tags | exact 0,1,2,3,6,7,8,9,10 accepted | Tier-1 |
| `KIND-002` | kind | negative | removed 4 and 5 | always reject | Tier-1 |
| `PARITY-001` | execution | differential | VM/structured/AOT pending operations | same value, exit, events and traps | Tier-2 |
| `VERSION-001` | version | structural | all Arcweft-owned markers | exactly 1 | Tier-1 |
| `NEED-001` | need_identity | positive | same host plan/args/site | same NeedId | Tier-1 |
| `NEED-002` | need_identity | positive | different host argument | different NeedId | Tier-1 |
| `NEED-003` | need_identity | positive | different producer contract | different NeedId | Tier-1 |
| `NEED-004` | need_identity | positive | different site | different NeedId | Tier-1 |
| `NEED-005` | need_identity | positive | direct Await handle | embedded NeedId preserved | Tier-1 |
| `NEED-006` | need_identity | positive | AwaitMany duplicate values at different indexes | different child NeedIds | Tier-1 |
| `NEED-007` | need_identity | positive | AwaitMany reordered sources | different base and child identities | Tier-1 |
| `NEED-008` | need_identity | positive | AwaitMany index u32::MAX | exact 4-byte LE transcript index | Tier-1 |
| `NEED-009` | need_identity | positive | timeout source/output | source unchanged; output distinct | Tier-1 |
| `NEED-010` | need_identity | positive | timeout different limit | different output NeedId | Tier-1 |
| `NEED-011` | need_identity | positive | JoinSameKey duplicate observer | one TaskKey/TaskId and shared publication | Tier-1 |
| `NEED-012` | need_identity | positive | AlwaysStart duplicate | distinct TaskKey/TaskId; same logical NeedId | Tier-1 |
| `NEED-013` | need_identity | positive | replay launch ordinal | exact TaskId restored | Tier-1 |
| `NEED-014` | need_identity | positive | hot replacement compatible semantic digests | explicit generation rebind | Tier-1 |
| `NEED-015` | need_identity | positive | hot replacement changed contract | cancel/rebuild | Tier-1 |
| `NEED-016` | need_identity | negative | malformed zero fixed ID restore | whole snapshot rejected | Tier-1 |
| `NEED-017` | need_identity | negative | tampered child ID | recomputation mismatch rejects | Tier-1 |
| `NEED-018` | need_identity | negative | contradictory terminal publication | deterministic conflict failure | Tier-1 |
| `NEED-019` | need_identity | positive | equal duplicate terminal publication | idempotent dedupe | Tier-1 |
| `NEED-020` | need_identity | positive | line task through shared substrate | line domain transcript | Tier-1 |
| `NEED-021` | need_identity | positive | View program same semantics/recompiled HIR IDs | same product digest and NeedId | Tier-1 |
| `NEED-022` | need_identity | positive | View revision without explicit compatible mapping | replacement does not preserve live state | Tier-1 |
| `COV-001` | coverage | negative | guarded wildcard only | non-exhaustive hard error | Tier-1 |
| `COV-002` | coverage | positive | unguarded wildcard then arm | later arm unreachable warning/evidence | Tier-1 |
| `COV-003` | coverage | negative | constant false wildcard | arm false-guard unreachable and non-exhaustive | Tier-1 |
| `COV-004` | coverage | positive | duplicate Bool literal | second duplicate unreachable | Tier-1 |
| `COV-005` | coverage | negative | missing Result Err | non-exhaustive witness Err | Tier-1 |
| `COV-006` | coverage | negative | missing Option None | non-exhaustive witness None | Tier-1 |
| `COV-007` | coverage | positive | Or overlap | usefulness union without duplicate matrix fabrication | Tier-1 |
| `COV-008` | coverage | positive | nested tuple/variant complete | exhaustive | Tier-1 |
| `COV-009` | coverage | negative | nested tuple/variant missing payload case | non-exhaustive witness | Tier-1 |
| `COV-010` | coverage | positive | record exact all fields | product usefulness | Tier-1 |
| `COV-011` | coverage | positive | record ignore rest | omitted fields wildcard | Tier-1 |
| `COV-012` | coverage | positive | sequence exact [a,b] | only length 2 covered | Tier-1 |
| `COV-013` | coverage | positive | sequence rest [a,..] | length interval >=1 | Tier-1 |
| `COV-014` | coverage | negative | infinite integer literals no wildcard | non-exhaustive Other | Tier-1 |
| `COV-015` | coverage | positive | infinite integer plus wildcard | exhaustive | Tier-1 |
| `COV-016` | coverage | positive | closed enum all variants | exhaustive | Tier-1 |
| `COV-017` | coverage | negative | open/future enum constructors only | residual open non-exhaustive | Tier-1 |
| `COV-018` | coverage | positive | Never with zero arms | exhaustive | Tier-1 |
| `COV-019` | coverage | positive | Never with arm | arm unreachable | Tier-1 |
| `COV-020` | coverage | positive | dynamic guarded covering arm then wildcard | wildcard remains reachable and closes | Tier-1 |
| `COV-021` | coverage | negative | caller fabricated exhaustive bit | API cannot construct; tamper rejected | Tier-1 |
| `COV-022` | coverage | negative | reordered unreachable rows | constructor normalizes sorted unique or rejects encoded tamper | Tier-1 |
| `COV-023` | coverage | negative | poisoned pattern | hard error before coverage | Tier-1 |
| `COV-024` | coverage | negative | unsupported opaque decomposition | hard error; wildcard remains legal | Tier-1 |
| `LIM-max_arms-E` | coverage_limit | exact_limit | max_arms=4096 | success if otherwise valid | Tier-1 |
| `LIM-max_arms-O` | coverage_limit | one_over | max_arms=4097 | hard error and no partial publication | Tier-1 |
| `LIM-max_pattern_nodes-E` | coverage_limit | exact_limit | max_pattern_nodes=65536 | success if otherwise valid | Tier-1 |
| `LIM-max_pattern_nodes-O` | coverage_limit | one_over | max_pattern_nodes=65537 | hard error and no partial publication | Tier-1 |
| `LIM-max_or_alternatives-E` | coverage_limit | exact_limit | max_or_alternatives=4096 | success if otherwise valid | Tier-1 |
| `LIM-max_or_alternatives-O` | coverage_limit | one_over | max_or_alternatives=4097 | hard error and no partial publication | Tier-1 |
| `LIM-max_matrix_rows-E` | coverage_limit | exact_limit | max_matrix_rows=8192 | success if otherwise valid | Tier-1 |
| `LIM-max_matrix_rows-O` | coverage_limit | one_over | max_matrix_rows=8193 | hard error and no partial publication | Tier-1 |
| `LIM-max_specializations-E` | coverage_limit | exact_limit | max_specializations=32768 | success if otherwise valid | Tier-1 |
| `LIM-max_specializations-O` | coverage_limit | one_over | max_specializations=32769 | hard error and no partial publication | Tier-1 |
| `LIM-max_sequence_partitions-E` | coverage_limit | exact_limit | max_sequence_partitions=2048 | success if otherwise valid | Tier-1 |
| `LIM-max_sequence_partitions-O` | coverage_limit | one_over | max_sequence_partitions=2049 | hard error and no partial publication | Tier-1 |
| `LIM-max_witness_nodes-E` | coverage_limit | exact_limit | max_witness_nodes=1024 | success if otherwise valid | Tier-1 |
| `LIM-max_witness_nodes-O` | coverage_limit | one_over | max_witness_nodes=1025 | hard error and no partial publication | Tier-1 |
| `LIM-max_recursion_depth-E` | coverage_limit | exact_limit | max_recursion_depth=64 | success if otherwise valid | Tier-1 |
| `LIM-max_recursion_depth-O` | coverage_limit | one_over | max_recursion_depth=65 | hard error and no partial publication | Tier-1 |
| `LIM-max_unreachable_rows-E` | coverage_limit | exact_limit | max_unreachable_rows=4096 | success if otherwise valid | Tier-1 |
| `LIM-max_unreachable_rows-O` | coverage_limit | one_over | max_unreachable_rows=4097 | hard error and no partial publication | Tier-1 |
| `COV-PROP-1` | coverage | property | finite enum versus brute-force enumerator | same useful/exhaustive result | Tier-2 |
| `COV-PROP-2` | coverage | differential | Maranget matrix versus generated finite domains | same result | Tier-2 |
| `OWN-001` | ownership | exhaustive | every current TypeKind row | one disposition or closed rejection | Tier-1 |
| `OWN-002` | ownership | negative | project nominal direct by-value cycle | RecursiveValueCycle | Tier-1 |
| `OWN-003` | ownership | positive | cycle broken by Shared | recursive snapshot admission | Tier-1 |
| `OWN-004` | ownership | negative | missing AcceptedNominal opaque evidence | MissingOpaqueEvidence | Tier-1 |
| `OWN-005` | ownership | negative | affine opaque line handle | AffineHandle | Tier-1 |
| `OWN-006` | ownership | positive | Need<T> type class | Copy handle disposition | Tier-1 |
| `OWN-007` | ownership | negative | Need producer argument non-snapshot | producer admission failure | Tier-1 |
| `OWN-008` | ownership | negative | Function type without value evidence | CallableNeedsValueEvidence | Tier-1 |
| `OWN-009` | ownership | positive | capture-free registered callable value | value-level Copy certificate | Tier-1 |
| `OWN-010` | ownership | negative | resource registry digest mismatch | catalog construction failure | Tier-1 |
| `OWN-LIM-E` | ownership | exact_limit | depth=64/nodes=65536 | success if otherwise valid | Tier-1 |
| `OWN-LIM-O` | ownership | one_over | depth=65 or nodes=65537 | hard error; no partial fact | Tier-1 |
| `DIG-001` | digest | property | same semantics with different HIR arena allocation | same CheckedMatchSemanticDigest | Tier-2 |
| `DIG-002` | digest | negative | arm reorder | different digest | Tier-1 |
| `DIG-003` | digest | negative | resource registry change | different digest | Tier-1 |
| `DIG-004` | digest | structural | persistent transcript fields | no ExprId/ScopeId/PatternId/LocalId | Tier-1 |
| `DIG-005` | digest | golden | nested Match semantic row | exact BLAKE3 digest fixture | Tier-1 |
| `BUNDLE-001` | bundle | tamper | selector digest differs from checked Match | atomic rejection | Tier-1 |
| `SAVE-001` | save | tamper | NeedId/task correlation mismatch | restore rejection before mutation | Tier-1 |
| `REPL-001` | replacement | rollback | new generation install fails | old generation remains authoritative | Tier-1 |
| `STRUCT-001` | structure | absence | legacy String NeedHandle | absent | Tier-1 |
| `STRUCT-002` | structure | absence | caller-supplied coverage | absent | Tier-1 |
| `STRUCT-003` | structure | absence | duplicate opcode numeric map | absent | Tier-1 |
| `STRUCT-004` | structure | absence | compatibility reader | absent | Tier-1 |

Tier-1 rows run in the ordinary workspace validation set. Tier-2 includes
property/differential/fuzz/parity tests and must run before the final atomic
switch. Every pending opcode has an exact first-byte and operand golden in the
same feature cut; all 256 opcode bytes are classified in one exhaustive test.
