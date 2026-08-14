# Required exact decisions 1–15 mapping

1. **CharacterCatalog digest owner/transcript** — `decision-01-character-catalog-digest.md` § Complete owner and transcript; outputs: CharacterCatalogRuntimeDigest; runtime_digest_v1; field transcript; limits; errors.
2. **ViewRegistry digest owner/transcript** — `decision-02-view-registry-digest.md` § Complete owner and transcript; outputs: ViewRegistryRuntimeDigest; public/anonymous/retired policy; implementation bytes; errors.
3. **catalog recomputation and admitted wrappers** — `decision-03-catalog-admission-api.md` § Generation-bound admission API; outputs: AdmittedGenerationCatalogs; admitted character/view borrows; mismatch precedence.
4. **role declaration owner/API/source evidence** — `decision-04-dialogue-role-declaration.md` § Rust-shaped declaration; outputs: CharacterDialogueRuntimeRoleDeclaration; constructor/accessors/errors/publication.
5. **exact Stage/Portrait/Focus/Cleanup/Hook/RichText/Style table** — `decision-05-dialogue-role-type-table.md` § Exact type table; outputs: six standard opaque nominal rows; derived ordered Style Choice.
6. **standard registration and role-coordinate substitution** — `decision-06-dialogue-role-registration.md` § Registration and projection; outputs: TypeKind::CharacterDialogueRole; exact once-only registration; no Named success.
7. **project/producer root facts and coordinate types** — `decision-07-root-facts-and-errors.md` § Bridge fact declarations; outputs: RuntimeProjectRootFact; RuntimeProducerFact; coordinate/source types; RuntimeProjectRootError.
8. **root-ID creation grammar** — `decision-08-root-id-grammar.md` § Lossless root identity; outputs: RuntimeSemanticTypeId byte copy; canonical 32-byte coordinate grammar.
9. **exhaustive RuntimePlan root mapping** — `decision-09-runtime-plan-root-mapping.md` § Typed site model and table; outputs: RuntimePlanTypedSite; RuntimePlanTypedRootUse; RUNTIME_PLAN_ROOT_MAPPING.csv.
10. **exhaustive AWBC mapping and plan correlation** — `decision-10-awbc-root-correlation.md` § AWBC typed-site model and equality; outputs: AwbcRuntimeTypeDeclaration; AwbcTypedRootUse; AWBC_ROOT_MAPPING.csv.
11. **project nominal construction authority** — `decision-11-project-construction-authority.md` § Borrowed admission domain; outputs: RuntimeNominalRecordAdmissionDomain; project domain issuance; errors/lifetimes.
12. **AWBC MakeRecord authority/wire/lowering** — `decision-12-awbc-make-record-wire.md` § Domain table and opcode payload; outputs: AwbcNominalRecordDomainDeclaration; LE bytes; verifier/admission/VM path.
13. **typed checked-value and Choice validation** — `decision-13-checked-value-validation.md` § Validator, paths, errors, budget; outputs: validate_value; RuntimeCheckedTypeError; branch mismatch; deterministic order.
14. **typed error mapping to all consumers** — `decision-14-error-mapping.md` § Structured mappings; outputs: ERROR_MAPPING.csv; no string flattening; boolean convenience boundary.
15. **compile-clean implementation/deletion order** — `decision-15-implementation-order.md` § Ordered implementation cut; outputs: 15-phase owner order; deletion gates; acceptance commands.
