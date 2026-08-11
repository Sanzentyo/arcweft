# Diagnostics and failure precedence

## Stable diagnostic codes

| Stage | Code | Meaning |
|---|---|---|
| sema | `sema.view.catalog.invalid_expression` | accepted View context contains a semantically invalid expression shape |
| sema | `sema.view.catalog.type_mismatch` | exact callable/member/value contract mismatch |
| sema | `sema.view.catalog.effect_invalid` | effect or suspension role invalid for render/handler context |
| sema | `sema.view.resource.missing` | typed resource declaration unavailable |
| sema | `sema.view.resource.type_mismatch` | wrong exact `ResourceTypeId` |
| sema | `sema.view.resource.stale_generation` | registry/generation mismatch during analysis |
| sema | `sema.view.static.required_dynamic` | authored static requirement failed typed proof |
| compiler | `compiler.view.catalog.generation_mismatch` | final HIR, symbols, resources, and catalog do not match |
| compiler | `compiler.view.catalog.incomplete` | a semantically accepted owner lacks a required catalog row |
| compiler | `compiler.view.execution.unavailable` | ordinary AWBC lowering failed to produce required execution evidence |
| compiler | `compiler.view.resource.binding_mismatch` | resolved resource/product binding disagrees |
| bundle | `bundle.view.program.invalid` | strict transcript/program structure invalid |
| bundle | `bundle.view.program.binding_type_mismatch` | projection/member/result type mismatch |
| bundle | `bundle.view.certificate.digest_mismatch` | fragment/evidence/certificate digest invalid |
| bundle | `bundle.view.certificate.stale_generation` | certificate generation/dependency stale |
| runtime | `runtime.view.binding.type_mismatch` | evaluated value cannot satisfy exact projection |
| runtime | `runtime.view.evaluation.budget_exceeded` | frame/program/repeat/output budget exceeded |
| runtime | `runtime.view.resource.unavailable` | accepted typed resource not available in active artifact/host |
| runtime | `runtime.view.generation.stale` | runtime catalog/program generation stale |
| runtime | `runtime.view.replacement.stale_candidate` | replacement candidate lost generation race |
| save | `save.view.program_mismatch` | saved program/artifact does not match active catalog |
| save | `save.view.value_type_mismatch` | saved RuntimeValue/nominal layout does not match slot |

Existing lower-level structured codec/AWBC/resource errors remain related causes;
these codes identify the View boundary and do not stringify them.

## Precedence

1. HIR snapshot/project symbol/resource catalog generation.
2. Ordinary semantic expression, type, effect, suspension, callable/member, and
   resource validity.
3. Catalog completeness and source roles.
4. `#[static]` requirement after a valid dynamic/static result exists.
5. Compiler ordinary-function/AWBC and resource/product binding.
6. AWFB envelope, transcript, cross-section, and certificate validation.
7. Runtime input binding, program execution, projection, resource availability,
   and budget.
8. Replacement candidate generation race/staleness.
9. Save artifact/program/value restoration.

Only the earliest stage is primary. Later hypothetical failures are not emitted.

## Source binding

Primary source is the exact final-HIR role for the failed authored construct.
Related evidence is typed and ordered:

- callee/member declaration;
- parameter/default or argument;
- expected resource declaration/type;
- first dynamic contaminant for `#[static]`;
- generated AWBC function/product source mapping;
- certificate subject/dependency;
- active versus candidate generation.

Runtime/product failures with no live source span still carry revision-scoped program node,
instruction, program, resource, and optional `SourceRangeRef`. Source text is never
used to re-resolve behavior.

## Deleted diagnostics

`MissingCheckedViewProjection` is an internal transitional error and disappears at
C4. Any `compiler.view.literal_text*`, static-only dynamic rejection, old Image
builtin, ordinal cardinality, or removed flattened-HIR stage code disappears with
the stale tests. Valid dynamic View must not be diagnosed as missing compiler
representation.
