# Lang-01.5.1.1.2 — final-HIR View execution catalog and static certification reconciliation

## Sequence position and precedence

This request follows the accepted Lang-01.5.1.1.1 as-built dialogue-profile
owner/admission reconciliation and the completed Proof-concurrency v6.1.1
typed syntax/HIR/project public switch. It closes the compiler-exposed gap
between the current final HIR/semantic View facts and the existing executable
View product, runtime, codec, hot-reload, and save boundaries.

The accepted dialogue-profile owner chain, typed `SyntaxNodeId`/final-HIR
identity, ordinary-function/direct-suspension roles, typed RichText authority,
typed resource registry, CharacterDialogue runtime, and current persistent
identity decisions remain authoritative. Do not redesign them without a new
repository-evidenced contradiction.

This contract is required before the stale pre-Proof View-product tests can be
replaced by a complete final-HIR matrix or Lang-01.5.1 can be called closed. It
does not block the already satisfied Lang-01.5.1.1.1 dialogue admission tests.

## Inspected production evidence

The clean pushed baseline is Git commit
`a6805f7375499e5cce70f84f1531832583474527`.

Current production exposes one real cross-layer gap:

- final HIR retains typed View parameters, defaults, exports, ordered value
  `ExprId`s, scopes, item identity, and source roles;
- `FinalSemanticAnalysis` classifies View expressions only as element, Text,
  RichText, or a modifier carrying its member name. It does not retain the
  exact parameter/argument role, nested child contract, modifier payload,
  handler, dynamic dependency plan, resource binding, export target, or
  static-certification evidence needed by later consumers;
- `ViewProjectLowerer` consumes the final-HIR generation directly, but accepts
  only argument-free built-in elements, literal Text/RichText, and two typed
  dialogue projections. It rejects modifiers and other dynamic shapes through
  one generic `MissingCheckedViewProjection` path;
- `ViewProgramResource` already exposes typed instructions for nested View
  calls, branches, keyed repeat, await, local binding, Fx, handlers, semantic
  targets, and value programs, but several observable properties remain
  static-only fields;
- `FxRuntimeType`/`FxRuntimeValue` are intentionally numeric/presentation
  scalar types. They do not own String, arbitrary nominal values, or resource
  identities. Text has a separate `ViewTextSourceKind` projection path;
- `ViewActionButtonResource::enabled`, layout bounds, and several control
  policies have no dynamic program binding, so compiling dynamic values by
  silently taking defaults would change semantics;
- the removed pre-Proof compiler lowerer comprised a second flattened-HIR/AST
  interpretation. Restoring it would violate the selected final typed
  authority and is not an option; and
- `cargo test -p arcweft-compiler --test view_product --jobs 4` currently runs
  seven tests in 0.05 seconds and reports one pass/six failures. The failures
  retain old `Image` builtin, rejection code/stage/cardinality, and static-only
  expectations. `dialogue_profile_admission` independently passes 5/5.

## Fixed semantic direction

The final contract must preserve these decisions:

1. Dynamic View is ordinary valid language behavior, not a rejection caused
   by missing compiler representation.
2. One checked View catalog contains both dynamic and static-certifiable
   definitions/subtrees. A static certificate is an additional optimization
   proof, never the admission authority.
3. Absence of a certificate selects the correct dynamic execution path.
4. Automatic static proof and an authored `#[static]` assertion run the same
   typed analysis. `#[static]` is a checked performance contract: it fails when
   the selected definition/subtree is dynamic and never bypasses validation.
5. Static certification is derived from final HIR and accepted semantic facts,
   not source spelling. It may cover immutable typed resources as well as
   literals when the resource contract proves immutability.
6. The compiler must never re-read source, traverse an old AST, reconstruct
   expressions from spans/strings, or consult a copied endpoint catalog.
7. The current language surface remains in force for this reconciliation.
   Broader lexical identity, `mount`, Action emit/receive, shared View parser,
   Dialogue `#call()[content]`, Ruby, and try/pipe changes are separate cuts.

## Required decisions

1. Define the sole checked View semantic catalog owner and exact Rust-shaped
   public/private APIs. Specify how definitions and subtrees retain final-HIR
   IDs, accepted generation identity, source roles, resolved callable/resource
   identities, types, effects, dependency sets, and execution shape.
2. Define complete typed semantic variants for the current View language:
   element construction and attached children, Text/RichText sources, nested
   View calls, parameters/defaults, locals, modifiers, branches, match, keyed
   repeat, direct await forms, handlers/actions, Fx, parts/exports, input
   controls, layout, scroll, navigation, semantic targets, and resource/image
   references. State which surface forms are currently canonical and which are
   later language-surface work.
3. Select the exact dynamic-value execution owner. Reconcile generic runtime
   values, the presentation-only `FxRuntimeValue` program, Text projections,
   nominal/resource values, and handler inputs without adding parallel value
   models or silently coercing unsupported values.
4. Reconcile every static-only product field that can be authored dynamically,
   including enabled state, labels/text, layout/modifier values, policies,
   nested View arguments, resources, and keys. Specify exact wire additions or
   replacements, codec tags/allocations, validation order, runtime evaluation,
   failure atomicity, and deletion of superseded fields/helpers.
5. Decide image/resource/animation behavior together. Define whether `Image`
   is a View constructor, a typed resource projection, or another existing
   owner; how still images, animated GIF/APNG/WebP or equivalent typed animated
   resources bind; and which identity reaches runtime. Do not generalize every
   resource through a guessed `Presentable` trait.
6. Define static-certification evidence and identity: definition versus
   subtree granularity, dependency closure, effects, immutable resources,
   modifier folding, generated artifact identity, deterministic digest, and
   invalidation across hot reload. Specify the exact typed representation of
   automatic proof and `#[static]` assertion.
