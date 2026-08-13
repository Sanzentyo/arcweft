# Non-goals and prohibited alternatives

This correction does not:

- reopen A1-A3 nominal/anonymous record identity, layout, field-ID, authored
  evaluation, defining order, carrier, or canonical-byte decisions;
- make `RuntimeNominalRecordValue::try_from_accepted_layout` public;
- add a public raw nominal/layout/fields constructor;
- add `RuntimeCheckedType::Dynamic`, optional validation, a producerless opaque
  owner, or an arbitrary-value predicate;
- recognize `"DialogueStage"`, `"DialogueContent"`, `"RichTextStyle"`, or any
  other role spelling at runtime or in the compiler bridge;
- reconstruct checked types from names, schema strings, hashes, display labels,
  nominal IDs, source snippets, or debug output;
- add a global mutable catalog/registry, copied descriptor table, friend
  feature, extension-trait authority, source gate, or compatibility shim;
- make `arcweft-dialogue` depend on runtime-plan, compiler, sema, HIR, syntax,
  runtime-driver, or another higher layer;
- treat `GenerationId(u64)` as the semantic artifact identity;
- treat a lifetime alone as generation correlation;
- let a claimed producer key authorize itself;
- leave raw `AwbcProgram` executable while only RuntimePlan is admitted;
- retain public `Deref`, `into_inner`, raw replacement, or restore bypass on an
  admitted wrapper;
- flatten the nested voice Option into a three-case variant;
- retain the old CharacterDialogue nominal root/custom/inline readers;
- bump any Arcweft-owned version above `1`;
- include production code, a patch, or an implementation overlay in this ZIP.
