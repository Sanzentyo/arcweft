# Static certification algorithm and evidence

## Granularity

Every View definition and every executable subtree `ExprId` receives one
`CheckedViewStaticDisposition`. Automatic proof always runs. An authored
`#[static]` attribute adds `CheckedViewStaticRequirement` to a definition or an
attached View expression/subtree that already has a typed HIR source role.

The HIR subject is session-only. Product evidence converts it to
`ViewProgramIdentity + ViewStaticSubjectResource`; a subtree uses a
`ViewProgramNodeId` valid only in the containing `AcceptedViewProgramRevision`.
No syntax/HIR identity is serialized or hashed.

## Analysis

The analyzer constructs the render-dependency graph, computes SCCs, and processes
its condensation graph in deterministic reverse topological order. Each node is
visited once and each edge/dependency is charged once.

A node is `Certified` when all conditions hold:

1. execution shape is finite and closed;
2. every render-time value is a literal/accepted constant, a previously certified
   value, or an immutable resource;
3. render effects are empty;
4. nested arguments/defaults and the callee subject certify; the dependency
   closure records the exact accepted callee program revision and certificate
   digest for each acyclic certified edge;
5. branch/match selector is constant and only the selected arm contributes;
6. keyed repeat source and every key are constant, unique, and within the static
   expansion limit;
7. every modifier/property has an exact owning fold implementation and static
   arguments;
8. resource selection and descriptor are immutable;
9. all required resolved identities, product coordinates, and source roles exist.

A recursive View-call SCC, await, host/environment read, mutable resource, dynamic
control, dynamic key, dynamic argument/default/local, effectful render value, or
unfoldable modifier produces `Dynamic` and contaminates ancestors through render
edges. Therefore no recursive edge is inserted into a static certificate digest;
acyclic static calls are revision-bound only in the certificate dependency closure,
not in either caller's program semantic digest.

Handler body effects and input-control mutable state are retained lifecycle, not
render evaluation. A static fragment still registers handlers, input slots, focus,
semantic targets, parts, and observation metadata. A handler's exact identity,
input/output types, effects, AWBC function, dependency digest, and source are in the
validated product even though its body is not run during render.

## Folding

Modifier/property folding is available only as inherent behavior on the owning
Arcweft member/property type or its accepted catalog context. No string switch or
extension trait may claim foldability. Folding uses checked native values and the
same validation as dynamic projection. Static branch/match/repeat expansion
consumes the ordinary instruction budget; there is no unbounded macro-like
expansion.

## Evidence and deterministic identity

Semantic evidence contains subject, program-local node coordinate, semantic digest,
exact dependency closure, immutable resource closure, folded modifiers, retained
lifecycle, and source roles. Source roles remain session/diagnostic evidence and do
not enter persisted digest input. Compiler emits one fragment using the same
instruction and native field types as dynamic execution plus one certificate.

Certificate digest is BLAKE3 over length-delimited canonical fields:

```text
domain = "arcweft.view.static-certificate.v1\0"
ViewProgramIdentity
subject kind and program-local coordinate
ViewProgramSemanticDigest
semantic digest
dependency-closure digest
immutable-resource closure digest
folded-modifier canonical bytes
retained-lifecycle digest
fragment ID and fragment digest
proof origin
```

`SyntaxNodeId`, HIR IDs/snapshots/revisions, source text/ranges/roles,
`ProductSourceId`, and dense arena ordinals are excluded. `proof_origin`
distinguishes an automatic certificate from a satisfied authored performance
requirement without changing fragment semantics.

The containing transcript derives `AcceptedViewProgramRevision` after certificate
and fragment digests are known. This avoids a circular certificate/revision seed
while binding all evidence to one exact accepted program revision.

## Serialization and validation

Compiler serializes evidence and fragment. Bundle decoder:

1. verifies canonical encoding, `ViewProgramIdentity`, and local coordinate bounds;
2. recomputes fragment digest;
3. recomputes dependency/resource/lifecycle digests from serialized typed rows;
4. verifies the whole-program semantic digest;
5. recomputes certificate digest;
6. recomputes `AcceptedViewProgramRevision` from program, fragment, and certificate
   digests;
7. ensures every fragment binding is constant and every retained lifecycle row has
   a dynamic runtime owner;
8. publishes only after full product validation.

Runtime repeats program/revision/digest joins against the accepted bundle catalog
and selects the fragment only when all checks pass. It does not rerun source
analysis.

Three cases are distinct:

- no certificate record for a subject: valid ordinary dynamic execution;
- program references a certificate/fragment that is absent: hard invalid product;
- present certificate/fragment is malformed, tampered, or stale: hard invalid
  product/replacement, with no dynamic fallback.

## `#[static]` diagnostic

Semantic validity precedes the performance assertion. If the subject is valid but
dynamic, the diagnostic is:

```text
code: sema.view.static.required_dynamic
primary: #[static] attribute source role
related: first CheckedViewDynamicReason source
notes: ordered contaminating dependency path and session subject identity
```

No unchecked hint or warning mode exists. The attribute is introduced only after
automatic proof and this exact diagnostic path are implemented and tested.
