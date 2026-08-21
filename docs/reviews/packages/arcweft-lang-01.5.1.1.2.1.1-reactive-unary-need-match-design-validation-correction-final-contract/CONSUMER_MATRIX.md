# Producer/consumer matrix

Every row preserves one dependency direction and forbids a second authority.
| ID | Owner | Produces | Consumer | Rule |
|---|---|---|---|---|
| C01 | arcweft-lang-syntax | lossless ordinary match/View surface | HIR lowering | no AwaitView/execution |
| C02 | arcweft-lang-hir | ExprId/PatternId/LocalId/source roles | final sema | session identities only |
| C03 | arcweft-lang-sema final analysis | CheckedViewCatalog + Need facts | compiler/tooling | sole checked authority |
| C04 | arcweft-lang-sema type/ownership | exact Need/T and retainability | catalog admission | reject affine/borrow/must-drop |
| C05 | arcweft-runtime-plan | generation-bound normalized facts | AWBC lowerer | no source reconstruction |
| C06 | arcweft-compiler::view | scratch View/AWBC/subscription product | CompiledProject | all or none |
| C07 | arcweft-core AWBC | function/task/pattern/match/type bindings | bundle/runtime | ordinary executable owner |
| C08 | arcweft-core RuntimeNeedState | state/cursor/digest/projection | selector | inherent owner |
| C09 | arcweft-view program | generic Match/local coordinates/mount IDs | bundle/runtime | Sans I/O; no core dependency |
| C10 | arcweft-view retained state | mount/local/arm occurrence | save/runtime | mount-scoped |
| C11 | arcweft-bundle model | strict v1 subscription/Match DTOs | codec/merge/runtime | no old Await DTO |
| C12 | arcweft-bundle codec | closed tags/canonical tables/budgets | ValidatedViewProduct | old bytes reject |
| C13 | arcweft-bundle digest | subscription/selector/arm contracts | merge/replacement | source excluded |
| C14 | arcweft-bundle merge | scratch candidate | accepted product | no partial publication |
| C15 | arcweft-bundle source maps | typed diagnostic refs | runtime/LSP | not identity |
| C16 | runtime View catalog | immutable accepted definitions/subscriptions | evaluator/replacement | one catalog/generation |
| C17 | runtime Need journal | selected generation/cursor/state | all observers | one producer authority |
| C18 | runtime observer table | mount/subscription/arm/invalidation | frame/save | independent observers |
| C19 | runtime evaluator | AWBC selector + generic Match + frame candidate | BundleViewFrame | no View matcher |
| C20 | runtime task registry | start dedup/cancel/generation | host/publications | producer lifecycle |
| C21 | runtime save | v1 producer/publication/observer/queue | restore | scratch then swap |
| C22 | runtime replay | ordered publication journal | same live API | no second selector |
| C23 | runtime replacement | semantic/contract join/reconcile | active runtime | atomic swap |
| C24 | native player | shared frame | native renderer/input | no resolution |
| C25 | Web player | shared frame | browser renderer/input | no endpoint authority |
| C26 | headless | shared frame/diagnostics | differential host | parity oracle |
| C27 | Agent/MCP | redacted observation/actions | protocol clients | no endpoint catalog |
| C28 | generated artifacts | program/subscription/contract digests | bindings/loaders | reject stale |
| C29 | LSP/tooling | checked catalog/source roles | diagnostics/navigation | no product reconstruction |
| C30 | docs/structure gates | maintained rules/zero old APIs | admission | no stale direct Await |
