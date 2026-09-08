# Implementation order

This is one public-switch cut. Intermediate local compilation points are
useful, but no commit may freeze a schema-less generic-body reject, a second
fragment inventory, or a runtime operand that has no callee destination.

## 1. Declaration owner

1. Add the trailing attached-content grammar to ordinary function,
   trait/impl method, and extern-capability parsers.
2. Add closed syntax/attachment role and presence carriers. Do not route
   through generic `HirAttribute` recognition; current `#[fx]` last-segment
   recognition is not a validator precedent.
3. Add source roles and formatter projections, then lower to one HIR attached
   parameter with a binding local/default expression.
4. Extend HIR callable validation/digests/symbol publication and trait-interface
   matching. Reject unsupported owners and recovery before catalog publication.

Mechanical work suitable for a Luna agent after the exact shapes land:
syntax-kind/accessor/source-role exhaustiveness, parser fixture expansion,
formatter projection, and constructor call-site migration.

## 2. Callable schema and checked interface

1. Evolve `CallableAttachedContentParameter` with terminal group and keep
   `CallableAttachedContentExecution::{Structural, RuntimeContent}`.
2. Change `RegisteredCallableCatalogBuilder::project_record` to use the
   attached-aware schema constructor only from accepted HIR evidence.
3. Add the checked callable attached binding row and dedicated attached
   binding/default semantic coordinates. Keep the default digest distinct from
   ordinary expression semantic digests.
4. Include attached defaults as callable execution/effect roots, close the
   existing SCC-capable effect graph, and freeze the move-only final resolution
   draft. The draft has final checked IDs/schemas/execution/exposed rows and no
   interface digest.
5. Seal default calls against that draft, issue the acyclic default transcript
   with declaration/schema/execution/effect leaves, and consume the entire
   default batch plus draft in one final callable-interface/catalog seal.
   Publish no preliminary interface digest or draft catalog.
6. Run ordinary semantic transcripts only after the final catalog exists.
7. Validate exact standard `DialogueContent` result and trait/impl structural
   attached equality; exclude declaration-local binding coordinates from the
   trait/impl equality projection.

Mechanically delegatable: schema constructor migrations, digest field wiring,
type visitor exhaustiveness, and fixture construction. Keep declaration
admission and trait/default ownership with the architecture owner.

## 3. Selected call and role seal

1. Evolve `PreparedCallAttachedContentOperand` to Omitted/Present and make the
   terminal group explicit.
2. Dispatch the typed selected-call site family first. An exact `DialogueLine`
   validates its HIR owner and returns no attached operand because its semantic
   content belongs to the dialogue application channel. Only an attached-call
   family reads typed HIR body presence. Present plus no schema and required
   omission are mapping rejection; owner/topology mismatch is invariant
   failure.
3. Evolve `CheckedCallAttachedContentOperand` so Structural cannot have, and
   RuntimeContent cannot lack, its final ABI position.
4. Include the attached member in the single ABI-ordered runtime operand
   iterator.
5. C2 joins checked call operand, HIR content identity, affine prepared report,
   and selected policy. `CheckedRichTextReport` owns final admission and its
   v1 transcript tag.
6. Replace late internal `WrongPayloadFamily` role failures with structured
   authored diagnostics at exact source roles.

Do not add a ContentResult candidate ID branch or callable-name exception.
Do not route `DialogueLine` through attached schema/body pairing or convert its
semantic content operand into attached metadata.

## 4. Runtime fragment semantic facts

1. Add `RuntimeContentFragmentFact` under
   `crates/arcweft-runtime-plan/src/semantic_facts/` as the sole runtime-plan
   projection of one checked report.
   Retain its generation-local `source: ExprId` only as a one-to-one lookup
   key; exclude it from fragment ID/digest/cache/persistence.
2. Project root and nested fragments from final sema in
   `crates/arcweft-compiler/src/lower.rs` (split into the existing lower
   modules as appropriate). Each fact owns template, value programs, effect
   programs, and capture bindings.
3. Evolve `RuntimePlanSemanticFactInput/Facts` to validate the complete
   fragment catalog atomically. Do not retain separate
   `dialogue_content_templates`, root application values, or effect-site
   inventories as peer authorities.
4. Seal `CheckedDialogueEffectSite.effects` from the selected application's
   closed checked row alongside the exact free-local capture list. Commit both
   in the sema transcript, copy both into the runtime program fact, and
   validate them before fact publication. Never infer the set from the
   operation.
5. Make every ContentResult insertion reference its independent fragment ID.

Mechanically delegatable: fact accessors, batch-validation fixture migration,
and repetitive constructor changes. Keep fragment identity, binding order,
and compiler projection with the architecture owner.

