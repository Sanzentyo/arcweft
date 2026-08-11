# Producer / consumer / deletion inventory

This inventory is normative. `exact source` means the retained exact-commit
source bytes were inspected. `exact-commit evidence` means the immutable GitHub
blob/tree at `2585f527b02808305b3a8cab0442eb522e8d0352` was inspected. `compile closure` means the owner
is required by the request or exhaustive Rust enum/type migration; its exact
call sites must be confirmed by workspace compilation at the named gate.

| ID | Layer | Owner/path | Symbol or behavior | Target baseline | Required action | Gate | Evidence |
|---|---|---|---|---|---|---|---|
| INV-001 | core value | arcweft-core/src/value/nominal_record.rs | RuntimeNominalRecordValue | type_id/layout/fields + public unchecked new | retain shape; add admitted ctor/validator/field ID access | A4 | exact source |
| INV-002 | core value | arcweft-core/src/value/nominal_record.rs | RuntimeNominalRecordError | Type/Layout/FieldCount | add InvalidFieldIdentity/FieldType with fixed precedence | A4 | exact source |
| INV-003 | core value | arcweft-core/src/value/nominal_record.rs | RuntimeNominalRecordLayout | absent | add sole executable layout owner | A1 | request + exact absence evidence |
| INV-004 | core value | arcweft-core/src/value/nominal_record.rs | RuntimeNominalRecordLayoutField | absent | add private-field checked projection | A1 | design closure |
| INV-005 | core value | arcweft-core/src/value.rs | RuntimeFieldValue | public name/value | add private accepted field ID and inherent accessors | A3 | exact source + parent |
| INV-006 | core value | arcweft-core/src/value.rs | RuntimeValue::Record | raw field vector | retain variant; all producers use try_record | A3 | exact source |
| INV-007 | core value | arcweft-core/src/value.rs | RuntimeValue::NominalRecord | existing carrier | retain variant and canonical distinction | A4 | exact source |
| INV-008 | core value | arcweft-core/src/value.rs | RuntimeRecordAdmissionError | absent in inspected target | add accepted parent enum | A3 | parent contract |
| INV-009 | core value | arcweft-core/src/value.rs | RuntimeExpr::Record | anonymous only | retain for anonymous records only | A2 | exact source |
| INV-010 | core value | arcweft-core/src/value.rs | RuntimeExpr::NominalRecord | absent | add checked nominal expression carrier | A2 | exact source defect |
| INV-011 | core value | arcweft-core/src/value.rs | RecordSeqField | public name/values | add private ID and accessors | A3 | exact source + parent |
| INV-012 | core value | arcweft-core/src/value.rs | RuntimeSeqError | ColumnLength/DuplicateRecordField | retain owner; add count/identity variants | A3 | exact source |
| INV-013 | core value | arcweft-core/src/value/sequence_impls.rs | RecordSeq::new | raw public constructor | replace and delete with accepted fields ctor | A3 | exact source |
| INV-014 | core value | arcweft-core/src/value/sequence_impls.rs | RuntimeSeq::record_columns | accepts Vec<RecordSeqField> | change to name/sequence pairs; sole RuntimeSeqError | A3 | exact source |
| INV-015 | core value | arcweft-core/src/value/sequence_impls.rs | RecordSeq row reconstruction | rebuilds raw RuntimeFieldValue names | preserve stored IDs in into_values/tail/value_at | A3 | exact source |
| INV-016 | core value | arcweft-core/src/value.rs | record_rows_to_columnar | compares names and builds raw fields | require ID+name+order equality; preserve IDs | A3 | exact source |
| INV-017 | core pattern | arcweft-core/src/pattern.rs | RuntimeCheckedType::Nominal | nominal + semantic identity | add TypeLayoutHash | A1 | exact source defect |
| INV-018 | core pattern | arcweft-core/src/pattern.rs | runtime_value_matches_pattern_type | private free helper; nominal checks type only | move into RuntimeCheckedType::accepts_value; delete helper | A1 | exact source defect |
| INV-019 | core pattern | arcweft-core/src/pattern.rs | RuntimePattern::Record.owner | Option<RuntimeCheckedType> | replace with Option<Arc<RuntimeNominalRecordLayout>> | A2 | exact source |
| INV-020 | core pattern | arcweft-core/src/pattern.rs | nominal record matcher | positional pattern/value zip | validate layout and resolve names to IDs; delete zip | A2 | exact source defect |
| INV-021 | core ownership | arcweft-core/src/value/ownership | record traversal | separate current branches | consume carrier IDs through one visitor | A5 | required inventory + foundation evidence |
| INV-022 | core nesting | arcweft-core/src/value/nesting.rs | aggregate recursion | independent traversal | delegate to shared traversal contract or owner iterator | A5 | exact source anchor |
| INV-023 | core entry | arcweft-core/src/entry/roles.rs | RuntimeNominalRole | identity/layout/RuntimeTypeSchema | retain; explicit scalar cross-check only | A1 | exact source |
| INV-024 | core entry | arcweft-core/src/entry/schema.rs | RuntimeTypeSchema::try_layout_hash | existing sole BLAKE3 canonical schema hash owner | retain byte grammar; use for every nominal layout projection; never retain schema in executable layout | A1 | exact source lines 150-156, 434-585 |
| INV-025 | core plan | arcweft-core/src/plan.rs | RuntimePlan / expression persistence | embeds RuntimeExpr | validate nominal expression descriptor on ingress | A2/A6 | exact source anchor |
| INV-026 | core pure | arcweft-core pure evaluator/value evaluator | record expression evaluation | anonymous record path only | add authored-order scatter and admitted nominal value ctor | A2/A4 | required compile closure |
| INV-027 | core engine | structured engine execution | RuntimeExpr consumers | add exhaustive NominalRecord branch; no anonymous fallback | A2 | required compile closure |
| INV-028 | core AWBC | AWBC verifier | RuntimeExpr/RuntimeCheckedType validation | validate layout descriptor and field IDs | A2/A6 | required compile closure |
| INV-029 | core AWBC | AWBC VM | record construction/matching | execute nominal carrier and admitted value ctor | A2/A4 | required compile closure |
| INV-030 | core root/replay | root and replay validators | live RuntimeValue/RuntimePlan ingress | validate against active descriptor before traversal | A4/A6 | required compile closure |
| INV-031 | HIR | arcweft-lang-hir nominal declaration/record expr | defining field order + authored initializer names | retain sole source order facts | A1/A2 | exact-commit evidence |
| INV-032 | sema | arcweft-lang-sema final_analysis model | CheckedProjectNominal | retain declaration/args/semantic digest authority | A1 | exact-commit evidence |
| INV-033 | sema | project record checker | duplicate/missing/extra/type checks | retain; publish accepted facts, no core dependency | A1 | exact-commit evidence |
| INV-034 | runtime-plan | semantic_facts.rs | RuntimeResolvedNominal | no layout scalar | add TypeLayoutHash and checked projection | A1 | exact source |
| INV-035 | runtime-plan | semantic_facts.rs | nominals/pattern_nominals maps | record identity only | replace/delete with nominal record facts | A1 | exact source |
| INV-036 | runtime-plan | semantic_facts.rs | RuntimeNormalizedType::checked_type | nominal lacks layout | supply catalogued schema-derived layout; UnresolvedNominalLayout on absence | A1 | exact source |
| INV-037 | runtime-plan | final_expr.rs | HirExprKind::Record -> RuntimeExpr::Record | drops nominal identity/layout | lower to RuntimeExpr::NominalRecord; delete fallback | A2 | exact source defect |
| INV-038 | runtime-plan | final_pattern.rs | pattern nominal checked_type only | consume shared layout fact | A2 | exact-commit source evidence |
| INV-039 | runtime-plan | awbc_lower | checked type/expression projection | add exhaustive layout-aware nominal paths | A2/A6 | exact tree + compile closure |
| INV-040 | compiler | arcweft-compiler/src/project/entry_runtime.rs | RuntimeSchemaProjection::nominal | directly casts CheckedNominalRole.schema_digest bytes to TypeLayoutHash | project schema once; call try_layout_hash; compare digest; typed mismatch | A1 | exact source lines 490-503 |
| INV-040A | compiler | arcweft-compiler sema-to-runtime-plan bridge | ordinary nominal record layout producer absent | reuse RuntimeSchemaProjection::schema/layout_hash; project defining layout and Arc descriptor | A1 | exact absence + design closure |
| INV-040B | sema entry | arcweft-lang-sema/src/entry/digest.rs | nominal_schema(TypeShape) | existing BLAKE3 digest over same domain/version/tags/order | retain as checked witness; no runtime TypeLayoutHash construction | A1 | exact source lines 267-274, 407-580 |
| INV-041 | runtime codegen | arcweft-runtime-codegen | RuntimeExpr/plan consumers | consume new variant or reject typed unsupported path | A2/A6 | workspace crate + compile closure |
| INV-042 | runtime driver | arcweft-runtime-driver | activation/root/replay | validate plan/value layout before activation | A4/A6 | workspace crate + required inventory |
| INV-043 | bundle | arcweft-bundle | plan/value bundle codecs | serialize interim descriptor; validate on load; no dual reader | A6 | workspace crate + required inventory |
| INV-044 | save | arcweft-save | save/restore RuntimeValue | restore against active plan layout before owner traversal | A4/A6 | workspace crate + required inventory |
| INV-045 | tests | arcweft-core trybuild | raw field/unchecked ctor surfaces | add compile-fail cases | A3/A4 | foundation pattern |
| INV-046 | tests | workspace integration | canonical bytes/visitor/parity | add matrices and full gates | A6 | request |


## Deletion completeness rule

At each gate, search output is discovery evidence only. Acceptance requires the
new typed owners plus compile/test success. Once all typed call sites migrate,
the old declaration/API is deleted immediately; no source-string grep is used
as a substitute for compilation.

Mandatory deletion symbols:

- public `RuntimeNominalRecordValue::new`;
- `RuntimeNominalRecordValue::validate_shape`;
- `RecordSeq::new`;
- old `RuntimeSeq::record_columns(Vec<RecordSeqField>)` signature;
- runtime-plan `push_nominal`, `push_pattern_nominal`, `nominal`, and
  `pattern_nominal` record APIs;
- identity-only record fact maps;
- nominal record lowering to anonymous `RuntimeExpr::Record`;
- positional nominal pattern zip;
- private free `runtime_value_matches_pattern_type`; and
- every raw struct literal for `RuntimeFieldValue`, `RecordSeqField`, and the
  new nominal field-expression carrier.
