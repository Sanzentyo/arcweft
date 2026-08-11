# Direct migration and deletion

## 1. Public replacement rule

After any independently mergeable private substrate cut, the first public
change to `SyntaxKind`, `Expr`, or `HirExprKind` begins one unmerged direct
replacement series. That series must migrate every exhaustive consumer, delete
the old forms, restore workspace compilation, and pass the required validation
before it may be committed, merged, or offered for review.

There is no reviewable syntax-only or HIR-only public cut that knowingly leaves
the workspace uncompilable.

## 2. Syntax production owners

| Current owner/consumer | Required direct migration |
|---|---|
| `crates/arcweft-lang-syntax/src/grammar/kinds.rs` | add generic postfix/colon kinds and roles in the original enums; delete `DialogueCallExpression`; stop CST index selection |
| `crates/arcweft-lang-syntax/src/grammar/budget.rs` | charge generic node/candidate work through existing document transaction; no new public limit |
| `crates/arcweft-lang-syntax/src/parser/expression.rs` | delete `emit_dialogue_context_expression`, `DialogueSurface`, spelling/shape heuristics, and direct postfix index/dialogue emission; emit one generic postfix node |
| `crates/arcweft-lang-syntax/src/parser/dialogue.rs` | replace speaker/string application parsing with colon application and typed content attachment; delete string reconstruction and bracket search |
| `crates/arcweft-lang-syntax/src/parser/helpers.rs` and `src/parser.rs` | remove dialogue call/delimiter source-search helpers and route owner boundaries to the final parser |
| `crates/arcweft-lang-syntax/src/expr.rs` | direct enum replacement; add final payloads; make `EntityRef` carry `IdRef`; delete `DialogueCall` |
| `crates/arcweft-lang-syntax/src/expr/source_ranges.rs` | use typed surface accessors; delete post-parse content-range collection |
| `crates/arcweft-lang-syntax/src/ast/dialogue.rs` | keep content model; delete `SpeakerLine`, `SpeakerLineSurface`, string `ContentCall`, and speaker-derived application helpers |
| `crates/arcweft-lang-syntax/src/ast/ids.rs` | retain `IdRef`; delete `EntityRefSyntax` after expression/declaration callers migrate; use checked authored-range construction |
| `crates/arcweft-lang-syntax/src/ast/flow.rs`, `src/ast/view.rs`, `src/ast/line_plan.rs`, `src/parser/statements/expr_context.rs` | update exhaustive expression matches and owner-boundary/plan composition without special dialogue paths |
| syntax parser/literal/dialogue tests | replace old speaker/dialogue-call expectations with public typed behavior tests in `TEST_MATRIX.md` |

## 3. HIR production owners

| Current owner/consumer | Required direct migration |
|---|---|
| `crates/arcweft-lang-hir/src/model.rs` | complete the accepted arena public replacement; add final `HirExprKind` payloads; delete `HirFlowItem::Dialogue` and `HirDialogue` |
| `src/lower_dialogue.rs` | replace sidecar/string lowering with direct expression lowering or delete the module after responsibilities move to typed expression/content lowering |
| `src/lower_flow.rs`, `src/lower.rs`, `src/id_context.rs`, `src/lower_ids.rs` | lower flow/function/closure/branch/block occurrences into one arena; typed `HirIdRef`; no speaker-derived fields |
| `src/project.rs` | retain module-preserving project view; remove old dialogue sidecar traversal/append handling |
| `src/dialogue_identity.rs` | remove speaker-derived source identity inputs; defer final line identity to AW-AH-009.4.3 |
| accepted HIR identity/source-map/scope modules | add variants/methods to original enums and inherent impls; stage source components and candidate IDs transactionally |
| HIR tests | replace syntax-clone/sidecar tests with arena, scope, source-role, poison, revision, and atomicity tests |

## 4. Sema consumers

Every exhaustive `Expr::DialogueCall`, old HIR sidecar, or ContentCall consumer
moves mechanically to typed `ExprId`/`HirExprKind` accessors. Current indexed
owners include:

- `crates/arcweft-lang-sema/src/symbols.rs`;
- `src/semantic.rs`, `src/semantic/traversal.rs`, and semantic facts;
- `src/checker/expr.rs`, `src/checker/expr/partial.rs`,
  `src/checker/expr/support.rs`, and line-plan checking;
- `src/project_index/entities.rs`, `flow_control.rs`, and `relations.rs`;
- `src/style/token_graph.rs`;
- dialogue callable/resolve/canonicalization paths and their tests.

Required end state:

- one typed `HirDialogueContentApplication` checker;
- one `HirPostfixBracket` resolver keyed by root `ExprId`;
- target Character ownership resolved from existing typed expression facts;
- coordinates read from ordered HIR records, never source;
- no speaker slug/callee string fallback;
- no project line-ID/collision decision in this cut.

## 5. Runtime-plan, verifier, compiler, CLI, LSP, and tooling consumers

Current indexed consumers that require exhaustive migration include:

- `crates/arcweft-runtime-plan/src/expr.rs`;
- `src/flow/syntax_helpers.rs`, `src/function_values.rs`, `src/line_task.rs`,
  flow/render/fx dialogue paths;
- `crates/arcweft-verify/src/lib.rs`;
- compiler HIR/project traversal and accepted-world handoff;
- `crates/arcweft-tooling/src/dialogue_content.rs` and canonicalization/export
  paths;
- `crates/arcweft-lsp` actions, inlay, cascade, navigation, and source lookup;
- `crates/arcweft-cli/src/app/bundle/view_mounts.rs` and other exhaustive
  expression/flow traversal;
- Agent REPL/test support and project tooling consumers reached by compilation.

Required end state:

- verifier/runtime-plan/codegen consume only checked executable resolution;
- tooling/LSP consume the accepted `Arc<HirProject>` and component source map;
- no layer searches source for brackets, calls, `with`, IDs, or content;
- no old speaker/string model is reconstructed for display;
- runtime wire and Cut 1 values remain unchanged.

## 6. Exact deletion inventory

Delete, rather than alias or deprecate:

```text
SpeakerLine
SpeakerLineSurface
string ContentCall / ContentCallSurface
Expr::DialogueCall
DialogueCallExpression CST kind
HirFlowItem::Dialogue
HirDialogue
HirExprKind::DialogueCall
DialogueSpeakerSlug and speaker-derived HIR application fields
collect_dialogue_call_content_ranges
find_content_bracket and equivalent delimiter/call searches
source-substring/reparse coordinate extraction
callee/speaker spelling heuristics
old dialogue-specific missing-close diagnostic owner
source-less dialogue/call constructors
old fixtures that assert removed shapes
```

`EntityRefSyntax` is also deleted after every expression/declaration caller uses
the appropriate existing `IdRef` or declaration-specific owner.

## 7. Forbidden residual state

The final tree must contain none of the following production behaviors:

- compatibility alias, deprecated variant, wrapper, shim, adapter record, or
  dual reader for the removed model;
- feature flag or parser mode that accepts the old speaker/`.say` form;
- source gate, file-spelling test, or source-text assertion standing in for
  typed behavior;
- local extension trait/free helper that works around an Arcweft-owned enum
  instead of updating its inherent implementation;
- fake zero range, display-string delimiter kind, string coordinate map, or
  source-search fallback;
- CSS or Takumi route;
- separate dialogue expression arena or Flow-only sidecar.

## 8. Deletion gate

Deletion is complete only when:

1. the original enums expose only the final variants;
2. exhaustive compiler matches cover every downstream crate;
3. compile-fail tests prove removed public types/constructors are unavailable;
4. behavior tests prove old syntax is not a dedicated mode while generic parser
   recovery remains normal;
5. workspace check/tests/clippy/format/`just verify` pass;
6. no implementation cut remains intentionally uncompilable.

Human review may use repository search as evidence, but no source-spelling scan
is accepted as a test or gate in place of compilation and typed behavior.
