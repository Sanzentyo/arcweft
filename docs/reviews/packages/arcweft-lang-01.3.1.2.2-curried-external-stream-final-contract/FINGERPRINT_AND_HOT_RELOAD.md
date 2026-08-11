# Canonical fingerprints and hot reload

## 1. Hash primitive and scalar encoding

All fingerprints in this correction use BLAKE3 with domain separation. Hash input
is binary canonical data, never debug text, source text, JSON, `Display`, map
iteration order, or a host endpoint name.

Canonical scalar encoding for fingerprint input is:

```text
enum tag       = one u8
u16/u32/u64    = little-endian fixed width
bool           = 0x00 or 0x01
byte string    = u32 byte length followed by bytes
UTF-8 string   = u32 byte length followed by validated UTF-8 bytes
digest/hash    = 32 raw bytes
vector         = u32 item count followed by canonical items
optional       = 0x00, or 0x01 followed by the item
```

Every domain prefix is the exact ASCII bytes shown followed by one zero byte.

## 2. Runtime callable declaration digest

The compiler projects the typed HIR `CallableDeclarationId` once:

```text
BLAKE3(
  "arcweft.callable.declaration.v1\0" ||
  package || canonical_module_path || owner_tag || owner_path || name
)
```

Module and owner-path segments are encoded as vectors of UTF-8 segments. The owner
tag uses the HIR owner family, including `ExternCapability`; aliases/re-exports do
not enter the digest. This is a projection of existing typed identity, not a
second symbol resolver.

## 3. External Stream signature fingerprint

```text
BLAKE3(
  "arcweft.external-stream.signature.v1\0" ||
  definition_id || declaration_digest ||
  group_count ||
  for each group in group index order {
    group_index || group_kind_tag || parameter_count ||
    for each parameter in parameter index order {
      coordinate.group || coordinate.parameter ||
      optional_name || passing_tag || presence_tag ||
      optional_default_digest || type_layout_hash
    }
  } ||
  item_type_layout_hash || error_type_layout_hash ||
  effect_set_fingerprint || provider_abi_fingerprint
)
```

Parameter names are included because named and named-rest resolution is part of
the public callable contract. A default expression's canonical accepted-plan
digest is included, not its source spelling or span.

The stored fingerprint is recomputed and checked by RuntimePlan validation, AWBC
verification, bundle validation, save restore, and host-open validation whenever
the full signature metadata is available.

## 4. Argument/capture fingerprint

A prefix or full product has this digest:

```text
BLAKE3(
  "arcweft.external-stream.arguments.v1\0" ||
  definition_id || declaration_digest || generation_u64 || signature_digest ||
  completed_groups || cell_count ||
  for each cell in coordinate order {
    coordinate.group || coordinate.parameter || disposition_tag ||
    disposition_payload
  }
)
```

Disposition payloads are:

```text
Explicit:
  type_layout_hash || runtime_value_digest

Defaulted:
  default_expression_digest || type_layout_hash || runtime_value_digest

OmittedOptional:
  empty

RestPositional:
  item_type_layout_hash || item_count ||
  each (item_type_layout_hash || runtime_value_digest) in source order

RestNamed:
  value_type_layout_hash || entry_count ||
  each (name || item_type_layout_hash || runtime_value_digest)
  in canonical UTF-8 name order
```

`runtime_value_digest` is the existing ABI-2 canonical runtime value digest. It
includes affine identity according to the parent affine contract. Host JSON bytes
are not hashed.

A complete direct application and equivalent staged application must have equal
argument fingerprints. A prefix fingerprint is snapshot/session evidence and is
not part of the static bundle interface fingerprint.

## 5. Bundle/code fingerprints

The external Stream definition's interface digest includes:

- static definition ID;
- declaration digest;
- complete signature fingerprint;
- item/error handle layout;
- capability and operation typed IDs;
- provider ABI fingerprint;
- Stream policy layout/default fingerprint; and
- host request ABI digest.

The definition's code digest additionally includes every group application plan,
authored evaluation ordering, accepted default expression plan, RuntimePlan/AWBC
instruction sequence, and frame/register layout.

The program tables fingerprint includes the group-aware callable tables and the
sole Stream definition table. It contains no Source table and no flat final-group
projection.

## 6. Compatibility classification

The existing four classes remain authoritative. This correction supplies the
facts they compare.

| Change | Minimum classification |
| --- | --- |
| No executable/interface change; dialogue/content only | `ContentOnly` |
| Code changes while the complete external Stream signature, provider ABI, adapter requirements, and frame-visible layout are identical | `CodeCompatible` |
| Group count/order/kind changes | `CodeGenerational` |
| Parameter count/order/coordinate/name changes | `CodeGenerational` |
| Passing mode or rest kind changes | `CodeGenerational` |
| Required/optional/defaulted presence changes | `CodeGenerational` |
| Default-expression fingerprint changes | `CodeGenerational` |
| Parameter, item, or error type layout changes | `CodeGenerational` |
| External capability/operation provider ABI or adapter requirements change | `RestartRequired` under the existing adapter-requirement rule |
| AWBC ABI/codec or save schema mismatch | `RestartRequired` |
| Parent Stream policy change | parent policy classification, never less strict than the row it affects |

A group/parameter layout change never passes as code-compatible merely because
current call sites use only the final group.

## 7. Active partials and generation pins

Every external partial pins its exact `ProgramGeneration`. Hot reload never edits
its definition, signature, captured product, next group, or generation in place.

For an identical signature/layout, new calls use the new active generation while
an existing partial remains valid against its pinned old generation. The old
generation is retained until all partials, Stream instances, fibers, host tasks,
and snapshots release their pins. This remains true even when the overall swap is
classified `CodeCompatible`.

For a code-generational layout change:

- existing partials continue only against the retained old signature and provider
  binding;
- newly constructed callable values use the new signature;
- an old partial cannot accept a new-generation group plan; and
- no coordinate or captured value is translated between signatures.

If the required old generation is not retained, applying or restoring the partial
returns `StaleGeneration` before argument evaluation, affine movement, instance
allocation, or host request emission.

## 8. Cache identity and invalidation

Compiler query/cache keys that own external Stream lowering include the accepted
call fact generation/revision, declaration digest, signature fingerprint, group
index, authored expression identities, and default plan digests. They do not key
on source text snippets or provider display labels.

Changing any signature-fingerprint field invalidates RuntimePlan, AWBC, verifier,
bundle, host-ABI, and save-compatibility products that depend on it. Changing only
captured runtime values changes the session/snapshot argument fingerprint, not the
static compiled artifact fingerprint.
