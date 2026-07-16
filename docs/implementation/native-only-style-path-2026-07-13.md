# Native-only typed Style path

- Date: 2026-07-13
- Baseline: `9f905dcd07f500edf40bb7fa510d791598e9ddce`
- Status: implemented and validated on Jujutsu change `oupuzpzo`

## Outcome

Arcweft Style is a single native, typed path. The implementation replaces the
unfinished mixed native/CSS bridge directly; it does not retain a compatibility
reader, deprecated alias, dual resolver, or migration shim.

```text
Arcweft native Style
  -> syntax AST
  -> native-only HIR
  -> CheckedViewStyleCatalog
  -> sheet-owned ViewStyleProgram
  -> ordered static application targets
  -> runtime ViewStyleApplication inventory
  -> single arcweft-view computed-style resolver
  -> bundle/player projection
  -> shared runtime and renderer
```

The Takumi adapter crate and its workspace dependency were removed first so the
compiler exposed every remaining product-path dependency directly.

## Source, HIR, and semantic model

The source model has one Style language:

- `StyleDecl` owns a native typed sheet;
- inline `StylePatch` owns native typed declarations;
- HIR preserves those native inventories and gives each inline patch its sole
  source-order ordinal;
- `CheckedViewStyleCatalog` contains only checked sheets and patches;
- top-level sheets and inline patches use the same property/value checker.

The following provisional discriminators and raw-source contracts were deleted:

- `StyleSyntax`, `StyleDeclBody`, `StyleCssSource`, and CSS patch variants;
- `HirStyleSyntax`, `HirStyleBody`, `HirCssStyleSource`, and HIR CSS fields;
- `CheckedViewStyleSyntax` and checked CSS source/range fields.

### Removed syntax behavior

The original package proposed dedicated diagnostics for `.Css`, `.Arcweft`, and
`.style(.Css)`. The user's final direction explicitly removed that diagnostic
surface. Old markers and CSS-like colon assignments now fail through ordinary
existing `syntax.parse` recovery. No dedicated removal code, message, repair
branch, deprecated spelling, or compatibility parser remains.

## Sheet-owned product resource

`arcweft-view::style::ViewStyleProgram` is the canonical serialized Style model.
It owns:

- named `ViewStyleSheet` values;
- each sheet's typed token and ordered rule inventories;
- inline `ViewStylePatch` values identified by the HIR/sema ordinal;
- typed properties, selectors, values, and assignment operations.

`ViewStyleResource` is a thin bundle wrapper around the program plus source-map
and cross-section references. The old flattened resource and its CSS/source
identity model were deleted, including raw Arcweft/CSS source inventories,
external stylesheet descriptors, weak string/list values, flattened part rules,
and syntax discriminators.

Compiler lowering uses `CheckedViewStyleCatalog` as the semantic source of
truth. It does not reconstruct Style values by re-parsing HIR strings, and it
does not allocate a second inline-patch counter. Product construction and codec
validation preserve sheet ownership and authored rule/declaration order.

## Static target to runtime application adaptation

The design proposal placed a complete `ViewStyleApplication` in a standalone
`ApplyStyle` instruction. The final implementation keeps an ordered
`Vec<ViewStyleApplicationTarget>` directly on every static node producer and on
the View definition root.

This is necessary because scope identity, depth, mounted path, application
order, and nested-View/exported-part boundary facts are runtime facts. The
runtime driver materializes the only complete `ViewStyleApplication` inventory
when those facts are known. Named applications can participate in descendant
scope; inline patches stay node-local. There is no trailing `ApplyStyle`
instruction, `ViewStyleApplyRef`, CSS discriminator, or synthetic
`style.inline.patch.*` part.

## Single computed-style resolver

`arcweft-view::style` owns the only cascade through:

- `ComputedViewStyle` and `ComputedViewProperty`;
- `ComputedViewStyleBuilder`, typed contributions, and `ViewStylePriority`;
- `ViewStyleResolver` and `ViewStyleResolveContext`;
- typed node/state facts, revisions, trace modes, budgets, and bounded cache.

Resolution first copies only properties explicitly marked inheritable from the
parent, then applies ordered named sheets and inline patches. The winner key is:

```text
scope depth
  -> application order
  -> selector predicate specificity
  -> selector element specificity
  -> rule source order
  -> declaration order
```

Named tokens resolve only within their owning sheet. Inline token references
must have one unambiguous sheet owner. Interaction and element states are
simultaneous sets, selector traversal observes View boundaries, append is
limited to typed list values, and resolution is protected by deterministic work
budgets and FIFO cache eviction.

