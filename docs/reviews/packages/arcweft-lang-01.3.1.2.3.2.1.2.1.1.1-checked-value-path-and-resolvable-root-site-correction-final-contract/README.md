# Lang-01.3.1.2.3.2.1.2.1.1.1 final contract

Status: `READY_FOR_IMPLEMENTATION`  
Open questions: `0`  
Evidence head: `36f83f8509417d1110a34f1b32aee6f4a113dcf3` on `main`  
Delivery: design-only; no production Rust source, patch, diff, overlay, compatibility reader, dual authority, or implementation artifact is present.

This archive is the mandatory narrow correction requested by `SOURCE_REQUEST.md`. It retains the accepted Character/View digest transcripts, six CharacterDialogue role meanings, generation/root scalar model, AWBC nominal-domain table, `MakeRecord` authority order, unique-Choice semantics, 65,536 work budget, nesting limit 64, and structured error propagation from the retained retry archive `e0aa31dfefa5bc0d9fab213d19fef6fd74a142cef6dd7d4e6922d05c077bc998`. It replaces only the retry decisions that the current request identifies as non-resolvable or layer-invalid.

## Final selections

1. The sole canonical value path remains `arcweft_core::value::ownership::path::RuntimeValuePath`; it gains exactly one edge, `OpaquePayload`, tag `10`. No second `RuntimeValuePath` exists in `pattern`.
2. Checked-type branch evidence uses a distinct non-Serde diagnostic `RuntimeCheckedTypePath`; Choice changes only the type path, while physical value edges continue to use the canonical value path.
3. Raw `RuntimePlanTypedRootUse` and `AwbcTypedRootUse` authority rows are deleted. Plan and AWBC type/root evidence is recomputed from mandatory typed fields on current table owners and checked against one independently admitted generation.
4. `AwbcTypedOrigin` retains only a plan site and an AWBC site; it carries no root, semantic ID, checked type, or dense type ID and therefore cannot authorize itself.
5. `AdmittedRuntimeGeneration` and its sealed checked-value context land before the validator. There is no placeholder generation, public validator constructor, temporary resolver, or boolean authority fallback.
6. Every current `RuntimeValue` family has an exact outer-shape row. Physical bytes are `RuntimeValue::Seq` and therefore report `Sequence`; `Bytes` remains only an expected checked shape.
7. Nominal semantic identity is obtained from the admitted descriptor selected by expected semantic identity after raw nominal/layout checks. It is never reconstructed from a name or layout hash and is never claimed to be stored on `RuntimeNominalRecordValue`.
8. Catalog admission is shared through a dialogue-owned bridge backed by a core generation-provenance token. No caller-provided target-generation scalar remains. The unsupported Character-to-View relationship check and `MissingCharacterView` error are removed.
9. Standard role issuance is integrated with the current `TypeCheckEnv`, `AcceptedNominalWorld`, registrar, and `RegisteredTypeCheckEnv`; no nonexistent `AcceptedNominalEnvironment` API remains.

## Reading order

1. `FINAL_CONTRACT.md`
2. `REQUEST_DECISION_MATRIX.md`
3. `decision-01-*.md` through `decision-12-*.md`
4. `ADMISSION_AND_PAIR_API.md`
5. `RUNTIME_CHECKED_TYPE_V1_BYTE_GRAMMAR.md`, `RUNTIME_PLAN_SLOT_ENUMS_AND_TAGS.md`, `AWBC_SLOT_ENUMS_AND_TAGS.md`, and the normative path/site/tag/equality/Serde/error tables
6. `IMPLEMENTATION_ORDER.md`, `ACCEPTANCE_COMMANDS.md`, and `VALIDATION_EVIDENCE.md`

`MANIFEST.sha256` covers every member except itself. Package verification distinguishes checks actually run from implementation-time commands that require a real checkout and production changes.
