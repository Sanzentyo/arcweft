# Final contract

## 1. Final inventory

| Retained identity family | Authored top-level declaration | Source keyword | Public AST item | HIR item | Non-source owner |
|---|---:|---|---|---|---|
| Asset | no | none | none | none | project/build asset catalog and bundle virtual-file inventory |
| Character | yes | `character` | `Item::Character` | `HirItemKind::Character` | Character registry is a projection of the one project symbol |
| View | yes | `view` | `Item::View` | `HirItemKind::View` | View catalog and callable facet project from the same item |
| Action | yes | `action` | `Item::Action` | `HirItemKind::Action` | typed channel/callable facet projects from the same item |
| Activity | yes | `activity` | `Item::Activity` | `HirItemKind::Activity` | concrete implementation binding belongs to manifest/adapter admission |
| Signal | yes | `signal` | `Item::Signal` | `HirItemKind::Signal` | runtime producer/storage policy is not source declaration metadata |
| Metric | yes | `metric` | `Item::Metric` | `HirItemKind::Metric` | exporter/storage selection remains runtime/profile owned |
| Layer | yes | `layer` | `Item::Layer` | `HirItemKind::Layer` | presentation tree materialization projects from the declaration |

There is no generic `entity` declaration and no authored `asset` shell. `res` remains the independent configured-resource declaration selected by Lang-01.4.

## 2. Global invariants

1. A source declaration is parsed once by the lossless grammar transaction. Expression, type, pattern, parameter, path, reference, delimiter, attribute, documentation, recovery, and missing-token descendants are common typed nodes, never family-local text fragments.
2. Every identity-bearing syntax node is attached to its exact lossless Rowan node by the accepted syntax database transaction. No range search, source-text match, line ID reuse, traversal-order reconstruction, or second parse participates.
3. The seven authored declarations share one typed retained header contract but retain seven distinct item kinds and seven distinct body/signature types.
4. `RetainedIdentityFamily` remains the owned eight-variant identity vocabulary. Missing prefix/family behavior is added as inherent behavior on that enum or on a dedicated Arcweft-owned identity newtype; it is not reimplemented as scattered string matches, local extension traits, or endpoint-named conversion helpers.
5. All source-authored retained declarations require one ordinary non-keyword, non-dotted local name. An explicit declaration public ID is optional, absolute, and family-checked. Omission derives `family.<name>` through the owned family API.
6. Every declaration, including a private declaration, has a stable semantic `PublicId`. Visibility affects accessibility and re-export, not identity creation.
7. Top-level local names share the module project-symbol namespace with ordinary callables, nominal types, `res`, and other declarations. Semantic `PublicId` values share one project-wide identity namespace. No signature overloading or family-specific duplicate table exists.
8. Character surface alias, authored display label, declaration local name, semantic public ID, dense runtime/catalog index, and session-local syntax/HIR IDs are separate identities and are never substituted for one another.
9. Recovered syntax is lossless and inspectable. Any error-severity diagnostic owned by a retained declaration poisons that declaration for semantic publication: lowering records an error item/source slot for tooling, but project symbol registration and executable/runtime projections do not occur.
10. Asset references resolve against the catalog generated from the canonical asset virtual-file inventory. Asset bytes, virtual path, digest, media metadata, and build dependencies never enter source AST or HIR declaration payloads.
11. View declaration and View callable are one source item, one HIR item, and one project symbol with a callable facet. Action similarly has one item/symbol with a typed channel facet. No clone-based second callable owner is permitted.
12. Activity source declares only an abstract interface. Concrete Rust, WASM, process, crate, adapter, module, or path origin is structurally impossible in its grammar and belongs to typed manifest/adapter admission.
13. `content`, `source`, `extern mod`, `dialogue defaults`, old configured-resource family keywords, concrete Activity origins, and regular-project top-level statements receive ordinary current-grammar `ErrorItem` recovery. The final tree contains no historical kind or dedicated removed-spelling diagnostic.
14. The public syntax switch is atomic: all source-backed callers move to the attached `ParsedSource` authority and the generic detached entity declaration API is deleted in the same compiling cut.
15. Arena-HIR publication follows the accepted Proof sequencing. This contract supplies exact retained-declaration payloads; it does not create a declaration-only HIR database, a second public HIR, or an early bypass around the broader Stage 3/5/6 authority switches.

## 3. `res` separation

`res` is the sole authored configured-runtime-resource declaration. It has a typed nominal resource head and typed field initializers. It may contain typed references to catalog assets and retained declarations where the nominal resource schema permits them. It does not own packaged bytes.

`asset` is not an alias, nominal `res` subtype, special `res` branch, or declaration keyword. A top-level token sequence beginning with `asset` is ordinary non-executable recovery. An `@asset...` value is a typed entity reference whose target is catalog-owned.

## 4. Metadata contract

- Consecutive outer documentation and outer attributes immediately preceding a declaration attach to that declaration. A blank logical line or an ordinary non-documentation comment terminates the attachment run.
- Inner attributes do not exist in any of the seven retained bodies.
- Attribute syntax is common and typed. Attribute applicability is decided by the common attribute registry using the exact retained family target. This contract introduces no family-specific attribute parser and no attribute-to-body-field fallback.
- A non-repeatable attribute repeated on one declaration produces one common duplicate-attribute error with primary range on the duplicate and related range on the first.
- Unknown or wrong-target attributes remain typed and produce common semantic diagnostics; they are not copied into an untyped bag.

## 5. Range contract

- The item range starts at the first attached outer doc/attribute, or at visibility/family when no prefix exists, and ends after the closing delimiter or optional bodyless semicolon. It excludes the following newline and unrelated trailing trivia.
- The private `DeclarationHeader` lossless node includes attached outer prefixes and ends immediately before the body or bodyless terminator. Public access additionally exposes `core_header_range`, structurally spanning visibility/family through the last family-specific header child.
- Body ranges include opening and closing delimiters, including a zero-width missing-close node when recovery supplies the close.
- Member ranges exclude the logical terminator consumed by the enclosing list. Each delimiter, missing node, name, ID, alias, type, pattern, expression, parameter, reference, export, port, label, bucket, and policy has its own exact node range.
- All ranges are obtained from attached nodes and revision-bound `SourceDocument` ownership; no payload stores a copied range as an alternate authority.

## 6. Completion condition

Implementation is complete only when the private seven-row grammar remains green; source `asset` remains absent; the public generic entity declaration and cloned/stringly HIR are deleted; all downstream consumers use typed declarations or the asset catalog; the 184 direct rows in `TEST_MATRIX.md` pass; and the validation and structural gates in this archive are recorded without claiming unrun commands.
