# Repository evidence

## Snapshot

- Clean pushed production baseline named by the request:
  `a6805f7375499e5cce70f84f1531832583474527`.
- The inspected current `main` was `e619231de8fe0e7c2a9d0d7be15a3608be042058`. Its parent is the
  production baseline above and its diff adds only this request and its blocker
  note. Production claims are therefore pinned to `a6805f7375499e5cce70f84f1531832583474527`; the later
  documentation-only commit is recorded but not treated as a production delta.

## Current final-HIR and semantic authority

`arcweft-lang-hir` already retains `HirSnapshotId`, `ItemId`, `ExprId`, `LocalId`,
scopes, ordered View values, parameter defaults, exports, source-query roles, and
project-symbol generation (`ProjectSymbolWorldId` plus
`ProjectSymbolRevision`). `FinalSemanticAnalysis` is explicitly the
generation-bound semantic authority and already publishes typed expression,
callable, type, effect, suspension, nominal, RichText, and resource facts.

The current View classification is incomplete rather than wrong: it retains only
`CheckedViewCall::{Element, Text, RichText, Modifier}`. It lacks a complete
execution node, dependency closure, parameter/default role, nested-call binding,
handler input, exact modifier payload, resource projection, export target, and
static result. The selected fix extends `FinalSemanticAnalysis`; it does not add a
compiler-side semantic catalog.

## Current compiler gap

`arcweft-compiler::view::ViewProjectLowerer` validates final-HIR/semantic
generation but only lowers:

- argument-free built-in elements;
- one-argument literal Text/RichText; and
- two typed Dialogue text projections.

Every other valid shape—including modifier calls and dynamic values—falls through
`ViewProjectLowerError::MissingCheckedViewProjection`. The same path synthesizes
text IDs from ordinals and hard-codes layout bounds. These are scheduled for
physical deletion at compiler cut C4, not preservation or repair.

## Current executable substrate

`arcweft-view::ViewInstruction` already owns `OpenElement`, `CloseElement`,
`EmitText`, `EmitImage`, `EmitCustom`, `CallView`, `Branch`, `RepeatKeyed`,
`Await`, `BindLocal`, `ApplyFx`, `BindEvent`, and `AttachSemantic`. The final
contract extends this owning enum with `Match` and replaces static authored values
with typed constant/program bindings. `Image` is not a `ViewElementKind`; the
current element inventory is Panel, Box, Scroll, Row, Column, Stack, Button,
TextField, TextArea, and SecureField.

The current `ViewValueProgramInventory` evaluates a presentation-scalar
`FxRuntimeValue` domain despite its generic name. It cannot represent String,
RichText, arbitrary nominal values, resource identity, sequences, or handler
inputs. The runtime already stores root and mount parameters as
`RuntimeBinding`/`RuntimeValue`, proving that the correct general owner exists.

## Current general value and resource facts

`arcweft-core::value::RuntimeValue` is the single ordinary runtime value domain and
already includes scalars, String, Char, duration, range, iterator, tuples,
sequences, records, nominal records, functions, variants, and entity references.
AWBC functions are already a `RuntimeFunctionBody` and the product VM is the
ordinary execution owner.

`arcweft-resource-model::ResourceRefValue` is the accepted exact configured
resource identity triple:

```text
(entity: EntityId, public: PublicId, resource_type: ResourceTypeId)
```

`ResourceValueType::ResourceRef { type_id }` remains disjoint from asset and
retained-identity references. Because `arcweft-resource-model` depends on
`arcweft-core`, it may own the contextual conversion to/from the existing generic
nominal runtime representation without reversing dependencies or introducing a
second runtime value enum.

## Current product, runtime, and save facts

- Existing ViewProgram AWFB allocation: section/product kind `9`, magic
  `AWVP\r\n\x1a\n`, common schema `1`, required field `1` containing the
  canonical compact JSON transcript.
- Existing ViewText allocation: product codec tag `11`, magic
  `AWVT\r\n\x1a\n`, its own canonical transcript.
- Existing AWBC allocation: ABI `1`, codec tag `10`; no View-specific change is
  required.
- Existing session save: schema ID `arcweft.bundle_session`, version `2`.
  `BundleSessionSnapshot` already carries exact artifact/generation identity,
  `BundleViewRuntimeSnapshot`, generic `RuntimeBinding`s, mount graph, logical
  clocks, allocator cursor, and View state. A save schema bump or certificate
  payload would duplicate existing authority.
- Runtime output already has shared typed RichText/display-frame paths, Fx output,
  native/headless/Web-facing renderer-neutral frames, Agent redaction, exported
  part evidence, and atomic program replacement owners.

## Image formats

The current image owner supports PNG, JPEG, GIF, and WebP. GIF and WebP may be
animated according to the typed decoded resource. APNG is not a current accepted
format and is not introduced by this cut.

## Test evidence named by the request

At the baseline, the request records `arcweft-compiler --test view_product` as
1 pass/6 failures from seven stale pre-Proof cases and independently records
`dialogue_profile_admission` as 5/5 passing. These observations were not rerun by
this design package. The stale matrix is replaced at C4; dialogue admission remains
an independent regression gate.