`arcweft-bundle::runtime_control_style` is reduced to projection from computed
typed values into runtime controls. It no longer performs property-name
normalization, generic string-value decoding, token suffix matching, selector
cascade, or synthetic-part matching. Native, web, and headless player paths
consume the same computed contract.

## Deleted product and dependency contracts

| Layer | Removed contract |
| --- | --- |
| Workspace | `arcweft-takumi-adapter` and the upstream Takumi dependency |
| Language | CSS/Arcweft syntax markers, raw CSS bodies, inline CSS parser branch |
| HIR/sema | CSS syntax enums, raw source preservation, unchecked CSS catalog entries |
| View product | `ViewStyleApplyRef`, flattened rules/parts, weak stringly values |
| Bundle | Arcweft/CSS source identities, external CSS descriptors, CSS budgets and discriminators |
| Runtime | bundle-owned cascade, tail token matching, synthetic inline-patch parts, CSS diagnostics |
| Fixtures/tooling | CSS-only fixtures and CSS-named product sample/tool entrypoints |

Historical design and implementation notes retain their bodies for provenance.
Documents whose normative premise was the removed CSS/Takumi product path carry
a superseded note pointing here.

## Changed files summary

The implementation slice changes these ownership clusters:

- native-only syntax, HIR, and semantic catalog/checking under
  `crates/arcweft-lang-syntax`, `crates/arcweft-lang-hir`, and
  `crates/arcweft-lang-sema`;
- checked Style compilation in `crates/arcweft-compiler/src/style.rs`;
- canonical program, values, selectors, computed result, cascade, resolver, and
  trace modules under `crates/arcweft-view/src/style`;
- bundle resource model, codecs, merge/cross-section handling, and runtime
  projection under `crates/arcweft-bundle/src`;
- checked-catalog lowering and static application targets under
  `crates/arcweft-cli/src/app/bundle.rs` and
  `crates/arcweft-cli/src/app/bundle_view`;
- runtime application materialization under
  `crates/arcweft-runtime-driver/src/view_runtime`;
- shared player/renderer projection call sites under the player and renderer
  crates, including responsibility modules for resolution, consumer support,
  layout, and tests;
- workspace manifests and lockfile for Takumi removal;
- native Style samples, fixtures, evidence names, capture tooling, parity gates,
  and related documentation.

Documentation for the final contract is
`docs/design/seq-06.16.4-runtime-control-style-contract.md`; this note records
implementation state and validation evidence.

## Validation

All commands below used one stable feature set and disabled debug information
and incremental compilation for the large validation run.

| Validation | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| Syntax, HIR, and sema Style suites | Pass: 27 + 2 + 11 tests |
| `arcweft-view` metadata, sheet, resolver, and motion suites | Pass: 38 tests |
| Bundle program, codec, projection, and cross-section suites | Pass: 42 tests |
| Compiler Style and CLI library suites | Pass: 3 + 191 tests |
| Runtime-driver View, player Style, player-web, and renderer focused suites | Pass: 14 + 10 + 36 + 15 tests |
| Relevant workspace all-target/all-feature check | Pass |
| Workspace all-target/all-feature clippy with warnings denied | Pass |
| `just test-workspace` | Pass after replacing one stale whole-program equality assertion with direct linked-Style behavior checks |
| `just native-style-layout-coverage` | Pass; canonical AWFB generated |
| `just reactive-view-style-sample` | Pass; interaction-state artifacts generated |
| `just native-style-parity` | Pass for default, compact, and HiDPI; native/web SSIM 1.0 and zero changed pixels |
| Capture Zscript compile checks | Pass for bundle-scene and text-parity tools |
| Canonical structural audit | Pass: 0 errors, 126 warnings; reports under `docs/implementation/structure-audits/native-only-style-path/` |
| Tracked and ignored non-`target` AWFB inspection | Pass across 14 bundles; all 10 tracked bundles decode, and none contain removed CSS source, descriptor, discriminator, synthetic-part, or Takumi identities |

## Independent follow-up design requests

The native-only d.1 through d.3 implementation is complete. The remaining
authoring, adaptive, and explain surfaces require decisions that were not fixed
by the package. They are split into new, self-contained requests so none of
them silently expands this completed cut or redesigns its substrate:

- [seq-06.11d.2.1.1.1 exported-part production reconciliation](../reviews/requests/2026-07-15-seq-06.11d.2.1.1.1-view-exported-part-authoring-production-reconciliation.md);
- [seq-06.11d.4.1 native logical-axis Style](../reviews/requests/2026-07-14-seq-06.11d.4.1-native-logical-axis-style-contract.md);
- [seq-06.11d.4.2.1 native environment Style production reconciliation](../reviews/requests/2026-07-15-seq-06.11d.4.2.1-native-environment-style-condition-production-reconciliation.md),
  whose supplied implementation ZIP was re-audited on 2026-07-15 and has no
  checkout, patch, baseline, manifest, changed file, or verified result. Its
  sole substantive input is the earlier request itself, so no production
  payload can be integrated. The linked standalone request now records the
  landed d.4.1 boundary and concrete specificity, text-scale, enum-comparison,
  revision, codec, trace, and tooling decisions that a final design must close;
- [seq-06.11d.4.3 native container conditions and invalidation](../reviews/requests/2026-07-14-seq-06.11d.4.3-native-container-style-condition-invalidation-contract.md);
- [seq-06.11d.5.1.1 native Style trace reconciliation](../reviews/requests/2026-07-14-seq-06.11d.5.1.1-native-style-trace-contract-reconciliation.md),
  which replaces the original d.5.1 capability brief for dispatch and is
  self-contained, but remains gated until the d.4.1 core, d.4.1.1 → d.4.1.2
  provider/geometry branch, d.4.2, and then d.4.3 have landed and passed
  validation;
- [seq-06.11d.5.2 Agent Style observation](../reviews/requests/2026-07-14-seq-06.11d.5.2-agent-native-style-observation-protocol.md);
- [seq-06.11d.5.3 Style LSP and formatter](../reviews/requests/2026-07-14-seq-06.11d.5.3-native-style-lsp-formatter-contract.md).

The d.4.1 logical-axis core and d.4.1.1 host seed/provider lifecycle are landed.
The d.4.1.2 physical-geometry and d.4.2.1 environment reconciliation designs
may now run in parallel. View d.2.1.1.1 design may also run now, but its later
implementation should land before environment when they overlap source-map,
resolver, codec, player, formatter, or LSP owners. d.4.3 container conditions
begins only after d.4.1.2 and d.4.2.1 implementations have landed, because it
consumes the mounted provider revision, final measured geometry, and the
zero-specificity environment activation model. The d.5.1.1 trace
reconciliation then inspects all of those landed contracts before fixing
provider/environment/container evidence, revision, cursor, or cache bindings.
Agent observation and the LSP/formatter contract follow the reconciled trace
contract; those two may then be designed in parallel. Each request repeats its
fixed substrate and acceptance criteria and can be handed to a designer without
this implementation
note.

## Structural audit disposition and follow-ups

The canonical audit reports no error-level findings. Style-specific production
files introduced or substantially reshaped by this cut are below their warning
thresholds: the player responsibilities are 711 physical LOC for the resolver,
608 for consumer projection, and 334 for layout; compiler Style lowering is 768
LOC; runtime scope materialization is 363 LOC.

`crates/arcweft-bundle/src/resource_codec/view/codec.rs` remains a warning at
2,360 physical LOC and grew by more than 300 LOC in this cut. It still owns the
canonical orchestration across View program, Style, text, input, and theme
sections, while transcript, merge, Style model, Style contract, and runtime
projection responsibilities have already moved to named modules. It remains
below the 2,500 LOC error threshold. A later mechanical decomposition should
move the remaining section codecs behind those responsibility modules; doing
that in this cut would mix a broad non-Style file move into the product-contract
replacement without changing observable behavior.

Container predicates intentionally do not match until typed runtime container
facts are supplied; the resolver trace reports that rejection rather than
guessing. Building the container-fact dependency graph is not part of this cut.
Likewise, `ReadOnly` and `Invalid` remain typed selector states without player
fact producers because the current UI model exposes neither state. Their source
and runtime contracts require separate design before implementation.

## Design deviations and final directions

- The Takumi crate was deleted immediately instead of being temporarily
  quarantined behind a private adapter API.
- Per the user's final override, removed CSS/explicit-marker syntax uses
  ordinary structured parser recovery rather than dedicated removal diagnostic
  codes.
- Static node producers retain ordered application targets; the runtime driver
  constructs full applications. This replaces the proposal's standalone
  `ApplyStyle` instruction because boundary and scope facts are dynamic.
- The resolver responsibility is split into cohesive `computed`, `cascade`,
  `resolver`, and `trace` modules under one `arcweft-view::style` boundary.

## Non-goals

- Preserving or migrating unpublished mixed CSS/native bundle payloads.
- Reintroducing CSS through aliases, a sidecar reader, a browser path, or a
  private runtime escape hatch.
- Adding a source-text grep gate; invariants are tested through typed APIs,
  codecs, parser behavior, and dependency structure.
- Redesigning generic renderer paint, shadow, filter, clip, mask, or compositing
  algorithms that remain useful for native typed Style values.