## 5. Core plan/value construction

1. Evolve `crates/arcweft-core/src/plan/dialogue_content.rs` so the v1 content
   manifest owns value slots and the lower core trigger/callback capture ABI.
2. Evolve `RuntimeDialogueContentValue` in
   `crates/arcweft-core/src/value/opaque.rs` to own evaluated value bindings and
   site-keyed zero-argument `RuntimeFunctionValue` callbacks.
3. Evolve `RuntimeFunctionSite`/`RuntimeFunctionSiteSeedId` in place with
   `RuntimeFunctionSiteBody::{Expression, Executable}`. The executable body
   owns the exact typed effect set and existing `FlowOp` algebra; it is not a
   content-specific function table.
   First move the existing canonical `EffectId` implementation unchanged to
   `arcweft-id`; use it in core's sorted `RuntimeFunctionEffectSet`. Sema
   re-exports it, and runtime-plan performs the one checked-set projection.
4. Lower each checked content effect program once to a zero-ordinary-argument,
   Unit-result executable site whose captures and effect set exactly match the
   manifest. Keep synchronous structured apply expression-only.
5. Evolve `RuntimeExprSeedKind::DialogueContent`, final runtime expression
   kind, construction seed/lower/builder validation, and their digests.
6. Evolve text-model materialization to rebase effect sites and callback
   bindings together.
7. Preflight all slots/effect sites/captures/function body families/effect sets
   before interning or plan mutation.

Mechanically delegatable: closed-field plumbing and deterministic encoder
updates after the schema is fixed. Keep capture timing and atomic validation
with the architecture owner.

## 6. FinalExpr, callable ABI, and defaults

1. Add `RuntimeResolvedAttachedContent` to `RuntimeResolvedCall` in
   `arcweft-runtime-plan/src/semantic_facts.rs` (or its split owner).
2. Evolve the checked/compiler/runtime call fact to retain one source-ordered
   physical operand row with a typed ABI destination per member. Validate the
   ABI permutation without sorting or dropping the row. General consumers
   evaluate once in source order and install by destination; specialized
   structural consumers first bind the row to shared typed ANF locals and only
   then assemble target payloads in ABI/role order.
   Construct the fragment envelope and keep Optional/Defaulted call-interface
   values at the checked final Option ABI position.
   `RuntimeResolvedAttachedContent` retains the exact normalized ABI type for
   every presence, so omitted `None` does not reopen the callee schema.
3. Evolve checked callable/function lowering to append the matching ABI
   descriptor. Optional remains an Option binding. For Defaulted, ProjectCall
   materialization projects present Content or invokes the declaration-owned
   default FunctionSite for omission, then supplies only the final Content
   binding to the terminal target; default evaluation is conditional,
   once-only, and may suspend.
   Project the interface once into
   `RuntimeProjectCallable.attached_content_abi`; make
   `RuntimeEntryCallableInput` own that `RuntimeProjectCallable` instead of
   duplicate declaration/owner fields. Defaulted function-site reservation
   consumes only the descriptor's checked default source/coordinate/digest and
   exact logical prefix/current capture-source row; it does not add a hidden
   raw Option parameter to the target site.
4. Ensure partial applications do not consume or rank the terminal body.
5. Replace project-call `RuntimePureHelper` reservation with call-fact-owned
   unit `Direct | Continuation { callee, abi }` input and
   `Continue { abi, next_group } | Invoke { instance }` outcome. Nonterminal
   `Continue` constructs a typed continuation from once-evaluated flattened
   prefix values and allocates no function site. Compiler publishes one exact
   closed terminal instance fact per
   `(RuntimeCallableId, CallableInstantiationDigest, CallableGroupIndex)`.
   Reserve every reached `Invoke` instance before defining any body.
   The instance ABI maps all earlier groups to continuation-prefix positions
   and the terminal group to logical current positions; current-group
   materialization indexes the source row and deterministically packs rest
   operands into one Vec binding. Its substituted owner rows
   come from the exact HIR executable partition and exclude nested executable
   roots. Never use a site-specific application digest, descendant-scope scan,
   callee-value inference, registry lookup, or first observed call as the
   monomorphization/type source.
   Project the live/snapshot continuation ABI as RuntimeSemanticTypeId rows.
   Native resolves through the plan type table and AWBC copies semantic
   identity from its program-local type rows; delete every ordinal conversion
   between RuntimePlanTypeId and AwbcTypeId.
