# Decision register

| ID | Decision | Selection | Reason |
|---|---|---|---|
| D-001 | Final schema replacement | RuntimeNominalRecordLayout | Executable layout, not persistence schema |
| D-002 | Absent schema name | Do not declare or alias RuntimeNominalRecordSchema | Avoid compatibility fiction |
| D-003 | Layout owner layer | arcweft_core::value::nominal_record | Core vocabulary without reverse dependency |
| D-004 | Layout fields | nominal, semantic_identity, layout, boxed defining-order fields | Single immutable descriptor |
| D-005 | Layout field contents | diagnostic name + RuntimeCheckedType | No copied RuntimeTypeSchema |
| D-006 | Field identity storage in layout | derive from ordinal; no vector | Accepted one-based owner remains sole mapping |
| D-007 | Sharing | runtime-plan Arc | Values do not retain schema |
| D-008 | Equality | structural; pointer identity irrelevant | Deterministic and serializable |
| D-009 | Canonical value identity | RuntimeNominalTypeId + TypeLayoutHash | Preserves current carrier/bytes |
| D-010 | Semantic identity | projection provenance only | Not added to runtime value bytes |
| D-011 | Role relationship | exact scalar cross-check; no schema copy | Entry owner remains separate |
| D-012 | Sema relationship | retain CheckedProjectNominal | No sema object in core |
| D-013 | Resolved nominal | add TypeLayoutHash | Checked nominal predicates need exact layout |
| D-014 | Record fact | RuntimeResolvedNominalRecord | Pairs nominal fact with one descriptor |
| D-015 | Old fact maps | delete in same cut | No dual fact reader |
| D-016 | Checked nominal type | add layout field | Fix type-only acceptance defect |
| D-017 | Value predicate | RuntimeCheckedType inherent accepts_value | Owner behavior, no helper trait |
| D-018 | Nominal expression | new checked RuntimeExpr variant | Prevent identity loss |
| D-019 | Initializer storage order | authored order | Preserve effects |
| D-020 | Final value order | layout order via ephemeral scatter | Preserve nominal representation |
| D-021 | Name admission owner | RuntimeNominalRecordExpr constructor | Value ctor has no names |
| D-022 | Value admission owner | RuntimeNominalRecordValue::try_from_accepted_layout | Count/ID/type checks |
| D-023 | Restore validation | validate_against_layout | Identity/layout/count/type precedence |
| D-024 | Unchecked new | delete | No compatibility constructor |
| D-025 | validate_shape | delete | Descriptor-aware validator replaces it |
| D-026 | Nominal pattern owner | Arc<RuntimeNominalRecordLayout> | Name-to-ID authority |
| D-027 | Positional pattern zip | delete | Incorrect for reordered fields |
| D-028 | Record sequence error | existing RuntimeSeqError | No absent/new error owner |
| D-029 | Record seq variants | add TooManyRecordFields/InvalidRecordFieldIdentity | Complete typed failure surface |
| D-030 | Record seq precedence | count; per ordinal ID, length, duplicate | Preserve current length-before-duplicate |
| D-031 | RecordSeq::new | delete | Raw carrier construction prohibited |
| D-032 | RuntimeSeq::record_columns | pair input; RuntimeSeqError | Public admitted replacement |
| D-033 | Anonymous field carrier | private field/name/value | Accepted parent shape |
| D-034 | Column field carrier | private field/name/values | Accepted parent shape |
| D-035 | Nominal field IDs | derive from layout ordinal | No side vector |
| D-036 | Shared visitor | one path-aware owner | Consumer convergence |
| D-037 | Visitor names | never used | No fallback recovery |
| D-038 | Canonical bytes | unchanged | No identity/codec redesign |
| D-039 | Plan Serde | atomic internal replacement | No dual reader/version allocation |
| D-040 | AWBC | ABI 1/codec 8 | Parent decision preserved |
| D-041 | Interim traits | retain required Clone/Serde | Compile-clean target |
| D-042 | Final trait removal | accepted parent stages | No new compatibility boundary |
| D-043 | Open questions | zero | Implementation-ready |
| D-044 | Layout hash producer | RuntimeTypeSchema::try_layout_hash via existing RuntimeSchemaProjection | Exact repository owner; checked sema digest is parity witness, not a substitute |
