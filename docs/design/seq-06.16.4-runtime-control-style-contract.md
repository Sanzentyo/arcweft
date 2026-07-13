# seq06.16.4 native computed Style and runtime-control contract

## Decision

Every retained View consumer uses the single native computed-Style resolver
owned by `arcweft-view`. `arcweft-bundle` owns deterministic resource codecs and
runtime projection only; it does not implement a second cascade.

The canonical path is:

```text
Arcweft native Style
  -> CheckedViewStyleCatalog
  -> sheet-owned ViewStyleProgram
  -> ordered ViewStyleApplicationTarget values on static node producers
  -> runtime-materialized ViewStyleApplication values
  -> arcweft-view::style::ViewStyleResolver
  -> ComputedViewStyle
  -> bundle/player runtime projection
  -> shared native, web, and headless rendering paths
```

CSS source, CSS syntax discriminators, external stylesheet descriptors, and
Takumi adapter data are not inputs to this contract.

## Ownership

| Contract | Owner | Responsibility |
| --- | --- | --- |
| `ViewStyleProgram` | `arcweft-view::style` | Canonical sheets, sheet-local tokens and rules, and inline patches |
| `ViewStyleApplicationTarget` | `arcweft-view::style` | Static named-sheet or inline-patch identity |
| `ViewStyleApplication` | `arcweft-view::style` | Runtime scope, depth, authored order, and View-boundary facts |
| `ViewStyleResolver` | `arcweft-view::style` | Selector matching, token resolution, inheritance, cascade, tracing, and caching |
| `ComputedViewStyle` | `arcweft-view::style` | Fully token-resolved typed properties and winning provenance |
| `ViewStyleResource` | `arcweft-bundle` | Program serialization plus source-map and cross-section references |
| Runtime-control projection | `arcweft-bundle` and player adapters | Mapping computed typed values to control and renderer payloads |

Lower layers remain Sans I/O. Resource loading, platform handles, and renderer
resource acquisition stay in player or platform adapters.

## Static targets and runtime applications

A node-producing View instruction stores an authored-order
`Vec<ViewStyleApplicationTarget>`. It does not store a partially known runtime
scope and there is no trailing standalone `ApplyStyle` instruction.

The runtime driver materializes each target into `ViewStyleApplication` when the
mounted path and nested-View relationship are known. It supplies:

- the named-sheet or inline-patch target;
- a stable runtime scope identity;
- scope depth;
- global application order preserving authoring order;
- `ViewStyleBoundaryFacts` for nested View crossings and exported parts.

Named applications may enter descendant scopes. Inline patches are local to the
node where they were authored. A synthetic part name is never used to represent
an inline patch.

## Resolver input and result

`ViewStyleResolveContext` identifies one concrete node using:

- `ViewStyleNodeKey` for mount, retained path, and instruction identity;
- `ViewStyleNodeFacts` for element kind, implementation/exported part, and all
  simultaneously active interaction and element states;
- root-to-parent ancestry within the visible View boundary;
- the ordered runtime applications;
- the parent `ComputedViewStyle`, when present;
- the complete `PresentationEnvironment` snapshot;
- explicit sheet, patch, token, application, interaction, and container
  revisions;
- the requested trace mode.

Resolution returns `ViewStyleResolution`, containing one `ComputedViewStyle`, an
optional deterministic trace, and whether the bounded cache supplied the
result. Each `ComputedViewProperty` retains its typed value, priority, and
sheet/patch/inherited provenance.

## Inheritance and cascade

Inheritance is a separate first step. The resolver seeds the builder from only
those parent properties for which `ViewPropertyKind::is_inherited()` is true.
Layout, paint, transform, clip, and mask properties are not inherited merely
because a parent has a value.

The resolver then processes every application in order. Named applications
match typed selectors, resolve tokens only from their owning sheet, and emit
per-property contributions. Inline patches use the same contribution path. A
later application is stronger than an earlier application, including when an
inline patch appears between two named applications.

The canonical winner tuple is:

```text
scope depth
  -> application order
  -> selector predicate specificity
  -> selector element specificity
  -> rule source order
  -> declaration order
```

Later tuple values win. Selector traversal across a nested View is permitted
only by the runtime-supplied boundary facts; a numeric scope depth does not by
itself authorize cross-boundary matching.

Interaction and element states are sets rather than one mutually exclusive
slot, so selectors can match combinations such as focus-visible and hover in
the same snapshot. Environment predicates use the typed presentation
environment. A container predicate without runtime container facts does not
match and is visible as a trace rejection; it is never guessed.

## Tokens and append assignment

Named-sheet tokens are resolved from that sheet's token inventory. An inline
patch may reference a token only when that identity has one unambiguous sheet
owner. Missing, ambiguous, cyclic, or over-budget references fail through typed
model or resolver errors; no string suffix matching is performed.

`Append` is a typed list operation. It is valid only for the list-valued
properties accepted by the model, including font-family, shadow, filter,
backdrop-filter, and transition lists. A later `Replace` discards the previously
accumulated list.

## Budgets, cache, invalidation, and trace

The default resolver limits are:

| Work | Limit |
| --- | ---: |
| Applications per resolution | 4,096 |
| Rules visited | 65,536 |
| Contributions retained | 262,144 |
| Tokens inventoried | 65,536 |
| Selector steps | 262,144 |
| Token reference depth | 64 |
| Cache entries | 1,024 |

The cache key includes stable node identity, node and ancestor facts, all style
revision counters, the parent computed revision, and all presentation
environment values and its revision. Eviction is deterministic FIFO. Full
tracing bypasses the cache so rejection and contribution evidence is complete;
the default `Off` mode allocates no trace buffer, while `Winners` retains only
the final provenance.

`ComputedViewStyle::invalidation_from` compares typed property values and unions
the owning property's invalidation class. Consumers do not infer invalidation by
normalizing property names.

## Runtime projection and renderer behavior

The bundle runtime-control layer accepts a computed result and projects supported
properties into runtime surfaces, text controls, and action buttons. Player
adapters then convert those payloads to renderer-owned control styles and
existing View primitives. This keeps `arcweft-render-wgpu` independent of bundle
types while native, web, and headless observation consume the same computed
result.

Projection does not reopen source text, resolve selectors or tokens, normalize
property strings, or interpret synthetic part identifiers. Unsupported target
projection is reported at the typed projection boundary; there are no
player-facing CSS diagnostic categories.

## Errors and diagnostics

Resolver failures are typed native-Style errors for bounded work and broken
references, including missing sheets, patches, or tokens and ambiguous inline
token ownership. `ViewStyleTrace` explains winners and rejected typed selectors
when requested.

Removed authoring syntax has no dedicated CSS-removal diagnostic contract. It
falls through the language parser's ordinary structured recovery, just like any
other unsupported syntax.

## Non-goals

- CSS parsing, external stylesheets, browser DOM controls, canvas/image
  fallbacks, or Takumi-owned cascade/layout.
- A compatibility reader, syntax alias, migration shim, or dual runtime path.
- A second runtime-control-specific cascade or state-slot precedence model.
- Guessing container-query results before typed runtime container facts exist.
- Redesigning the renderer's existing typed paint, shadow, filter, clip, mask,
  or compositing primitives.
