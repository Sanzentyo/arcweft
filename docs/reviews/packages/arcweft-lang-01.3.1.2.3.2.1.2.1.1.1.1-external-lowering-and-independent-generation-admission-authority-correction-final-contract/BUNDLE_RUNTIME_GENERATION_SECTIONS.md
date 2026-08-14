# Product AWFB runtime-generation sections and exact fact grammar

## Existing owner, in-place enum extension

Owner: `crates/arcweft-bundle/src/container.rs`. Do not add a parallel section
kind enum or a string resolver. Extend the existing Arcweft-owned enum and its
original inherent `impl` in place:

```rust
pub enum BundleSectionKind {
    // existing variants 1..=22 remain unchanged
    RuntimeGenerationFacts, // encoded() == 23
    RuntimePlan,            // encoded() == 24
}
```

`BundleSectionKind::encoded` and `from_encoded` map exactly `23` and `24`.
`is_executable` returns true for both; `default_residency` is `Startup`;
`patch_default_compatibility` is `RestartRequired`; Program and
AgentController allow them, while ContentPack and Patch do not. Add both to the
existing `REQUIRED_PROGRAM_SECTIONS`; AWBC remains the existing required
`ProgramBytecode = 1` section. Every descriptor has `schema_version == 1`.
There is no `V2`, optional fallback, alias, or old reader.

The existing canonical section ID formula remains the sole owner:

```text
section_id(kind) = u32_le(kind.encoded())
                 || blake3("arcweft-awfb-v1-section")[0..12]
```

The writer emits the three runtime-generation payloads from one admitted
product through one checked bundle boundary:

```rust
pub struct RuntimeGenerationProductSections {
    generation_facts: SectionInput,
    plan: SectionInput,
    awbc: SectionInput,
}

impl RuntimeGenerationProductSections {
    pub fn try_from_product(
        product: &AdmittedRuntimeProduct,
    ) -> Result<Self, RuntimeGenerationSectionEncodeError>;

    pub fn into_sections(self) -> [SectionInput; 3];
}
```

`try_from_product` calls `product.generation().fact_section()`, serializes
`product.plan().plan()` through the checked RuntimePlan v1 writer, serializes
`product.awbc().program()` through the checked AWBC v1 writer, and creates
required `SectionInput`s for kinds 23, 24, and 1. It performs all serialization
before returning and publishes no partial vector. No overload accepts three
unrelated raw artifacts or a caller-provided generation declaration.

## Generation fact payload

The `RuntimeGenerationFacts` decoded payload is exactly:

```text
magic[8]             = 41 57 47 46 0d 0a 1a 0a   # "AWGF\r\n\x1a\n"
section_version      = u32_le(1)
transcript_len       = u32_le(N)
transcript           = N bytes
EOF                   # no trailing byte
```

`transcript` is exactly the canonical generation transcript in
`GENERATION_ISSUANCE_API.md`:

```text
"arcweft.runtime-generation\0" || u16_le(1)
|| nominal_catalog_digest[32]
|| character_catalog_digest[32]
|| view_catalog_digest[32]
|| dialogue_custom_field_digest[32]
|| u32_le(project_count) || project rows
|| u32_le(producer_count) || producer rows
|| u32_le(nominal_count) || nominal rows
```

Project rows are
`semantic_identity[32] || kind_tag:u8 || kind_payload` where tag `0` uses the
self-delimiting checked-type v1 grammar and tag `1` uses exactly one
`RuntimeOperationalType` canonical tag. Producer rows are
`u16_le(producer_utf8_len) || producer_utf8 || semantic_identity[32] || 0x00`;
`0x00` is `ExactIdentity`, and ProducerWide is rejected. Nominal rows are
`u16_le(nominal_utf8_len) || nominal_utf8 || semantic_identity[32] ||
layout_hash[32] || u32_le(field_count) ||
(field_id:u32_le || field_semantic_identity[32])*` in one-based defining-field
order.

The decoder validates magic, both version markers, checked integer conversion,
UTF-8/newtype construction, configured byte/count/node limits, exact tags,
strict row order, duplicate keys/fields, checked-type grammar, exact EOF, and
then invokes only the checked projection constructors and
`RuntimeGenerationProjectionBuilder::finish`. It never reads the RuntimePlan
or ProgramBytecode payload to fill a missing fact.

## Verified section token

`verify_runtime_generation_sections` first uses the existing `BundleView` and
`ContainerError` owners for AWFB magic/version/header/index/file length,
section bounds and overlap, read budget, stored/content/index/manifest digests,
compression, and external-payload validation. It then requires exactly one
kind 23, one kind 24, and one kind 1 descriptor, each schema version 1, and
owns their decoded bytes in `VerifiedRuntimeGenerationSections`.

The token has private fields, no Serde, Clone, Default, raw-byte accessor,
constructor, mutation, or replacement method. Its only payload operations are
`decode_generation_projection`, `decode_runtime_plan`, and
`decode_awbc_program`; each invokes the corresponding checked v1 decoder.
This allows the separate runtime-driver crate to consume verified data without
reintroducing friend-crate visibility or accepting caller-substituted slices.
