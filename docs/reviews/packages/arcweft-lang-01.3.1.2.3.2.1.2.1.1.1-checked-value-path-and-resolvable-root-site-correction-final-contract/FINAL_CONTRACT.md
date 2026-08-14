# Final design contract

## Authority hierarchy

The accepted generation contract remains the only generation authority. The compiler/runtime-plan bridge creates `AdmittedRuntimeGeneration` from independently accepted semantic project facts, producer root facts, canonical Character/View/custom catalog digests, and the retained version-1 generation declaration. Raw plan and AWBC artifacts remain quarantine data.

Plan admission and AWBC admission never trust a serialized root-use row. They resolve exact types from the real current owner fields described by `RUNTIME_PLAN_SITE_RESOLUTION.csv` and `AWBC_SITE_RESOLUTION.csv`; they then compare the resulting semantic ID, checked type, and project/producer domain to the already-admitted generation. A raw artifact can describe data, but cannot extend the admitted generation.

## One value-path authority

`RuntimeValuePath` in `value::ownership::path` is the only canonical, ordered, Serde-capable runtime-value graph path. Existing tags 0 through 9 retain their meaning and wire shape; `OpaquePayload` is tag 10. Checked validation does not redeclare these names. `RuntimeCheckedTypePath` is a separate, non-Serde diagnostic path for edges that exist only in the expected checked-type graph, including Choice alternatives and Result/Option branches.

## Mechanically resolvable plan facts

Every expression-bearing and pattern-bearing current owner receives `RuntimeTypedExpr` or `RuntimeTypedPattern`, whose complete node fact tables are keyed by validated `RuntimeIndexPath`. Fixed signature-like owners receive mandatory `RuntimePlanTypeId` fields. Admission traverses the actual enum, derives the exact expected node-path set and scope/signature constraints, rejects missing/extra/duplicate rows, and resolves each type declaration against the admitted generation. The in-memory `RuntimePlanTypedSite` map is derived output, not serialized authority.

## Mechanically resolvable AWBC facts

AWBC runtime types are declarations validated against the admitted generation; constants and patterns retain mandatory type IDs in their own owners. Register types are obtained only from the owning function's frame layout. Signatures, functions, task/effect/audio/stream/source/entry tables and every typed instruction/terminator field resolve through named slot enums. All vector indices have a normative field-specific mapping. Reference bounds, duplicate tables, canonical ranges, type alias equality, and cycle checks complete before root correlation.

## Tamper resistance

The plan side and AWBC side are independently resolved. Plan-paired admission compares the direct canonical row transcript in `PLAN_AWBC_EQUALITY_GRAMMAR.md`, using the fully concrete `RUNTIME_CHECKED_TYPE_V1_BYTE_GRAMMAR.md` and exact site/slot tag CSVs; no new digest or root map is added. `AwbcTypedOrigin` contains only coordinates. Changing an origin, a runtime type declaration, a constant type, a frame slot, a signature, a pattern expected type, or both a claimed coordinate and a declaration cannot create authority because the corresponding actual owner and the admitted generation are checked independently.

## Admitted wrappers and execution cut

`ADMISSION_AND_PAIR_API.md` fixes the exact opaque publication owners. `RuntimePlan::try_admit` produces `AdmittedRuntimePlan`; `AwbcProgram::try_admit` produces standalone `AdmittedAwbcProduct`; and consuming `AdmittedRuntimePlan::try_admit_awbc` admits the candidate through the existing plan generation and returns `AdmittedRuntimeProduct` only after direct site equality. The generation parent is an immutable private `Arc` aggregate and same-parent comparison is pointer identity, not a caller-reconstructed scalar. Resolved type rows and pair-correlation rows are derived, non-Serde state.

`AwbcProductStepExecutor` owns `AdmittedAwbcProduct`. Its constructors, replacement, and accessor use the admitted wrapper; current raw `AwbcProgram` ownership, `program()`, and `replace_program_preserving_state` are deleted in the same compile-clean execution cut. VM, fiber, runtime-driver, player, restore, and hot-swap consumers follow the same wrapper boundary before any execution or publication.

## Validation and nominal evidence

`RuntimeNominalRecordAdmissionDomain::checked_values()` is the only public way to obtain a borrowed `RuntimeCheckedValueContext`; the domain and generation provenance are inseparable. `RuntimeCheckedType::validate_value` consumes that context. The validator shares one 65,536-unit budget and one depth count across every Choice branch and nominal traversal. Every current physical `RuntimeValue` maps to one exact `RuntimeValueShape`. Physical byte buffers are sequences; `Bytes` validation checks a sequence whose elements satisfy `Unsigned(U8)`. A raw nominal record has no semantic ID; descriptor lookup keyed by the expected semantic ID supplies the actual admitted semantic evidence.

## Catalog and role issuance

`arcweft-dialogue` owns `CharacterDialogueGenerationCatalogs<'generation>` because it already depends on core, character, and View while runtime-driver may depend on dialogue. Core supplies an opaque generation-provenance token after exact digest comparison. Character catalogs do not encode Character-to-View relationships, so that unsupported check is removed. Custom-field accepted View IDs remain checked from the accepted custom-field registry.

`TypeCheckEnv` registers six standard role nominals before constructing `AcceptedNominalWorld`. The world atomically projects the exact declarations into `CharacterDialogueRuntimeRoleRegistry`; callable publication substitutes typed role coordinates through that registry. `RegisteredTypeCheckEnv` publishes nominal world, role registry, custom-field registry, metadata, and callables in one allocation. Style remains derived and cannot be registered.

## Version and migration rule

Every Arcweft-owned schema, ABI, codec, digest-domain, protocol, persistence, save, and snapshot version remains `1`. This is an unreleased direct replacement. No alias, dual enum, translation shim, optional authority field, old reader, defaulted coordinate, fallback root, public generation-erasing handle, or source-name recognizer remains.
