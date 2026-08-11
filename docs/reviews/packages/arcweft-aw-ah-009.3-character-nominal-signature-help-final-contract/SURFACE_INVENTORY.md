# Surface inventory

## 1. Inventory method

This inventory follows current typed syntax, HIR, checker dispatch, project
symbol ownership, accepted environment facts, and Rust-adapter publication at
Git commit `76d39983ad8770a87d6e81745785b6b362a381b4`.

“Applicable” means that a cursor can be inside an exact current argument list
and the shared semantic resolver can produce a callable signature. “Character
nominal” means a parameter can carry `TypeKind::CharacterNominal`; it does not
mean a label happens to contain a character-like word.

## 2. Parenthesized expression calls

| Surface | Syntax/HIR | Semantic owner and signature source | Character nominal capability | Decision |
| --- | --- | --- | --- | --- |
| FX constructor | `Expr::Call`; current checker probes `check_fx_constructor_call` first | sema FX catalog and shared call resolver | only when its sema-owned typed schema contains a character nominal | applicable |
| enum variant constructor | `Expr::Call` | sema enum variant payload catalog; expected type may disambiguate | payload can be any registered `TypeKind`, including character nominal | applicable |
| `Ok(...)` / `Err(...)` | `Expr::Call` | sema result-constructor specification | payload may be character nominal when the expected `Result` type carries it | applicable |
| builtin | `Expr::Call` | sema builtin specification | no current builtin parameter is character nominal, but it uses the generic query shape | applicable |
| Agent intrinsic | `Expr::Call` | sema agent intrinsic/action inventory | an accepted intrinsic parameter can be character nominal if its typed action signature says so | applicable |
| presentation special | `Expr::Call` | sema-owned `PresentationCallableId::signature_schema` | `show.look` is character nominal; other current parameters are not | applicable |
| project source function | `Expr::Call` path | `CallableDeclarationId`, `ProjectSymbolTable`, and project callable facts in the accepted registered environment | yes when its normalized parameter is character nominal | applicable |
| extern capability function | `Expr::Call` qualified path | accepted project/environment signature facts | yes when declared/registered type normalization yields character nominal | applicable |
| standard environment function | `Expr::Call` path | `EnvironmentCallableId::Function` in accepted `RegisteredTypeCheckEnv` | yes | applicable |
| Rust-adapter function | `Expr::Call` path | adapter metadata normalized to `EnvironmentCallableId::Function` before accepted publication | yes when typed metadata carries it; display-only Rust text cannot create it | applicable |
| selected/method call | `Expr::Call` with `Expr::Select` callee | receiver type plus accepted project or environment method candidates | yes | applicable for accepted methods |
| current source `impl` method | source declaration exists, but no canonical published project method signature is present | checker body ownership only | cannot be queried until normal project method publication exists | not currently applicable; no synthetic publication |
| first-class function value | arbitrary call callee whose type is `TypeKind::Function` | target-mode type checker and shared resolver | yes when function parameter vector contains it | applicable |
| curried/partial call | nested `Expr::Call`; `remaining_param_group` | `FunctionSignature` and function-value type | yes in any current call group | applicable |
| overload set | one resolved lookup key with multiple typed candidates | ordered candidate set in project/registered world | yes per candidate | applicable; ambiguity is partial help |
| unresolved path | `Expr::Call` | no semantic owner | none | `NotApplicable(UnknownCallee)` |
| resolved non-callable | `Expr::Call` | project/environment symbol says value is not callable | none | `NotApplicable(NonCallableCallee)` |

## 3. Presentation special forms

Current checker-owned names are exhaustively classified below. The final
implementation moves their schema to inherent behavior on
`PresentationCallableId` and makes the checker consume it.

