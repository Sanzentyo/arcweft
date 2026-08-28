# Source evidence

Inspected on 2026-08-29 in `D:\git\arcweft`, branch `main`.

```text
HEAD        163a3b0da9fdcd5524ffeca8b055d774d53008e2
origin/main 163a3b0da9fdcd5524ffeca8b055d774d53008e2
request     43,679 bytes
request SHA-256 935f1df409c074979027618427dea985675f3e0c2c598e537fedda59b998f46a
```

The checkout contained a large user-owned dirty worktree. This task did not
reset, move, edit, stage, or otherwise normalize it. Relevant evaluated-effect
WIP was inspected as current implementation evidence; committed `main`, stable
documentation, and accepted contracts remained the design authority.
`machine/source_inventory.json` records exact inspected HEAD/worktree blob
identities and the validator rechecks them in full mode.

## Current source findings

### Syntax and final HIR

- `crates/arcweft-lang-syntax/src/attachment/trigger.rs` uses one
  `AttachedPatternTrigger` shape for Input/Event/Mark/Select/Task/Scope.
- `crates/arcweft-lang-syntax/src/parser/statement.rs` strips a trailing `?`
  from Select Bind source and creates `propagates_error`.
- `crates/arcweft-lang-hir/src/stmt.rs` owns exactly 35 `HirStmtKind` variants
  in tag order `0x0700..0x0722`; it still owns `HirTriggerPattern` and a raw
  `HirUnsafeAudit` identity.
- `crates/arcweft-lang-hir/src/stmt/thread.rs` and the syntax/thread projection
  retain `propagates_error`.
- `crates/arcweft-lang-hir/src/stmt/child_edges.rs` has 27 direct statement
  child roles and 13 statement body roles. Mark currently follows the pattern
  child path; Select roles already carry branch indices.
- `crates/arcweft-lang-hir/src/dialogue_application/rich_text.rs` has tag
  payloads but no content-owned typed mark catalog.
- Both `final_lowering/statement_lowering.rs` and dialogue candidate block
  lowering project unsafe statements, so one shared typed ID helper is needed.

### Registration, type, Entry, and analyzer owners

- `crates/arcweft-lang-sema/src/registration/model.rs` has no standard
  statement-ingress record in `RegisteredTypeCheckEnv`.
- `registration/environment_input.rs` provides typed source-backed projection
  nodes but no task/scope/frame role publication.
- `types.rs::EntityKind` has typed `Input` and `ChoiceOption` plus
  `Other(String)`; it has no acceptable task/scope/frame owner.
- `types.rs::TypeKind` has no task/scope/frame ingress atom. Its canonical
  digest match in `types/digest.rs` currently uses outer tags through `87`, so
  this design assigns `StatementIngress` tag `88`.
- `entry/checker.rs::PreparedEntrySemanticAuthority` already borrows the exact
  type/item/call/callable/runtime-nominal facts and exposes `ty(TypeId)`; it is
  the legitimate private Event type source.
- Current analyzer order resolves/seeds statement patterns without Entry-root
  context. Finishing every call first would itself be circular when a call
  depends on an Event-bound local. A typed Entry-seeded declaration worklist,
  short-lived selector borrows, and a completed-graph recheck are therefore
  required; lexical guessing is not constructible for shared/recursive Flows.
- `final_project/semantic_paths.rs` owns typed topology/path information and is
  the legitimate owner for a new enclosing-Choice lifecycle query.

### Current dirty evaluated-effect WIP

The worktree versions of `final_analysis/model.rs`, `analyzer.rs`,
`analyzer/statements.rs`, new `analyzer/evaluated_effects.rs`, compiler lower,
and runtime semantic facts already establish the selected evaluated-effect
direction:

- `CheckedEvaluatedEffectOperation` is a closed operation algebra;
- operands retain a sealed `CheckedCallExecutionSource` and exact `TypeKind`;
- open arguments use typed `OpenArgumentId`;
- application sites are typed;
- compiler/runtime-plan projection has typed effect rows.

The new statement contract reuses those shapes. It does not redesign them and
does not repair the old `CheckedStatementRole` integration scheduled for direct
replacement.

### Checked statements, marks, runtime, and unsafe audits

- Worktree `final_analysis/model.rs` still publishes sparse
  `CheckedStatementRole` and `CheckedStatement { effects, role }`.
- `checked_rich_text/model.rs::CheckedRichTextAction::Marker` carries
  `PublicId`.
- prepared/final line plans own mark and handler slices; sema expression
  analysis recursively collects handlers.
- compiler `lower.rs` enumerates line-plan marks and copies mark handlers.
- runtime-plan `RuntimeDialogueApplication` retains `mark_handlers` and final
  line-plan lowering reads them.
- core/AWBC/content/line-task already use typed `RuntimeDialogueMarkId`; these
  lower typed owners should remain.
- core still has legacy `RuntimeWaitTarget::Mark(String)`. Current final-flow
  statement lowering reaches its unsupported-family error for unadmitted Wait
  forms; there is no checked mark-wait success path to preserve. Executable
  rejection is therefore the coherent current cut.
- `arcweft-id/src/unsafe_audit.rs` already owns `UnsafeAuditId` and
  `AcceptedUnsafeAuditSemanticId` under a version-one domain.
