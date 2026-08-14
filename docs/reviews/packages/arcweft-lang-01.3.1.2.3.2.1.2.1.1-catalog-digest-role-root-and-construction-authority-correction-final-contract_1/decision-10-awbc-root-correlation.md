# Decision 10 — exhaustive AWBC root mapping and plan correlation

## Runtime type declarations and root uses

Owner: `arcweft_core::awbc::schema`.

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcRuntimeTypeDeclaration {
    semantic_identity: RuntimeSemanticTypeId,
    checked_type: AwbcRuntimeType,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "table", rename_all = "snake_case", deny_unknown_fields)]
pub enum AwbcTypedSite {
    RuntimeType { ty: u32 },
    Constant { constant: u32 },
    SignatureParameter { signature: u32, parameter: u32 },
    SignatureResult { signature: u32 },
    FrameSlot { frame: u32, slot: u32 },
    InstructionInput { function: u32, block: u32, instruction: u32, input: u32 },
    InstructionResult { function: u32, block: u32, instruction: u32 },
    PatternExpected { pattern: u32 },
    PatternRecordField { pattern: u32, field: u32 },
    PatternVariantPayload { pattern: u32 },
    TaskPlan { task: u32, slot: u32 },
    AudioCommand { command: u32, slot: u32 },
    EffectPlan { effect: u32, slot: u32 },
    ChoiceOption { choice: u32, option: u32 },
    ContentUnit { content: u32 },
    LineTaskNode { group: u32, node: u32, slot: u32 },
    StreamPlan { plan: u32, slot: u32 },
    SourcePlan { plan: u32, slot: u32 },
    PureHelper { helper: u32, slot: u32 },
    TraitMethod { method: u32, slot: u32 },
    ResourcePayload { resource: u32 },
    CallableExecutable { executable: u32, slot: u32 },
    FlowBinding { binding: u32 },
    FlowExecutable { executable: u32, slot: u32 },
    Entry { entry: u32, slot: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AwbcTypedRootUse {
    plan_site: RuntimePlanTypedSite,
    awbc_site: AwbcTypedSite,
    root: RuntimeProjectRootId,
    ty: AwbcTypeId,
}
```

Directly replace `AwbcProgram.runtime_types: Vec<AwbcRuntimeType>` with `Vec<AwbcRuntimeTypeDeclaration>` and add mandatory `typed_root_uses: Vec<AwbcTypedRootUse>`. Fields are private; no deserialization default. Runtime-type dense IDs are artifact-local indices only.

## Dense-ID resolution and canonical order

For each use, admission checks `ty` bounds, reads `runtime_types[ty].semantic_identity`, requires `RuntimeProjectRootId::from_semantic_type(...) == root`, projects `checked_type` into the canonical core checked type, and compares it with the generation root declaration. Root-use rows are canonical in `(plan_site, awbc_site)` order; duplicate plan or AWBC site is an error. Reordering the runtime-type table while remapping every dense reference does not change semantic root identity; failing to remap any reference fails structural verification before correlation.

## Plan-to-AWBC preservation

Lowering consumes an admitted/canonical raw `RuntimePlan` candidate. For every plan `RuntimePlanTypedRootUse`, it emits at least one exact AWBC use for each resulting AWBC typed site and copies the `plan_site` and `root` unchanged. Pair admission compares sorted tuples:

```text
canonical(RuntimePlanTypedSite)
|| RuntimeProjectRootId[32]
|| RuntimeSemanticTypeId[32]
|| canonical RuntimeCheckedType bytes
```

The plan side tuple is compared byte-for-byte with the AWBC side tuple after dense resolution. No separate digest is introduced. A plan site omitted by lowering, substituted with another root, duplicated, or associated with a different canonical checked type fails before AWBC operational publication. Standalone AWBC admission uses the embedded `RuntimeGenerationContractDeclaration` and retained `plan_site` rows as the same correlation evidence.

`AWBC_ROOT_MAPPING.csv` maps every current AWBC table. Display/source maps and identity-only tables are deliberate exclusions.
