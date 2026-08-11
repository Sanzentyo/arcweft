# Checked catalog construction and completeness

## Inputs

Catalog construction is a final semantic pass over the accepted
`HirExecutableProjectView`. It receives the same `FinalSemanticAnalysisInput`,
checked callable catalog, nominal/type report, RichText report, effect facts,
resource registry, project symbols, and generation that produce the rest of the
final report. It does not open files or retain a `HirModule` beyond construction.

## Construction order

1. Validate HIR snapshot and project-symbol world/revision.
2. Enumerate View `ItemId`s in canonical `(HirModuleId, ItemId)` order.
3. Allocate definition shells and exact parameter/export identities.
4. Traverse ordered View root `ExprId`s and attached-body ownership once.
5. Classify each node from checked call/select/match/pattern/type/effect facts.
6. Resolve every member/callee/resource to exact accepted identity.
7. Intern one `CheckedViewNode` per reachable `ExprId`; duplicate reachability
   reuses the same row and charges one reverse edge.
8. Build canonical dependency sets and reverse index.
9. Run SCC/topological static analysis and attach a result to every definition and
   node.
10. Verify completeness, source-role availability, generation, and work counters.
11. Publish the `Arc<CheckedViewCatalog>` with the complete final report.

Any error aborts final semantic publication. There is no partially checked View
catalog visible to compiler or tooling.

## Source roles

`CheckedViewSourceRole` is a typed wrapper around the final-HIR source-query
coordinates. Required roles include definition, parameter name/type/default,
root value, callable/callee, argument, attached child, modifier/member,
branch/match/repeat control, await source/arm, handler/event/action, Fx call and
argument, part/export, input property, layout/scroll/navigation property,
semantic target/label, and resource reference. The source map resolves these roles
to spans only when producing diagnostics or product source references.

## Canonical forms

The semantic catalog recognizes only forms already accepted by current grammar and
callable registries. It does not identify behavior from spelling. Built-in elements
come from `ViewElementKind`; modifiers/properties come from resolved member
identity; nested Views come from resolved project callable identity; resources
come from `ResourceRef<T>` facts; Dialogue text comes from the accepted typed
projection coordinate.

`match` retains typed checked patterns in sema, but compiler lowers the already
checked decision to one synthetic selector program returning an arm ordinal. This
prevents a second runtime pattern matcher.

## Dependency closure

Dependencies are unique, sorted by typed discriminant and stable identity, and
hashed after canonical encoding. A node's direct set contains only facts read by
that node. Definition/subtree closure is the transitive union over child,
call/default, local, control, modifier, resource, text, and handler-registration
edges. Handler body dependencies are recorded in lifecycle evidence but excluded
from render-purity contamination because the body is not executed during render.

Cycles are processed as SCCs. Existing semantic-invalid cycles remain errors.
A valid recursive View-call SCC is always dynamic with
`RecursiveViewCall`; it is never statically expanded.

## Completeness invariant

For every accepted View definition:

- every declared parameter has one row and default row when present;
- every ordered root value has one node;
- every attached child and attachment has one exact owner;
- every nested call argument maps to one callee parameter;
- every local read maps to one `ViewLocalRef`;
- every product-emittable value has one `CheckedViewValue`;
- every export maps to one target node/part/site; and
- every definition/node has one static disposition.

Compiler encountering an absent row reports `compiler.view.catalog.incomplete` as
an internal generation defect. It must not reinterpret HIR to recover.
