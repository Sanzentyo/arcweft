# Normative final contract

The terms MUST, MUST NOT, REQUIRED, and MAY are normative.

## C1. One semantic catalog

`arcweft-lang-sema::final_analysis::view::CheckedViewCatalog` is the sole checked
View semantic catalog. `FinalSemanticAnalysis` owns one immutable `Arc` to it and
publishes it only in the same successful generation transaction as all other final
semantic facts. Every accepted View definition and every reachable View/value
`ExprId` MUST have exactly one catalog row. A missing row after semantic success is
an internal catalog-completeness error, not a language rejection.

The catalog retains snapshot-local HIR IDs for compiler lookup and a typed session
generation containing `HirSnapshotId`, `ProjectSymbolWorldId`,
`ProjectSymbolRevision`, and `ResourceTypeRegistryDigest`. That generation and all
`SyntaxNodeId`/HIR IDs are non-Serde, never serialized, and never enter a product,
certificate, save, cache, or generated-artifact identity. Product identity reuses
the accepted `ViewId`, `ViewProgramId`, and `AcceptedViewProgramRevision` owners;
program-local typed coordinates are meaningful only under that exact accepted
program revision. Source roles and `ProductSourceId` are diagnostic provenance,
not semantic identity or digest input.

## C2. Complete current-language execution algebra

The checked algebra covers element construction and attached children,
Text/RichText, nested View calls, parameters and defaults, locals, modifiers,
branch, match, keyed repeat, direct await, handlers/actions, Fx, parts/exports,
input controls, layout, scroll, navigation, semantic targets, and typed
resource/image references. Every callable/member/resource is the exact resolved
identity from the accepted project catalogs. No product or runtime consumer may
resolve a source spelling again.

The current canonical source surface remains authoritative. This contract does not
add `mount`, Action emit/receive, a shared View parser, Dialogue
`#call()[content]`, Ruby, try/pipe, Choice, CSS, Takumi, or new Style naming.

## C3. One dynamic value owner

All non-Fx View expressions—including defaults, nested arguments, locals,
predicates, match selectors, repeat sources/keys, handler inputs, text values, and
resource references—MUST lower through the ordinary function lowerer to an
`AwbcFunctionKind::Synthetic` function and execute as `RuntimeValue` in the
existing product AWBC VM.

No `ViewRuntimeValue`, View expression interpreter, source reader, copied AST, or
parallel bytecode is permitted. `FxRuntimeValue` remains presentation-only and is
created only by `ViewValueProjection::Fx` after exact result-type validation.
Text and RichText use explicit projections; arbitrary debug/display stringification
is forbidden.

`ResourceRef<T>` uses the existing generic nominal runtime carrier. The owning
`ResourceRefValue` gains contextual inherent encode/decode methods against the
ordinary nominal type layout. Decode validates the nominal type identity, layout,
field types, and exact expected `ResourceTypeId` before constructing the accepted
identity triple. Runtime resource selection joins that triple to the validated
resource catalog; it never parses a source name.

## C4. Dynamic-capable product fields

Every authored field that can be dynamic MUST be represented as either an exact
native constant or a `ViewProjectedProgramRef`. This includes enabled state,
labels/text, input values and policies, bounds/layout/modifier arguments, scroll
and navigation policies, nested arguments/defaults, image/resource selection,
repeat keys, and semantic labels. Missing a dynamic program MUST fail; taking a
static default is forbidden.

The existing ViewProgram AWFB envelope is retained and its unreleased transcript is
directly replaced. No new section, alias, V2 wrapper, dual reader, or source gate is
created. AWBC ABI 1 and session save v2 remain unchanged.

## C5. Image/resource/animation

`Image` is a typed resource projection consumed by `EmitImage`; it is not added to
`ViewElementKind`. A still or animated image is selected by exact
`ResourceRef<Image>` identity. PNG and JPEG are still; GIF and WebP follow their
typed decoded animation descriptor. APNG remains unsupported by this cut. Runtime
receives the exact configured-resource triple, registry digest, declaration digest,
and accepted bundle generation.

Static proof may eliminate resource-selection evaluation when the resource
registry proves the reference and descriptor immutable. Resource acquisition,
release, animation logical clock, frame selection, visibility, and save/replay
state remain mandatory. No guessed `Presentable` trait is added.

## C6. Static certification

Automatic proof and authored `#[static]` use exactly the same analysis and result
type. Proof is computed for every definition and subtree. `#[static]` is an
additional requirement attached to that result; after ordinary semantic validity,
a dynamic result emits `sema.view.static.required_dynamic` at the attribute and
first contaminating source. It never admits an otherwise invalid program.