6. Retain `CheckedCallApplicationCore::current_group()` on
   `RuntimeResolvedCall` and retain the attached ABI position on its final
   attached operand. Validate both against the descriptor for direct calls
   and against the remaining function type plus lineage for value
   continuations. Only the terminal `Invoke` outcome carries a site key.
7. Seal one selected-expression executable-control role for ordinary
   functions, closures, and attached defaults. Select Expression only for
   ExpressionCompatible + NonSuspending + a closed empty effect row; any
   selected project call is FlowRequired and therefore Executable. Delete the
   item-role parallel selector and the raw HIR effect-clause
   filter in `reserve_called_project_helpers` and delete project-function
   `PureCall` lowering.
8. Add the suspension-capable `RuntimeFlowOpSeed/FlowOp::ProjectCall` state
   machine. Native execution uses a same-fiber call/default/target-frame return
   continuation; AWBC lowers it to the v1 `ProjectCall` terminator with one
   resume register. Expression contexts ANF-lower to a mini CFG. There is no
   synchronous ApplyFunction/PureCall or AOT-linear fallback.

Do not append the attached value manually in compiler code after using an
otherwise incomplete operand iterator.

## 7. AWBC and VM

1. Evolve `AwbcDialogueContentTemplate`, `MakeDialogueContent`, call operand
   lowering, function parameter schemas, codec, structural verifier, and VM in
   place under version `1`.
2. Evolve `PendingAwbcClosure` with the same expression/executable body enum.
   Lower executable sites through the existing flow lowerer so
   `RuntimeFlowOp::EvaluatedEffect` becomes `EmitEffect`; intern the exact
   runtime function effect set into the AWBC signature rather than using the
   empty synthetic set.
3. Encode one callback function plus capture registers for each effect site.
   Native reveal starts a structured executable frame; AWBC reveal creates an
   internal callback fiber and binds captures through the same factored
   activation validator/frame binder used by `ApplyFunction`.
4. Execute AWBC reveal callbacks only through `step_with_host_context`, so
   `EmitEffect` publishes the ordinary `VmObservation::Effect`. Construction
   never runs the callback.
5. Evolve AWBC session-save projection for callbacks through the existing
   function snapshot authority.
6. Add mutation tests for every new field and cross-reference before switching
   compiler production.
7. Replace direct line-task execution atomically: delete
   `final_flow/line_plan.rs::lower_dialogue_effects`, both its ContentEffect
   child and delayed schedule producers, every core/AWBC ContentEffect
   line-task trigger variant, and the reducer effect-ready set. Preserve the
   reveal event only as callback ingress, reserve
   `(DialogueActivationId, rebased RuntimeDialogueEffectSiteId)`, and enqueue
   the stored function. Never ship both paths together.
8. Pass each ordinary executable project callable's final checked effect set
   to both its AWBC function signature and every nested evaluated-effect plan.
   Verify direct calls, curried continuations, returns, suspension/resume, and
   save/restore through the ordinary AWBC frame path.

Mechanical work suitable for Luna: codec read/write pairs, exhaustive match
migrations, golden fixture regeneration, and one-mutation verifier rows.

## 8. Compiler switch and deletion

1. Switch compiler ContentResult lowering to the fragment fact and complete
   runtime attached operand.
2. Delete `_child_values`, dropped `child_effects`, separate effect-site
   arguments/scans, raw attached-call runtime scans, and old template/value
   staging.
3. Delete any schema-less success for a present body and any inferred Rich
   role.
4. Run the complete acceptance matrix and only then create the reviewable
   commit.

## Dependency-safe owner map

| Authority | Owning path/type |
|---|---|
| Source declaration | `arcweft-lang-syntax` function/method/capability grammar and attachment |
| Accepted declaration | `arcweft-lang-hir::item::*Signature` + attached parameter + local/source roles |
| Project schema producer | `arcweft-lang-sema/src/callable/builder.rs::project_record` |
| Schema contract | `arcweft-lang-sema/src/callable/schema.rs::CallableAttachedContentParameter` |
| Candidate input | `PreparedCallInputs::attached_content` |
| Final call input | `CheckedCallExecutionProjection::attached_content` |
| Final role | `CheckedRichTextReport::admission` |
| Runtime fragment fact | `arcweft-runtime-plan/src/semantic_facts/content.rs::RuntimeContentFragmentFact` |
| Core manifest/value/callback ABI | `arcweft-core/src/plan/dialogue_content.rs` and `value/opaque.rs` |
| Runtime call consumer | `RuntimeResolvedCall::attached_content` + `FinalExpressionLowerer` |
| AWBC/VM/save | `AwbcInstruction::MakeDialogueContent`, call ABI, codec/verifier/VM, `value/awbc_save.rs` |
