# Dependency and sharing boundaries

## Allowed direction

```text
arcweft-lang-hir
        -> arcweft-lang-sema
        -> arcweft-compiler / arcweft-runtime-plan
        -> arcweft-core typed runtime vocabulary
        -> runtime driver / bundle / save consumers
```

Core may own runtime vocabulary consumed upward. Core may not import HIR, sema,
runtime-plan, compiler, or entry registration construction logic.

## Core owner contents

`RuntimeNominalRecordLayout` uses only core-owned types:

- `RuntimeNominalTypeId`;
- `RuntimeSemanticTypeId`;
- `TypeLayoutHash`;
- `RuntimeCheckedType`;
- `RuntimeRecordFieldId`; and
- ordinary owned strings/boxed slices.

The module relationship between `value::nominal_record` and `pattern` is within
one crate and does not create a cross-crate reverse dependency. Recursive Rust
type size remains finite because a checked nominal stores scalar identities,
not an Arc back to the full layout.

## Construction authority

The public checked constructor is required because runtime-plan is a separate
crate. Architecturally, only admitted compiler/runtime-plan generation and
validated test fixtures construct descriptors. Core validates every structural
property it can own; provenance of the canonical `TypeLayoutHash` is enforced
by runtime-plan fact publication and, where applicable, entry-role equality.

No global mutable registry is required. One plan-generation-local interner uses
`(RuntimeNominalTypeId, RuntimeSemanticTypeId, TypeLayoutHash)` as key and
rejects a structurally conflicting descriptor.

## Arc rules

- runtime-plan creates the Arc after descriptor validation;
- nominal expression and nominal pattern clone the Arc;
- runtime values never retain it;
- save values never serialize it as value identity;
- pointer equality is never consulted; and
- decoded plans validate/reintern before execution.

## Prohibited dependencies/models

- core -> HIR/sema/runtime-plan/compiler;
- core layout embedding `RuntimeTypeSchema` or `RuntimeNominalRole`;
- sema object stored in runtime value;
- alias from absent schema name;
- separate nominal field name map or ID vector;
- a second record value enum;
- name-derived visitor paths;
- source-string parsing at runtime; and
- compatibility/fallback readers.