A subject is certifiable only when its complete render structure and every authored
render-time value are closed, pure, and reproducible from accepted immutable facts.
Literal and typed constant values, immutable typed resources, and modifiers with an
owning exact fold implementation may certify. Await, unresolved host input,
dynamic branch/match/repeat control, dynamic arguments/defaults, mutable resource
selection, or an unproved modifier contaminates its ancestors. Handler bodies do
not execute during render and therefore do not contaminate a static render
fragment, but exact handler identity, input schema, registration, effects, source,
and lifecycle are included in evidence and remain active.

Compiler serializes both the certificate and a fragment built from the same
instruction schema. Decoder/runtime recompute canonical digests and validate
program/dependency/resource bindings; they do not rerun source sema. A subject with
no certificate record is ordinary valid dynamic execution. A present certificate
that is malformed, stale, tampered, or references missing fragment/evidence fails
closed before publication; it never falls back to dynamic execution.

## C7. Static/dynamic parity

A certified subject and its dynamic execution MUST produce the same canonical View
tree, text/RichText values, input behavior, part/export identities, semantic
targets, Agent/MCP observation, source diagnostics, logical resource/animation
state, and save/replay result. Certification may skip AWBC invocations, dynamic
instruction traversal inside the certified subject, repeated constant projection,
and immutable resource selection. It MUST NOT skip mount allocation, state/focus
slots, handler/input registration, dynamic descendants reached outside the
subject, resource lifetime/animation, observation, source mapping, save/replay, or
hot-replacement reconciliation.

## C8. Parameters, defaults, nested calls, and exports

Each parameter is authoritative by a `ViewParameterRef` allocated once from the
canonical typed parameter inventory of its owning `ViewProgramId`. It is a
nonzero program-local coordinate, not a hash of `SyntaxNodeId`, `LocalId`, spelling,
or source range. Ordinal/name are retained only for canonical validation and
diagnostics. A persisted nested-call edge carries stable `ViewId`/`ViewProgramId`
plus the callee parameter-table contract digest; candidate-catalog validation joins
that edge to exactly one accepted callee revision. It never embeds the callee
revision in the caller semantic digest, so recursive View call graphs have no hash
cycle. Defaults are ordinary value programs evaluated in callee declaration order
against the callee environment. Explicit arguments are evaluated in the caller
environment and bound to the exact parameter coordinate. No argument/default may be
omitted because an Fx/scalar evaluator cannot represent its type.

Sema retains a non-Serde `CheckedViewExportKey` backed by the exact owning
`ItemId`/target `ExprId` and typed source role. Product exports replace that session
key with `ViewId`, `ViewProgramId`, a `ViewExportContractDigest`, and typed
`ViewProgramNodeId`/`ViewInstructionId`/`ViewEvaluationSiteId`/`ViewPartId`
coordinates. The containing validated program supplies the exact
`AcceptedViewProgramRevision`; the export row never repeats or hashes its own
revision. Ordinal, source text, arena slot, or synthesized `text.<view>.<n>` identity
is never authoritative.

## C9. Diagnostics and precedence

Validation precedence is:

1. HIR/project-symbol/catalog generation;
2. semantic expression/type/effect/resource validity;
3. checked-catalog completeness;
4. authored `#[static]` requirement;
5. compiler execution/program/resource binding;
6. bundle envelope/transcript/cross-section/certificate validation;
7. runtime binding/evaluation/resource availability;
8. hot-replacement staleness;
9. save restoration.

Each error uses the exact final-HIR source role as primary and typed related roles
for parameter, callee/member, resource declaration, or attribute evidence. Generic
`MissingCheckedViewProjection` and obsolete literal-text rejection codes are not
part of the final surface.

## C10. Publication and consumers

Compiler builds ViewProgram, ViewText, Input, Style, image/resource bindings,
source maps, AWBC functions, fragments, and certificates in scratch state;
`CompiledProject` publishes none unless every cross-section reference and digest
validates. Bundle merge and runtime replacement likewise use candidate validation
and one atomic swap.

Native, Web, headless, Agent, MCP, generated artifacts, save/replay, and hot reload
all consume the same validated runtime catalog. Backend DTOs are observations of
that catalog, never an authoring or resolution authority.

## C11. Deletion-driven cutover

The exact compile-clean interleave in `IMPLEMENTATION_PLAN.md` is mandatory.
`MissingCheckedViewProjection`, old literal/dialogue-only lowering, ordinal text
IDs, generic Fx placeholder coercions, static-only authored fields, stale
`view_product` expectations, source/string endpoint reconstruction, and the old
`ViewValueProgramInventory` evaluator disappear at named cuts. No compatibility
aliases remain after each owning replacement compiles.

## C12. Bounded work

Every semantic, compiler, codec, proof, and runtime loop is charged to one limit in
`WORK_ACCOUNTING.md`. Limits are checked before allocation or execution. Static
proof is memoized O(V+E+D); a node, edge, or dependency is charged once. Exact-limit
and one-over tests are normative.
