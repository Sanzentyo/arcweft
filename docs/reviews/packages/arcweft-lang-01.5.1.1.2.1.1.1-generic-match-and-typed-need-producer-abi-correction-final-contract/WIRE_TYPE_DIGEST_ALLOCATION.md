# Version-1 wire, type, opcode, and digest allocation

All Arcweft-owned markers below remain exactly version 1. No version bump or compatibility reader is authorized.

## AWBC allocations

| Family | Allocation | Version-1 grammar |
|---|---:|---|
| runtime type Tuple | tag 10 | `0x0a || vec<AwbcTypeId>` |
| runtime type Variant | tag 13 | `0x0d || AwbcVariantIdentity || vec<AwbcVariantCase>` |
| runtime type TaskHandle | tag 18 | `0x12` (unchanged) |
| runtime type NeedHandle | tag 19 | `0x13 || payload:AwbcTypeId` (new mandatory payload) |
| ordinary opcode MakeNeedHandle | `0x1e` | `0x1e || dst:u32 || plan:u32 || site:u32 || vec<register:u32>` |
| ordinary opcode Drop | `0x1f` | unchanged |
| function flag NEED_PRODUCER | bit 4 | `flags & (1 << 4) != 0` |
| View reactive section | schema 1 | exact grammar below |
| Need-handle snapshot DTO | schema 1 | exact version field; unknown fields denied |

Old tag-19 single-byte input is malformed in strict ABI 1. Decoder must not infer payload, use default/Dynamic, or invoke an old reader.

## Primitive grammar

All integers are unsigned little-endian unless a field is explicitly signed (`priority:i32_le`). Digest values are exactly 32 bytes. Arrays use `count:u32` followed by elements. Enum tags are one byte. No padding, unknown field, alternate order, or trailing byte is accepted.

## View reactive section exact byte grammar

```text
schema_version:u32 = 1
resource_type_registry_digest[32]

selector_count:u32
for selector sorted by site:
  site:u32
  checked_match_digest[32]
  function:u32
  input_state_type:u32
  input_state_type_digest[32]
  result_type:u32
  result_type_digest[32]
  case_count:u32
  for case sorted by arm:
    arm:u32
    case_ordinal:u32
    payload_tuple:u32
    output_count:u32
    for output sorted by output ordinal:
      output:u32
      local:u32
      value_type:u32
      disposition:u8 = 0  // SnapshotClone

producer_count:u32
for producer sorted by producer_contract bytes:
  producer_contract[32]
  function:u32
  result_type:u32
  payload_type:u32
  payload_type_digest[32]
  task_plan:u32
  argument_type_count:u32
  argument_types[argument_type_count]:u32

source_map_count:u32
for source map sorted by role discriminant and coordinates:
  role_tag:u8
  role payload:
    0 MatchSite: site:u32
    1 MatchArm: site:u32, arm:u32
    2 MatchBinding: site:u32, arm:u32, output:u32
    3 NeedProducer: producer_contract[32]
  source_map:u32
```

Codec validates all referenced IDs/roles after parse and before section publication. Unknown role/disposition tag, noncanonical order, duplicate key, missing/extra byte, count overflow, or dangling coordinate is an error.

## Synthetic selector type transcript

```text
"arcweft-view-match-selector-type-v1\0"
resource_type_registry_digest[32]
checked_match_digest[32]
input_state_type_digest[32]
arm_count:u32
for arm in source order:
  arm_ordinal:u32
  binding_count:u32
  for binding in HIR local order:
    binding_ordinal:u32
    runtime_type_digest[32]
```

`semantic_identity = BLAKE3(transcript)`. Generated public ID is `arcweft.internal.view.match_selector.<lower-hex semantic_identity>`. Case name is `arm_` plus eight lowercase hex digits. Generated labels are digest-covered diagnostics, not source identity.

## Checked Match semantic digest transcript

```text
"arcweft-checked-match-v1\0"
owner_expr_id canonical generation-bound bytes
match result TypeKind digest[32]                  // read from owner CheckedExpression
scrutinee_expr_id canonical bytes
scrutinee TypeKind digest[32]                     // read from child CheckedExpression
arm_count:u32
for each arm:
  ordinal:u32
  scope_id canonical bytes
  pattern_id canonical bytes
  pattern TypeKind digest[32]                     // read from CheckedPattern
  guard_presence:u8; guard ExprId/type/effect digest when present
  value_expr_id canonical bytes + value type/effect digest
  local_count:u32
  for each local:
    binding_ordinal:u32
    local_id canonical bytes
    local TypeKind digest[32]                     // read from CheckedBinding
    ownership disposition/rejection tags
coverage transcript once
```

No copied TypeKind field is required inside CheckedMatch. Missing/stale referenced facts abort digest construction.

## AWBC runtime type digest

`AwbcProgram::canonical_type_digest` recursively hashes one verified type graph under `arcweft-awbc-runtime-type-v1\0`, writing selected wire tag, canonical identities, child digests instead of table indices, and exact case/field order. It rejects cycles, invalid indices, depth over 64, duplicate variant identity, malformed built-in Result/Option, and malformed Need payload.

Selector state/result and Need payload use this same digest. No View-specific type digest exists.

## Need producer contract transcript

```text
"arcweft-need-producer-contract-v1\0"
schema_version:u32 = 1
producer_function_semantic_digest[32]
site:u32
task_plan_canonical_digest[32]
producer_parameter_count:u32 + parameter_type_digests
handle_argument_count:u32 + argument_type_digests
payload_type_digest[32]
policy_tag:u8
```

Task-plan digest excludes deleted need_id and covers public ID, capability, operation, signature type digests, class, priority, cancel scope, policy, payload digest, host-argument metadata, and many.

## NeedId transcript

```text
"arcweft-need-id-v1\0"
producer_contract_digest[32]
canonical_argument_digest[32]
```

NeedId is the 32-byte BLAKE3 result. Lowercase hex is display-only.

## Runtime Need handle snapshot

```rust
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeNeedHandleSnapshotV1 {
    pub version: u32, // exactly 1
    pub need: [u8; 32],
    pub producer_contract: [u8; 32],
    pub payload_type: [u8; 32],
    pub arguments: Vec<AwbcRuntimeValueSnapshot>,
}
```

Restore recursively snapshots arguments, recomputes canonical argument digest/NeedId, and validates active producer binding. Generic String is never accepted as this DTO.

## Limits

| Limit | Exact maximum | One-over behavior |
|---|---:|---|
| Match arms/site | 4096 | reject checked View product |
| Bindings/arm | 1024 | reject checked View product |
| Total Match outputs/View program | 65536 | reject section construction |
| Runtime type nesting | 64 | reject verification/digest |
| Need producer arguments | 256 | reject producer verification |
| Recursive snapshot values/handle | 65536 | reject snapshot/restore transaction |
| Canonical handle snapshot bytes | 16 MiB | reject before allocation/commit |
| Reactive selectors | 65536 | reject bundle validation |
| Reactive producers | 65536 | reject bundle validation |
| Reactive source-map rows | 262144 | reject bundle validation |

Limits are inclusive; exact-limit and one-over rows are mandatory.