| Authored name | Required/fixed parameters | Named parameters represented by the schema | Open values | Character nominal position |
| --- | --- | --- | --- | --- |
| `view` | positional `view: Ref<View>` | `lifetime`, `target: Ref<Target>`, `layer: Ref<Layer>`, `id`, `handle`, `key`, `mount`, `depth: I32`, `visible: Bool`, `enabled: Bool` | current unknown named values map to `OpenNamed` | none |
| `menu` | positional `view: Ref<View>` | same as `view` | `OpenNamed` | none |
| `overlay` | positional `view: Ref<View>` | same as `view` | `OpenNamed` | none |
| `bg` | positional or named `asset: Ref<Asset>` | `target: Ref<Target>`, background `slot`, `scope`, `fade`, and current image-common options | loose options are `Unconstrained`; unknown names remain rejected | none |
| `image` | positional `source: Ref<Image> | Ref<Asset>` or named asset form | `lifetime`, `target`, `layer`, `id`, action/proxy/focus/input/owner/drop fields, alignment, depth, opacity, visibility, dimensions, transform, and playback fields | fields checked only structurally are `Unconstrained`; unknown names rejected | none |
| `player_viewport` | no fixed required parameter beyond current checker behavior | `width`, `height`, `fit` | positional/loose values are explicit `Unconstrained` schema entries | none |
| `show` | positional-only `character: Ref<Character>`; optional positional-or-named `look` | `target: Ref<Target>`, character `slot`, `scope` | current additional named values map to `OpenNamed` | `look: CharacterNominal(Look, resolved character)` |
| `ref.bg` | none | background `target`, `slot`, `scope` | `OpenNamed` | none |
| `ref.show` | positional-only `character: Ref<Character>` | character `target`, `slot`, `scope` | `OpenNamed` | none; current checker has no look branch here |
| `clear.bg` | none | background `target`, `slot`, `scope` | `OpenNamed` | none |
| `hide` | positional-only `character: Ref<Character>` | character `target`, `slot`, `scope` | `OpenNamed` | none |

### Character-stage names not implemented as special forms

The current checker has no independent `move`, `face`, `anim`, or
`alice.stage.*` presentation-special dispatch. Documentation or runtime intent
is not callable evidence. Such syntax receives signature help only when the
ordinary selected-call resolver finds a currently accepted typed method.
Nothing is synthesized for this feature.

## 4. Dialogue and speaker surfaces

| Surface | Syntax | HIR | Semantic owner | Character nominal position | Decision |
| --- | --- | --- | --- | --- | --- |
| colon speaker line | `alice(look=...): Text` | `HirDialogue` with `SpeakerLineSurface` and `look` field | dialogue schema plus project symbol/registered owner resolution | named `look` is `CharacterNominalType::look(alice)` when callee resolves to a character | applicable only inside the optional parentheses |
| colon speaker line without options | `alice: Text` | same HIR, no argument list | dialogue checker | no cursor-owned argument list | not applicable |
| canonical content call | `alice.say(look=...)[Text]` or current canonical equivalent | `HirDialogue` / `ContentCall` | dialogue schema plus selected/path resolution | named `look` is character nominal for a character owner | applicable inside parentheses; content brackets are outside |
| content call without options | `alice.say()[Text]` | exact empty argument list | same | look is an available next parameter when owner is character | applicable inside `()` |
| speaker/preset expression call | `speaker(options)` whose callee semantically has `Speaker` or `SpeakerPreset` | `Expr::Call` | dialogue schema selected by shared resolver | character speaker yields structural look; non-character preset uses registered schema | applicable |
| dialogue line forwarded arguments | named `LineArg` values not reserved by line options | `HirDialogue::args` | dialogue `OpenNamed` parameter | only a registered option explicitly typed character nominal has it | applicable as open named; no spelling inference |
| inline dialogue tag | `[face ...]`, `[move ...]`, and other `DialogueTag` forms | dialogue content/tag AST, not `Expr::Call` | dialogue tag checker/runtime plan | not an ordinary callable parameter | not applicable to this query |
| line content brackets | `[Text]` / rich content | dialogue content AST | dialogue content checker | none | not an argument list |