- verifier currently consults HIR/source unsafe identity; it must migrate to
  checked payload and typed children.

## Closed search inventories

AST/source inspection established:

| Inventory | Count | Ordered variants/roles relevant to this contract |
| --- | ---: | --- |
| `HirStmtKind` | 35 | Assertion, Let, Assign, LetElse, LetChoice, LetScope, LetActionReceive, Return, Out, Goto, DeferBlock, Defer, Yield, Signal, LifetimeSet, Wait, On, UnsafeLifetime, Choice, If, IfLet, Match, While, WhileLet, For, Close, Select, SourceLocale, Scope, Include, Break, Continue, Expression, ProofCall, Error |
| `HirStatementChildRole` | 27 | exact order and tags in `MARK_COORDINATE_AND_TRANSCRIPT.md` |
| `HirStatementBodyRole` | 13 | LetElse, Defer, On, UnsafeLifetime, Then, Else, MatchArm, While, WhileLet, For, SelectBranch, SourceLocale, Scope |
| `CheckedSelectResolution` | 5 | Method, DialogueView, AgentField, ProgressField, Field |
| `ViewSpecifiedValue` | 26 | Token through Resource in current source |
| current `CheckedRichTextAction` | 9 | eight non-marker families plus Marker |

The accepted generic-Match predecessor's seven expression-select resolutions
and 27 style values are stale. Later accepted source removed Tuple/Record
expression-select rows and one style row. This statement design does not import
or edit those unrelated grammars. It uses the five/26 inventories only as a
precedence guard so statement `CheckedSelectStatement` cannot be confused with
expression `CheckedSelectResolution`.

## Selected blob evidence

| Mode | Git blob | Path |
| --- | --- | --- |
| HEAD | `4cdc56d15893160d07578e59be1366aa4854f23e` | `crates/arcweft-lang-syntax/src/attachment/trigger.rs` |
| HEAD | `2c6d5140e25288e60f39bb532758cdc44755d864` | `crates/arcweft-lang-syntax/src/parser/statement.rs` |
| HEAD | `500b32d54d2e8257c97fc76d51565962a480b39d` | `crates/arcweft-lang-hir/src/dialogue_application/rich_text.rs` |
| HEAD | `e80032216df860ee57935086bb6b9c45695eb138` | `crates/arcweft-lang-hir/src/stmt.rs` |
| HEAD | `584610197af07e6d09d35f9829a3bc06abdc464a` | `crates/arcweft-lang-hir/src/stmt/child_edges.rs` |
| HEAD | `743d1b2aeac4d2c7f2660faff9dfa9c0d6576c49` | `crates/arcweft-lang-hir/src/final_project/semantic_paths.rs` |
| HEAD | `c8a27b3f49cabac0df5ffcf7883f2aa4d299de9d` | `crates/arcweft-lang-sema/src/types.rs` |
| HEAD | `a388968d5ead228540bba10e8e6fdbb12e0e2308` | `crates/arcweft-lang-sema/src/types/digest.rs` |
| HEAD | `b40771706e7228b14e35088bcd9b38c4cb04b8ad` | `crates/arcweft-lang-sema/src/registration/model.rs` |
| HEAD | `ab967ce26782e0123fdb33bf5318e78a8fd7fdf7` | `crates/arcweft-lang-sema/src/entry/checker.rs` |
| WORKTREE | `e1cb76e85a20fb3f767a869edb8b64844fb037fb` | `crates/arcweft-lang-sema/src/final_analysis/model.rs` |
| WORKTREE | `6211a0ea391b855370bf0b0d31be97bf03c7c500` | `crates/arcweft-lang-sema/src/final_analysis/analyzer/statements.rs` |
| WORKTREE untracked | `a22ed08c5d57a0c31398f4bf62a491ee8e03e1af` | `crates/arcweft-lang-sema/src/final_analysis/analyzer/evaluated_effects.rs` |
| HEAD | `0a9800554a81e0e5d84f0f321ac2f9ca83e34ae8` | `crates/arcweft-lang-sema/src/semantic_coordinate.rs` |
| HEAD | `be512e5941d9d3a4efe3216c1eb9c60076aa2aab` | `crates/arcweft-lang-sema/src/checked_rich_text/model.rs` |
| WORKTREE | `4f1e57d31b3f3e3eef813ad0d9284eb74dcbdc8c` | `crates/arcweft-compiler/src/lower.rs` |
| WORKTREE | `56acb51a22eb52e04b8eca4e3d06007f4c515641` | `crates/arcweft-runtime-plan/src/semantic_facts.rs` |
| HEAD | `6c5fc59bb99111cacee0ef2bd4c1c4567c260acc` | `crates/arcweft-core/src/line_task.rs` |
| HEAD | `c8d39d8f90f8bf13dd0b0e30f645762f5286b2a4` | `crates/arcweft-id/src/unsafe_audit.rs` |
| HEAD | `5b87201912bae1cf5821a8b615cd2a8d734963ed` | `crates/arcweft-verify/src/lib.rs` |
| HEAD | `7160ec8d764b94f4209e10a7fe7e347de2bc48b6` | `crates/arcweft-view/src/style/value.rs` |

The complete machine inventory distinguishes HEAD-equal, modified worktree,
and untracked worktree files. Blob identities are evidence of the inspected
cut, not a production semantic acceptance mechanism.
