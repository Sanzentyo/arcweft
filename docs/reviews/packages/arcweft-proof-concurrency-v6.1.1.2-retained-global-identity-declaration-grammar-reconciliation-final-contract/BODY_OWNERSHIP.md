# Body and semantic ownership

## 1. Ownership rule

Syntax owns authored structure and exact source identity. HIR owns typed, source-backed compiler structure. Sema/project admission owns resolution, type constraints, constant requirements, collisions, and poisoning. Runtime/product crates consume admitted values and must not reinterpret declaration source.

## 2. Asset

| Concern | Owner |
|---|---|
| asset root selection | project/profile configuration |
| directory walking and byte reads | CLI/build/project-loader adapter |
| normalized virtual path | typed asset virtual-path value |
| `asset.*` ID derivation | `AssetId`/Arcweft ID owner |
| duplicate normalized ID | project catalog transaction |
| bytes and content digest | bundle/container owner using existing digest contract |
| media/format/decode metadata | format-specific compiler/bundle owner |
| source AST/HIR declaration | none |
| reference resolution | unified project symbol table against catalog symbols |
| liveness | project/catalog generation, not syntax/HIR revision |

## 3. Character

Syntax body owns zero or one `display_name = Expr`. HIR stores `Option<ExprId>`. Sema requires a constant `String`. Omission means the accepted Character product derives its display label from the Character surface alias when present, otherwise from the declaration name; this is a product projection and does not synthesize a source expression or HIR expression.

Voice, Style, View, presentation, localization, dialogue defaults, and manifest fields are not Character body members. They remain in their accepted resource/catalog/profile owners. CharacterDialogue construction remains ordinary callable/dialogue syntax using the registered Character identity.

## 4. View

Syntax owns:

- fixed typed parameters and optional default expressions;
- ordered leading exported-part declarations;
- one View fragment containing ordered common expression descendants;
- exact View body, delimiter, exported-part, Style patch, and recovery nodes.

HIR owns parameter IDs, pattern/type/default IDs, export member IDs with typed local/public part paths, and expression IDs. The View item itself owns the callable facet; no second callable item or cloned function body exists.

Sema/compiler owns parameter type/default compatibility, View expression checking, exported-part validation, accepted Style patch behavior, catalog admission, mount product, and dialogue-View role projection. `ViewId(PublicId)`, dense `ViewRegistryId`, and private catalog index remain distinct.

## 5. Action

Syntax owns one fixed parameter list. HIR owns ordered parameter records with pattern, type, local binding, and source IDs. Sema owns duplicate names and type validity. The project symbol exposes a typed channel/callable facet whose payload is the ordered parameter product; zero parameters is `Unit`.

No body, return value, default parameter, effect annotation, capability metadata, or concrete handler belongs to the declaration. Runtime emission and receive operations refer to the same Action public ID and schema; they do not parse a signature display string.

## 6. Activity

Syntax/HIR own only the abstract interface:

- closed mode;
- closed lifecycle policy;
- ordered typed input ports;
- ordered typed output ports;
- ordered requires and ensures expression IDs.

Defaults are `deterministic`, `stateless`, empty ports, and empty contract. Sema owns unique port names, type validity, contract Bool/purity/context rules, and requires-before-ensures poison already emitted by syntax.

Manifest/adapter admission owns concrete implementation origin and verifies exact interface compatibility. A source Activity may exist without an implementation for editing and static analysis, but executable compilation fails with a structured missing/incompatible-binding diagnostic carrying the Activity declaration and manifest binding spans. No path/origin is stored in syntax or HIR.

## 7. Signal

Syntax/HIR own one typed observable type. Sema admits only:

- `Watch<T>`;
- `Stream<T, E>`; and
- `Sample<T>`.

Runtime producer, write authority, observation scheduling, replay, persistence, and host binding remain in runtime/profile/adapter owners. There is no initializer or source policy field.

## 8. Metric

Syntax/HIR own:

- closed metric kind;
- typed numeric value type;
- optional decoded unit string with source member identity;
- ordered typed label member IDs;
- optional ordered bucket expression IDs.

Sema owns numeric-type admission, label value capability, constant evaluation, histogram bucket ordering/representability, and kind/member compatibility. Runtime/profile owns storage, aggregation/export transport, retention, and a typed default histogram bucket policy when one is configured. No arbitrary kind or backend word survives as text.

## 9. Layer

Syntax/HIR own the closed kind and optional singleton members. Omitted values are represented as absence in HIR; defaults are applied by inherent methods on the owned Layer kind/policy types during admitted presentation-plan construction, not by parser-synthesized expressions.

Sema/project owns:

- family resolution for parent/View/Activity references;
- parent existence and cycle detection;
- constant/type checks for z, visible, and transform;
- content-reference exclusivity and kind compatibility;
- accessibility and cross-module visibility.

Presentation owns deterministic `LayerTree` construction, order `(phase, z, stable_index)`, transforms, visibility, hit-testing, input, capture, accessibility, and render content. `stable_index` is allocated deterministically from accepted project source order after phase and z; it is not authored and is not semantic identity.

## 10. Documentation and attributes

Syntax owns exact doc/attribute nodes. HIR carries typed attribute products and documentation handles/decoded content required by compiler tooling, with source slots for each attached node. Sema's common attribute registry decides repeatability and target validity. No family body parser interprets unknown attributes as members, and no HIR payload keeps raw attribute source for downstream parsing.