The current dialogue checker calls ordinary expression checking for `look`.
The implementation changes it to consume the shared dialogue schema and pass
the structural expected type.

## 5. Flows, functions, constructors, and callable declarations

| Family | Current callable-signature source | Parenthesized call surface | Character nominal result |
| --- | --- | --- | --- |
| source `fn` / stream function | HIR `FnSignature`, project declaration ID, normalized accepted callable facts | yes | applicable; any normalized parameter can carry structural nominal identity |
| flow declaration | HIR `HirFlow::signature`; parameters are used for flow checking, while `goto` expects `Ref<Flow>` and current callable maps do not publish flows as ordinary function calls | no current ordinary parenthesized flow-call authority | no signature help; generic flow invocation remains a separate language decision |
| Agent declaration | HIR Agent signature and effect boundary | not an ordinary expression-call target merely because it has a body signature | no synthetic call surface |
| top-level `callable` declaration | retained HIR declaration/effect contract but no current expression-call signature publication | no | no synthetic call surface |
| struct/record literal | dedicated record-literal syntax, not `Expr::Call` | no | not signature help |
| enum variant constructor | typed call resolver | yes | applicable |
| `Result` constructor | typed builtin/result resolver | yes | applicable |
| source type alias | type system only | no | not callable |
| closure/function local | `TypeKind::Function` from target-mode checker | yes | applicable |
| partial/curried declaration | `remaining_param_groups` | yes, one list per group | applicable |

## 6. Adapter/Rust surfaces

Rust exports currently enter `TypeCheckEnv` through adapter manifest
application. Current LSP bypasses sema and asks `arcweft-verify-lsp` for the
first metadata function matching a word. That path is removed.

The final path is:

```text
accepted adapter manifest
  -> typed EnvironmentCallableId + FunctionSignature + documentation
  -> RegisteredTypeCheckEnv inside RegisteredSemanticWorld
  -> shared sema resolver
  -> SemanticSignatureHelp
  -> LSP formatting
```

An adapter-only callable remains supported. Same-name project and adapter
callables follow project precedence. Duplicate same-rank accepted identities
are rejected rather than selected by map or iteration order.

## 7. Incomplete and recovered calls

| Recovery shape | Typed range available | Outcome |
| --- | --- | --- |
| `f(` | callee/open/content/recovery end | partial signature help, active slot 0, missing-close diagnostic |
| `f(a,` | arguments/separator/recovery end | partial help, next slot active |
| `f(name =` with recovered value | named argument and zero-width/recovered value range | partial help, named parameter active, recovered diagnostic |
| malformed inner call inside valid outer call | both exact lists | deepest containing list wins |
| raw recovery with no structural callee/list | no | typed semantic-unavailable request error when cursor claims that range; otherwise no applicable list |
| comment/string containing `f(` | no call node | no result |
| a word matching an adapter function outside `()` | no list | no result |

## 8. Character identity scope matrix

| Example typed parameter | Local spelling | Equality scope | Coalescing |
| --- | --- | --- | --- |
| Alice look `happy` | `happy` | family `Look` + Alice `CharacterId` | only exact same nominal identity |
| Bob look `happy` | `happy` | family `Look` + Bob `CharacterId` | distinct from Alice |
| Alice part `face` variant `happy` | `happy` | family `Variant` + Alice + part `face` | distinct from Alice look and other parts |
| Alice part `body` variant `happy` | `happy` | family `Variant` + Alice + part `body` | distinct from `face` |
| alias `hero` for Alice | authored alias | display only | resolves to Alice; does not create a nominal identity |

## 9. Generic native signature help boundary

Generic native signature help remains part of this implementation. Character
nominal support is one typed case in the shared result. Current families that
lack a callable authority remain explicit non-applicable surfaces; they are not
placeholders and do not receive a word fallback.
