# Consumer inventory

## Current producer evidence

- `crates/arcweft-lang-syntax/src/parser/function_grammar.rs` parses repeated
  `()` groups and then `->`; it has no trailing content-parameter production.
- Trait/impl and extern-capability function grammars likewise have no content
  slot.
- Generic item attributes are retained, but they do not own a callable local
  or ABI destination. Current `#[fx]` recognition is intentionally not copied:
  it is based on lax attribute classification and is not the final validator
  pattern for this contract.
- `crates/arcweft-lang-hir/src/item/callable.rs` has ordinary parameter groups,
  defaults, locals, and return types, but no attached parameter.
- `CallableSymbol` exposes `is_fx()` and no attached-content contract.
- `crates/arcweft-lang-sema/src/callable/builder.rs::project_record` always
  calls `CallableSignatureSchema::try_new`, publishing `attached_content=None`.
- Presentation and text-proxy schema factories are the only current production
  attached-content parameter producers, and both are Structural.

## Selected-call consumers

- `callable/resolver.rs::PreparedCallInputs`
- `final_analysis/analyzer/calls.rs::prepare_candidate`
- `final_analysis/analyzer/calls/constraints.rs` validation/materialization
- `final_analysis/analyzer/call_seal.rs::checked_execution_projection`
- `callable/checked_application.rs::CheckedCallExecutionProjection`
- `CheckedCallApplicationCore` v1 canonical encoder
- runtime operand users in compiler/runtime-plan producer admission

All must consume the dedicated attached field. None may scan HIR applications
after C1 or reclassify it as an ordinary argument.

Selected-call preparation also owns the exhaustive channel split:
`DialogueLine` validates the exact selected family/HIR owner and publishes no
attached operand, while its semantic content operand remains under the
dialogue application seal. The generic attached mapper must not classify an
exact line as malformed merely because no attached-body source exists.

## Checked content consumers

- `checked_rich_text/model.rs::CheckedRichTextReport`
- `final_analysis/analyzer/evaluated_effects.rs` C2 recursive seal
- `final_analysis/semantic_transcript.rs`
- `final_analysis/nominal_schema.rs`
- compiler rich-text/content lowering
- LSP diagnostic and hover projections

The report admission is the single final role owner. Insertion tokens may
derive it but must not store a second role.

## Runtime authority gaps exposed by this cut

- The current cut now publishes report-local runtime fragment facts and carries
  site-keyed zero-argument callback values through core/AWBC content
  construction. That carrier is necessary but not executable authority.
- `RuntimeFunctionSite` has expression/executable body families. The public
  switch must delete the remaining project `RuntimePureHelper`/`PureCall`
  reservation and raw effect-clause selector, and route every project
  application through the suspension-capable `ProjectCall` Flow/AWBC state
  machine. A selected project call is `FlowRequired` even when pure and
  non-suspending.
- AWBC ordinary functions can contain `EmitEffect`, but synthetic function
  signatures often use effect-set zero. A reveal callback must propagate its
  exact checked effect set into the AWBC signature.
- Dialogue reveal currently validates effect site IDs but does not resolve and
  activate the callback retained by the materialized content value. Native and
  AWBC reveal scheduler entrypoints are both missing.
- Attached default rows expose a separate semantic cycle: ordinary expression
  transcripts read final callable interface digests, while the accepted
  default expression was specified as a member of that same interface. A
  distinct acyclic default transcript and one final catalog seal are required.
- `RuntimeResolvedCall` currently discards the checked completed group and the
  positioned attached ABI coordinate. Both must survive runtime fact
  publication so curried/zero-ordinary-parameter terminal groups can join the
  declaration descriptor without scanning HIR.

## Final runtime consumers

- Compiler final-sema projection issues `RuntimeContentFragmentFact`.
- `CheckedDialogueEffectSite` is the sole checked producer of the closed
  effect set and capture schema; compiler copies both into the one runtime
  program fact and never derives either from the operation.
- The fragment fact's generation-local `source: ExprId` is only the validated
  one-to-one FinalExpr lookup key. Stable fragment coordinate/ID remains the
  identity and no digest or persistence row includes `source`.
- Runtime-plan `FinalExpr` builds the content envelope and complete call ABI.
- Runtime-plan `FinalFlow` consumes fragment facts for root/nested content and
  no longer traverses split raw children.
- Core plan construction validates the complete manifest and binding rows.
- Core function sites own expression or effectful executable bodies under one
  typed effect-set authority. The canonical `EffectId` type is foundational in
  `arcweft-id`; core's `RuntimeFunctionEffectSet` is the only runtime row and
  AWBC tables are encoding destinations. Reveal enters the executable
  scheduler path.
- Runtime-plan retains one typed continuation lineage across nonterminal
  groups and allocates no site for `Continue`. It reserves one ordinary
  function site per reached closed terminal `Invoke` instance. The terminal
  ABI consumes exact prefix/current coordinates, and the body/type projection
  consumes the exact HIR executable-owner partition rather than descendant
  scopes. Final sema seals one independent executable-control role for
  functions, closures, and attached defaults. Runtime facts validate
  `ExpressionFunctionSite` only for ExpressionCompatible + NonSuspending + an
  empty closed row; every other admitted DirectFrame uses the ordinary flow
  `ProjectCall` path. No item-role selector or source spelling participates.
- AWBC lowering emits v1 `MakeDialogueContent` plus the ordinary call.
- AWBC codec/verifier/VM retain and execute effect callbacks/captures;
  executable callback signatures retain the exact effect set, reveal shares
  the `ApplyFunction` activation authority, and session-save uses the existing
  function snapshot authority.
- Runtime driver/dialogue playback consumes the final content value only; it
  does not resolve HIR or sema identities.
- Rich-text effect events no longer arm line-task action nodes. The validated
  `(DialogueActivationId, rebased RuntimeDialogueEffectSiteId)` key reserves
  one callback activation; only marks remain reducer-ready. The old direct
  line-task effect producer is deleted in the same switch.

## Intentionally separate later work

Generic Match architecture is not changed by this contract. It may consume
the resulting typed expressions/content values, but it does not own attached
declaration syntax, role admission, fragment identity, or ABI position.

The generic Match work does not own the ordinary executable-function
predecessor above. That predecessor is required by this accepted contract and
cannot be deferred while effectful defaults are admitted semantically.
