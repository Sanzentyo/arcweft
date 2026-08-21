# Layout and persistence contract

## 1. Selected contract

Project nominal values have one layout contract in both transient execution and persistence:

```text
RuntimeTypeSchema -> RuntimeTypeSchema::try_layout_hash -> TypeLayoutHash
```

The exact nominal declaration identity and semantic identity remain separate validation fields. Neither one is used as the layout hash.

## 2. Why no second layout family is added

The concrete repository blocker is caused by projecting an unreachable function. Excluding that function closes fixture 013 without weakening opaque identity or altering the accepted nominal-record value contract.

A second transient layout family would require changes to `RuntimeCheckedType::Nominal`, `RuntimeNominalRecordLayout`, `RuntimeNominalRecordValue`, RuntimePlan type projections, AWBC type rows, VM/native parity, canonical bytes, external admission, and save/replay. The request does not provide a reachable use case that justifies reopening those accepted contracts. Therefore this cut preserves the single schema-derived authority.

## 3. Admission matrix

| Checked shape | Runtime checked type outside project nominal | Project nominal field/payload | Entry/persisted schema | AWBC session snapshot |
|---|---|---|---|---|
| primitive/closed structural | admitted | admitted | admitted when schema-supported | admitted |
| accepted opaque atom | `RuntimeCheckedType::Opaque` | rejected: no closed schema | rejected | producer-owned snapshot path, validated against AWBC type |
| `Option<Opaque>` | recursively admitted | rejected at nested opaque path | rejected | admitted only where enclosing AWBC checked type permits |
| `Result<Opaque, Opaque>` | recursively admitted | rejected | rejected | same |
| `Vec<Opaque>` / sequence | recursively admitted | rejected | rejected | same |
| tuple containing opaque | recursively admitted | rejected | rejected | same |
| project struct with only closed fields | nominal record with schema-derived layout | admitted | admitted | admitted |
| project struct with opaque leaf | n/a as project value | rejected | rejected | cannot be emitted |
| project enum with opaque payload | n/a as project value | rejected | rejected | cannot be emitted |
| unreachable declaration of either project nominal | no runtime projection occurs | no decision needed | no Entry root means no schema projection | no emitted state |

## 4. Typed schema path

Every descent into a project nominal schema carries `NominalSchemaPath`.

Examples:

```text
OpeningAssets.bg
  [Field { ordinal: 0, name: "bg" }]

Envelope.assets?.bg
  [Field("assets"), OptionItem, NestedNominal(OpeningAssets), Field("bg")]

Payloads[variant Ready].images[item]
  [VariantPayload("Ready"), Field("images"), SequenceItem]
```

At an accepted opaque atom the error is:

```text
NominalSchemaProjectionError::OpaqueLeaf {
    path,
    producer,
    semantic_identity,
}
```

No opaque `RuntimeTypeSchema` is returned.

## 5. Compiler mapping

For a reached non-suspending owner that requires the nominal, the compiler maps the typed sema error to:

```text
RuntimeSemanticProjectionError::OpaqueProjectNominalLayout {
    nominal,
    path,
    producer,
    semantic_identity,
}
```

Suggested stable diagnostic code:

`compiler.runtime_nominal.opaque_leaf_has_no_schema_layout`

The message explains that the opaque type is valid but the enclosing project nominal has no admitted schema-derived layout. It must not describe the type as unknown.

## 6. Error precedence inside a closed nominal

For a closed nominal value, existing precedence remains:

1. active catalog/descriptor lookup;
2. nominal identity;
3. semantic identity where owned by the descriptor/catalog boundary;
4. `TypeLayoutHash`;
5. field/case count;
6. defining-order field/case identity;
7. first checked child failure;
8. publication.

The reachability and unsupported-function preflight precede all of these at compile time.

## 7. No implicit conversion proof

Because transient and persisted project nominals use the same `TypeLayoutHash`, there is no `TransientNominal`, no `PersistedNominal`, and no conversion API to audit. The implementation must not add `From`, `Into`, `AsRef`, serde adapters, or unchecked constructors that bypass schema projection.

Opaque-containing project nominals cannot be constructed by accepted compiler/runtime paths. Existing raw deserialization remains quarantined and must pass the active catalog/descriptor validation before publication.

## 8. AWBC/native parity

No AWBC schema field changes:

- `AwbcRuntimeType::Nominal.layout` and `NominalRecord.layout` remain the schema-derived bytes;
- `AwbcRuntimeType::Opaque` retains producer, semantic identity, admission, and arguments;
- `AwbcProgram::nominal_record_layout` reconstructs the same descriptor;
- native execution consumes the same admitted RuntimePlan descriptor;
- parity compares the exact recursive checked type graph and the same `TypeLayoutHash` for closed project nominals.

A project nominal with an opaque leaf is rejected before either backend, so both backends observe the same failure rather than diverging representations.

## 9. Save/replay

Unreachable functions produce no AWBC function, frame layout, registers, resume points, or closure values. They cannot appear in a session snapshot.

For legal opaque runtime values outside project nominals, the existing AWBC save DTO retains producer and semantic identity. Save and restore validate against the active verified program and exact frame slot type before publication. A mismatch is a typed invalid-runtime-value/save error with a value path.

For closed project nominal snapshots, `type_id` and schema-derived layout are verified against the active program descriptor. Wrong layout, field count, or field order rejects before restoring a live value.

No compatibility reader, old/new branch, or version increment is introduced.
