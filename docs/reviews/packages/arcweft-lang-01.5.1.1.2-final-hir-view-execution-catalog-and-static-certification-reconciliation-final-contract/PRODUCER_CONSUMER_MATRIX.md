# Producer/consumer matrix

| Owner | Produces/owns | Consumer boundary | Normative rule |
|---|---|---|---|
| arcweft-lang-syntax | lossless CST/AST, View declarations, attributes, attached bodies | typed syntax nodes/ranges | No execution or static proof. Stage C7 only adds #[static] in existing attribute grammar. |
| arcweft-lang-hir | arena View item/expr/member/scope/source roles | ItemId/ExprId/LocalId/HirSnapshotId/SyntaxNodeId-backed source queries | Retains current language surface; no old flattened HIR. |
| arcweft-lang-sema | CheckedViewCatalog and static disposition | read-only FinalSemanticAnalysis APIs | Sole semantic authority and complete dependency graph. |
| arcweft-resource-model | ResourceRefValue and nominal runtime conversion context | typed inherent methods and registry digests | No source lookup or generic Presentable trait. |
| arcweft-compiler::view | AWBC-backed View value programs, product transaction | CompiledViewProductCandidate | Consumes only HIR + matching checked catalog. |
| arcweft-compiler image/style/Fx/dialogue | accepted cross-catalog bindings | typed catalog references/digests | No copied endpoint catalog. |
| arcweft-view | instruction algebra, accepted View/program/revision identity, and revision-scoped typed coordinates | ViewProgram/instructions | Sans I/O; no expression evaluator and no serialized syntax/HIR identity. |
| arcweft-bundle ViewProgram codec | strict transcript, digests, certificates | ValidatedViewProduct | AWVP field 1 direct replacement. |
| arcweft-bundle ViewText codec | literal/localized/RichText/dialogue/program text records | validated typed text sources | Deletes string projection/local variants. |
| arcweft-bundle Input/Style/image codecs | typed cross-section property/resource bindings | validated records | No new codec section. |
| arcweft-bundle merge/product section | atomic candidate merge | accepted product content root | No partial View publication. |
| arcweft-runtime-plan/host | ordinary AWBC program and host capabilities | validated executable/host plan | Direct await and handlers use existing owners. |
| arcweft-runtime-driver catalog | accepted lookup indexes/certificate joins | ViewProgramCatalog | One catalog per generation. |
| arcweft-runtime-driver evaluator | mounts, values, projection, resources, frames | BundleViewFrame | One static/dynamic evaluator. |
| arcweft-runtime-driver replacement | candidate validate/reconcile/swap | replacement outcome | Stale/tampered candidate leaves active state unchanged. |
| arcweft-save/session_save | quiescent semantic snapshots | schema v2 snapshot | No certificate/cache/static-path payload. |
| native player | shared renderer-neutral frame | native render/input | No native authoring resolver. |
| Web player | shared renderer-neutral frame | browser render/input | No CSS/Takumi path. |
| headless | shared frame and deterministic observations | test/host output | Parity oracle. |
| Agent/MCP | redacted shared observation and typed inputs | protocol observation/action | No endpoint catalog authority. |
| generated artifacts | program/node/certificate digest binding | generated binding metadata | Reject stale artifact identity. |
| LSP/tooling | semantic catalog read API and exact source roles | diagnostics/hover/navigation | No compiler product reconstruction. |

Every consumer validates the accepted generation before use. No row may create a second semantic or resolution authority.