7. Define static and dynamic runtime parity. A certified subtree must produce
   the same observable View, input behavior, source diagnostics, Agent
   observation, and save/replay result as the dynamic path. State which work is
   removed by certification and which lifecycle work remains mandatory.
8. Define parameter/default/export and nested-call identity end to end. A
   default or argument may not be dropped because the current scalar evaluator
   cannot represent it. Exported parts must bind to typed instruction/subtree
   identity, not ordinal or source-text inference.
9. Define source-bound diagnostics and failure precedence for semantic
   invalidity, unavailable execution evidence, failed `#[static]` assertion,
   resource mismatch, product validation, runtime binding, and stale
   generation. Do not preserve obsolete `compiler.view.literal_text`-style
   rejection codes when dynamic input is valid.
10. Define AWFB, bundle product, runtime catalog, native/Web/headless/Agent,
    save/replay, hot replacement, and generated-artifact consumer migration.
    State whether certificates are serialized, recomputed, or both, and how a
    tampered or stale certificate fails closed.
11. Provide a deletion-driven compile-clean interleave. It must name the point
    at which the generic current `MissingCheckedViewProjection` fallback, stale
    static-only tests, and any superseded static fields/helpers disappear.
12. Provide exact bounded work accounting and limits for semantic catalog
    construction, dynamic program generation, dependency closure, static proof,
    codec validation, and runtime evaluation.

## Required consumer inventory

The returned package must inspect and cover at least:

- `arcweft-lang-syntax` View/attribute/current attached-body ownership;
- `arcweft-lang-hir` View items, expressions, members, scopes, source roles,
  IDs, generation validation, and project view;
- `arcweft-lang-sema` final analysis, View callable classification, checked
  expression/type/effect/resource facts, and project catalog publication;
- `arcweft-compiler::view`, image/style/Fx catalogs, dialogue profile admission,
  source maps, diagnostics, and `CompiledProject` atomic publication;
- `arcweft-view` program, value program, mount state, parts, resources, and
  identities;
- bundle View program/text/input/style codecs, semantic digest, validation,
  merge, product section, and section compatibility policy;
- runtime-driver View catalog/evaluator/replacement/save paths, runtime-plan,
  runtime-host, native/Web/headless/Agent/MCP observations, and generated
  artifact binding; and
- current compiler/View/runtime tests, Tier 2 View/native/Web/Agent rows, Cargo
  metadata, and structure audit.

## Required implementation order

1. Freeze the checked semantic catalog, dynamic-value owner, exact wire delta,
   static proof, identities, budgets, and diagnostics.
2. Publish complete typed final-analysis facts while the current compiler path
   still fails closed; do not add a source or legacy-HIR fallback.
3. Switch compiler lowering for current valid dynamic/static View shapes and
   parameter/default/export/resource ownership in one product transaction.
4. Migrate bundle/runtime/save/hot-reload/tooling consumers and add dynamic
   versus certified parity/tamper tests.
5. Delete obsolete generic rejection branches, static-only assumptions, stale
   tests, and superseded product fields/helpers.
6. Add the authored `#[static]` assertion only after automatic proof and the
   same typed diagnostic path exist.
7. Run focused, workspace, Tier 2, metadata, and structural gates before
   claiming the boundary complete.

## Tests to specify

- literal, parameter, local, projection, nominal/resource, and computed dynamic
  values through final HIR, checked catalog, product codec, and runtime;
- nested elements/View calls, modifiers, branches, match, keyed repeat, await,
  handlers, parts/exports, defaults, and all input controls;
- still and animated typed image/resource references, missing/mismatched/stale
  resources, and exact source diagnostics;
- automatic whole-definition and subtree static proof, immutable resources,
  dynamic contamination, and exact dependency closure;
- `#[static]` success and dynamic-failure diagnostics, with no unchecked hint;
- certified/dynamic runtime, native/Web/headless/Agent, save/replay, and hot
  replacement parity;
- certificate/wire tampering, digest mismatch, stale generation, missing
  execution program, and no-partial-publication cases;
- exact-limit/one-over work accounting for nodes, instructions, dependencies,
  programs, constants, nesting, exports, and certificates;
- compile-fail/API tests proving old AST/flattened-HIR/source readers and
  endpoint catalogs are unreachable; and
- replacement of the stale compiler `view_product` matrix with current final
  semantic expectations, followed by workspace check, strict Clippy, relevant
  Tier 2, Cargo metadata, and structure audit.

## Constraints and non-goals

- Do not restore the deleted flattened-HIR/AST View lowerer or copy its old
  schema as a second authority.
- Do not reject valid dynamic View merely because the current compiler product
  cannot encode it.
- Do not add source reconstruction, stringly callable/resource lookup, a
  second parser, a second catalog, endpoint DTOs, compatibility aliases, dual
  readers, shims, source gates, or removed-syntax diagnostics.
- Do not revive CSS or Takumi.
- Do not infer the broader `mount`, Action emit/receive, persistent-reference,
  Dialogue content/Ruby, try/pipe, Choice, or Style naming surfaces here.
- Keep lower crates Sans I/O and preserve dependency direction.

## Expected output

Return one independently usable design-only archive named
`arcweft-lang-01.5.1.1.2-final-hir-view-execution-catalog-and-static-certification-reconciliation-final-contract.zip`.
It must contain `OPEN_QUESTIONS=0`, exact Rust-shaped owners/APIs, all wire and
save allocations, semantic/static certificate rules, dynamic/static parity,
complete producer/consumer/deletion matrices, bounded work accounting, an
ordered compile-clean plan, and a full positive/negative/tamper/Tier-2 matrix.
Do not include a production code overlay.
