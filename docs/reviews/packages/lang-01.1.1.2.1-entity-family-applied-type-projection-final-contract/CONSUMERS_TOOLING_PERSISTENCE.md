# Consumer, tooling, bytecode, and persistence contract

## Normal checker

All authored type-bearing surfaces call the accepted recursive resolver and use
its recovered `TypeKind`. The implementation must migrate any remaining
context-free `TypeRef -> TypeKind` conversion before deleting its `Ref`
spelling branch. There is no temporary dual path.

The current pass fixtures containing `Ref<Flow>` in project struct fields must
pass semantically after this correction rather than through an opaque fallback.

## Callable schemas

`ProjectSignatureResolver` remains the single callable-signature adapter:

- bind exact source-backed `AuthoredTypeRef`;
- build `TypeResolutionInput::accepted`;
- use `CheckedTypeReferenceCache`;
- record the full report in `NominalResolutionIndex`;
- consume `Complete` or poisoned recovered `TypeKind` according to the existing
  callable build policy;
- reject detached project signatures.

A parameter or return annotated `Ref<Character>`/`Ref<Flow>` is exactly
`TypeKind::Ref(EntityType { value: None, ... })` in `FunctionSignature`.

## Entry contracts versus persisted entry data

These are distinct boundaries:

1. **Entry-bound callable contract:** may use checked `TypeKind::Ref` in role
   signatures and comparisons.
2. **Persisted state/event data shape:** the existing
   `NominalSchemaExpander` projects checked `TypeKind` into
   `arcweft_data::TypeShape`. Because `TypeShape` has no typed entity-reference
   variant, `TypeKind::Ref` remains unsupported at this boundary.

The persistence error remains deterministic and typed through the existing
unsupported canonical persisted data shape path. Implementations must not
coerce it to `TypeShape::Named`, `String`, integer ID, or an opaque record.
Adding a persisted entity-reference shape, ID codec, compatibility policy, and
schema-version transition is explicitly outside this correction.

## Nominal and project indexes

The single `NominalResolutionIndex` retains:

- root `Builtin(Ref)` node;
- child `EntityFamily(E)` node;
- exact local/project source evidence;
- poison and detached facts.

`recovered_node_type` remains `None` for an entity-family child because it is
not a `TypeKind`.

`ProjectSemanticIndex` may contain `TypeKind::Ref` in checked callable/type
values. `checked_project_nominals` continues to emit reference edges only for
final `Project` and `Alias` outcomes. Therefore:

- valid `Ref<E>` emits no project-nominal edge;
- invalid `Ref<ProjectType>` emits no edge after the child outcome is replaced
  with `Failed(WrongArgumentKind)`;
- a valid use of the same project type outside a contextual slot still emits
  its normal edge.

No project semantic index schema bump is required solely for this correction.
The accepted LSP snapshot already retains the full `TypeCheckReport`, so
contextual node facts need not be duplicated into a second serialized index.

## LSP policy

### Hover

Using exact resolver node facts:

- on `Ref`: show a language-owned type constructor, exact signature
  `Ref<EntityFamily>`, and normalized result label when complete;
- on `Character`/`Flow` inside a valid slot: show “entity family” plus the
  canonical family name;
- on an invalid ordinary argument: show its existing diagnostic/type hover,
  not an entity-family claim.

### Completion

- ordinary type-position completion includes `Ref`, `Speaker`, and
  `SpeakerPreset` from `BuiltinTypeConstructor::ALL`;
- inside an argument position whose typed expectation is `EntityFamily`, list
  exactly `EntityKind::AUTHORED_FAMILIES` in deterministic canonical-name
  order;
- exclude `EntityKind::Other`, project nominals, externals, accepted exact
  records, open rules, and generic parameters from the entity-family-only list;
- do not infer context from raw prefix text or uppercase heuristics.

### Definition

`Ref` and fixed entity-family atoms have no project source declaration. Return
no location; do not fabricate a virtual source or redirect to a same-spelled
project nominal. Qualified non-contextual names retain existing behavior.

### Rename

`prepareRename` and `rename` return no edit for:

- `Ref`;
- `Speaker` / `SpeakerPreset` as language-owned constructors;
- fixed entity-family atoms in contextual slots;
- an invalid project nominal used as an entity-family argument after its final
  node outcome becomes `Failed`.

The same project nominal remains renamable at valid non-contextual uses.

## Runtime-plan and verification

No new runtime value or verifier syntax is introduced. Consumers that compare
or classify semantic types use the existing `TypeKind::Ref(EntityType)` and
`EntityKind` values. They must not parse `source_label()` back into identity.
The optional `EntityType::value` remains an internal semantic capability and is
`None` for authored `Ref<E>`.

## Bytecode and persistent interface digest

The repository’s persistent compiler digest records authored `TypeRef`
structurally, including generic base and arguments. Consequently:

- `Ref<Character>` and `Ref<Flow>` have deterministic, distinct authored
  signature digests;
- no bytecode `TypeKind` wire variant is added by this correction;
- no bytecode schema version changes;
- no old/new dual reader;
- no display-string encoding.

Tests must verify digest stability and distinction through the typed public or
crate-owned API. They must not inspect source text as an architecture gate.

## Save/replay

No semantic `TypeKind::Ref` crosses the existing save/replay data-shape boundary
in this slice. Required tests assert the deterministic unsupported-shape result
for persisted data and the absence of a schema change. A round-trip test becomes
mandatory only in the future contract that introduces a typed entity-reference
wire shape.
