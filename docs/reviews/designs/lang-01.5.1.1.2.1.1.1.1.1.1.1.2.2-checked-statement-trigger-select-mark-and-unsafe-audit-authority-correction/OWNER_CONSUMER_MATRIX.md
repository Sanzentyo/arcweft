# Owner and consumer matrix

Layer direction is syntax → HIR → sema/registration → compiler → runtime-plan
→ core execution, with verifier/tooling consuming read-only checked facts.
Lower layers never import sema. HIR IDs may index same-generation facts but do
not select semantic meaning or enter persistent identity.

| Area | Final producer/owner | Authorized consumers | Required migration/deletion |
| --- | --- | --- | --- |
| mark selector leaf | `arcweft-lang-syntax` rich-text grammar and Trigger attachment; one `SyntaxDialogueMarkName` | syntax diagnostics, formatter/canonicalizer, HIR final lowering | delete downstream `strip_prefix`, `parse_public_id`, `Display`/String reconstruction; reject attributes/multiple/missing/invalid selector |
| Select propagation syntax | ordinary prefix `Try` expression parser | HIR expression lowering, sema expression checking/transcript | delete trailing-`?` Select parser helper, attachment field, formatter success, fixtures |
| HIR mark identity/catalog | `arcweft-lang-hir::dialogue_application`; `HirDialogueContent.marks` | HIR line-plan trigger resolution, sema rich-text preparation, source-index diagnostics | delete mark names/IDs inferred from `PublicId` or tag text and any detached mark table |
| HIR Trigger | `arcweft-lang-hir::stmt::HirTrigger` | child edges/evaluation plans, sema preparation/statements, source index | delete `HirTriggerPattern`, `Expr` alias, Mark pattern child, recovery success arms |
| HIR Select head | `arcweft-lang-hir::stmt::thread::HirSelectBranchHead` | HIR child/body edges, evaluation topology, sema Select preparation | delete `propagates_error` from parser, grammar projection, attachment, lowering, source-index matching, tests |
| HIR unsafe identity | `HirUnsafeAuditIdentity` built by one shared final-lowering helper | source index for diagnostics, sema unsafe construction | delete raw `HirIdRefValue` as checked authority and all `id_ref_label` recovery paths |
| HIR statement/body inventory | `HirStmtKind`, `HirStatementChildRole`, `HirStatementBodyRole`, `HirBodyChildRole` | sema typed traversal and transcript | update exhaustive matches directly; Error/recovery remains rejection; no wildcard success |
| Choice lifecycle context | `HirProjectEvaluationTopology::enclosing_choice_lifecycle` | private sema scrutinee selector | no raw parent map/source scan or sema-specific duplicate walker |
| standard ingress IDs/input | `arcweft-lang-sema::env::TypeCheckEnv::new` contributes closed `StatementIngressTypePublicationInput` rows | registration transaction only | no adapter extension role, `Named`, `Other`, Any, or default row |
| registered ingress record | `arcweft-lang-sema::registration::RegisteredTypeCheckEnv` | final analyzer via borrowed accessor, environment digest | add one immutable field; input rows are consumed/dropped; no second registry/map |
| ingress semantic atoms | `arcweft-lang-sema::types::TypeKind::StatementIngress` | pattern/local checking, type digest/transcript, match-domain/visitor/normalization consumers | add exhaustive handling to every `TypeKind` match; no runtime-core reverse authority |
| selected call/callable facts | existing final analyzer and `CheckedCallableCatalog` | private Entry-seeded declaration worklist and Include proof | interleave one declaration's contextual seed with its ordinary call completion, then propagate selected edges; do not create a second call resolver |
| Entry root seed | shared private preparation helper using typed stateful Entry members and accepted Flow callable facts | reachability and final Entry checker | factor current Entry target resolution; consume seed; delete duplicate resolution branches |
| Event reachability | private move-only `PreparedExecutableIngressWorklist`, independently rechecked over the completed selected graph | `StatementScrutineeTypeAuthority`, final Entry seal | no final map/catalog; drop declaration/traversal scratch and consume statement proofs after digest comparisons |
| Include target proof | private prepared accepted callable proof | reachability edge and `CheckedIncludeFlowTarget` construction | one move-only proof; no HIR name re-resolution or copied TypeKind |
| contextual role selector | private `StatementScrutineeRole` and borrowed `StatementScrutineeTypeAuthority` | pattern seed context and statement validation only | no Clone, owned TypeKind, public accessor, report field, or transcript tag |
| checked pattern/local type | existing `CheckedPattern` and checked local facts | statement constructor and transcript child digests | remains sole contextual type owner; delete any statement scrutinee type copy |
| checked expression type | existing `CheckedExpression`/selected call/effect facts | Timeout/Expression/Signal/Select Bind validation and transcript child digests | no trigger-expression type row or propagation summary |
| mark coordinate | `arcweft-lang-sema::semantic_coordinate::SemanticCoordinateIndex` | checked rich text, CheckedTrigger Mark, transcript, compiler projection | constructor private; no label/tag/runtime ID in coordinate; add catalog collision checks |
| prepared marker | private move-only rich-text preparation only if sequencing needs it | final-analysis seal | consumes `HirDialogueMarkId`; cannot be public/Clone or coexist with final coordinate |
| checked rich-text marker inventory | source-ordered `CheckedRichTextAction::Marker(CheckedDialogueMark)` | compiler rich-text/content lowering, transcript, diagnostics | delete checked mark slice, checked ordinal, checked handler slice, PublicId marker action |
| checked dialogue line plan | `CheckedDialogueLinePlan.effect_sites` | compiler effect-site lowering | delete marks/handlers fields, accessors, constructor parts, `into_parts` results, recursive handler collection |
| checked Trigger/Select | `arcweft-lang-sema::final_analysis::model` with private constructors | compiler, runtime-plan semantic fact construction, transcript, project-index summaries | unit/non-child payload only; no recovery/type/ordinal copies |
| checked unsafe audit | `CheckedUnsafeAudit` private constructor | verifier, CLI/LSP display/actions, transcript | verifier consumes ID/SAFETY bit and checked reason/body children; delete HIR re-read and label renderer as authority |
| checked statement | `CheckedStatement { effects, payload }` constructed by one exhaustive analyzer match | compiler, verifier, project index, transcript, persistent diagnostics that classify statement families | delete `CheckedStatementRole`, `Ordinary`, `checked_break_role`, old constructors/validators, all direct fixtures |
| evaluated expression statement | existing dirty `CheckedEvaluatedEffect` sealed operation authority | Expression row of checked statement and compiler | integrate unchanged; delete only old effect success enum/path scheduled by its own cut; no source classification |
| control transfer | existing accepted `CheckedControlTransferTarget` and evidence | checked payload, compiler, transcript | promote read-only visibility as needed; consume affine evidence; never repair obsolete `checked_break_role` |
| compiler mark issuer | `arcweft-compiler` content-order runtime ID issuer and temporary coordinate map | compiler Trigger projection only | reject missing/duplicate coordinate; drop map before runtime facts; delete handler copy loop |
| runtime Trigger admission | `arcweft-runtime-plan::semantic_facts::RuntimeTriggerAdmission` constructed by compiler boundary | runtime-plan validation, final-flow line-plan lowering, AWBC lowering inputs | delete runtime HIR trigger recheck/label resolution; no public external constructor |
| runtime dialogue application | existing runtime-plan application minus handler field | compiler/runtime-plan line content | delete `RuntimeDialogueMarkHandler`, `mark_handlers` field/accessor/constructor arg and statement-side lookup |
| runtime mark identity | existing `arcweft-core::RuntimeDialogueMarkId`, AWBC typed IDs, content events, `LineTaskTrigger::Mark` | runtime execution/save/codec under existing contracts | unchanged lower authority; never import stable sema coordinate/string |
| wait mark | checked suspension analyzer/runtime-plan rejection in this cut | diagnostics/tests | do not route to `RuntimeWaitTarget::Mark(String)`; later admission must reuse HIR mark→stable coordinate→runtime ID |
| unsafe verifier policy | `arcweft-verify` policy over checked payload/children | CLI/LSP reporting and actions | keep missing reason/SAFETY obligations; delete HIR/source identity parsing |
| semantic transcript | final analysis purpose-built memoized statement/body/rich-text writers | Match/catalog seal, compiler-local semantic queries, tests | amend predecessor grammar, all 35 rows, atomic catalog; delete lazy Match-only builder at step 8 |
| project index and summaries | typed read-only checked statement/payload accessors | diagnostics/tools | replace sparse role matches; no raw HIR classification or compatibility path |
| formatters/canonicalizers | syntax typed nodes only | authored text output | removed Select suffix is invalid; diagnostic name is display-only and never round-tripped to semantics |
| constructors/fixtures | owning crate's private constructor or public parser/registration entry | tests through legitimate boundary | migrate every direct constructor/fixture of deleted types; compile-fail tests prove old APIs absent |

## Dependency assertions

- `arcweft-lang-hir` does not depend on sema.
- `arcweft-lang-sema` may consume HIR, core shared types, ID types, and registered
  environment inputs; it does not depend on compiler, runtime-plan, verifier,
  CLI/LSP, presentation adapters, or hosts.
- compiler consumes public read-only final sema facts and constructs
  runtime-plan inputs.
- runtime-plan and core do not import sema coordinates or sema types.
- verifier/CLI/LSP are leaves that consume checked projections; they do not
  become owners.
- Sans-I/O crates remain Sans I/O; no path, filesystem, network, clock, or host
  handle enters these schemas.

## Direct deletion search inventory

Implementation is not complete while repository searches find a live success
consumer of any of these anchors:

```text
HirTriggerPattern
propagates_error
CheckedDialogueMarkOrdinal
CheckedDialogueMarkHandler
RuntimeDialogueMarkHandler
mark_handlers
CheckedStatementRole
checked_break_role
id_ref_label
RuntimeWaitTarget::Mark(String)
parse_public_id        # in marker semantic projection
strip_prefix('.')      # in marker semantic projection
```

Occurrences in an explicit compile-fail deletion assertion or historical
request/design evidence do not count as live source consumers.
