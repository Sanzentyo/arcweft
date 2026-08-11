# Implementation handoff and compiling order

## 1. Fixed implementation order

### Cut 1 — validate the landed ordinary-call substrate

- Re-run the AW-AH-009.3.1 ordinary parenthesized/callback call tests at the
  implementation baseline.
- Confirm private fields, checked parser construction, exact argument ranges,
  128/32 limits, and missing-`)` recovery.
- Do not implement the superseded dialogue-specific
  `SpeakerLineSurface`/`ContentCallSurface` handoff.

**Compiling frontier:** independently mergeable only if no final dialogue public
replacement has started.

### Cut 2 — introduce private final substrate

- Add private/new final indentation, surface, candidate, HIR payload, typed
  `HirIdRef`, source-role, and invariant error types.
- Add direct invariant tests where they can compile without exposing a
  compatibility API.
- Extend Arcweft-owned enums/inherent implementations directly; do not add
  extension traits.

**Compiling frontier:** may merge while all new types remain private and no dual
public model is exposed.

### Cut 3 — replace shadow grammar/CST and parser recovery

- Emit one `PostfixBracketExpression` for every postfix `[`.
- Add colon application CST at the exact owner boundary.
- Run exactly two candidate attempts.
- Implement exact close/content/indentation/plan recovery and source surfaces.
- Delete name/non-ASCII/call-shape dialogue selection.

**Public-series rule begins:** the first public `SyntaxKind` replacement through
Cut 8 is one unmerged series.

### Cut 4 — replace source AST

- Change `Expr::EntityRef` to existing `IdRef`.
- Add final Index, DialogueContentApplication, and PostfixBracket payloads.
- Delete `Expr::DialogueCall`, `SpeakerLine`, `SpeakerLineSurface`, string
  ContentCall, and source-search range reconstruction.
- Preserve ordinary `CallExpr` unchanged.

### Cut 5 — reconcile typed proof HIR

- Complete the accepted arena/function/flow public switch where still pending.
- Add the final HIR variants/payloads and typed ID carrier.
- Allocate source-backed roots, shared targets, deterministic candidate IDs,
  lexical scopes, typed line-plan children, source components, and poison.
- Delete `HirFlowItem::Dialogue`, `HirDialogue`, syntax clones, and speaker
  fields.

### Cut 6 — migrate downstream exhaustive consumers

Mechanically migrate syntax/HIR/sema/runtime-plan/verify/compiler/LSP/CLI/
tooling/test consumers to final accessors. Add typed sema postfix resolution
and executable gating. Preserve Cut 1 runtime/domain and existing resolver
policy.

### Cut 7 — delete all obsolete paths

Delete old variants, direct constructors, source reconstruction helpers,
spelling heuristics, obsolete diagnostic ownership, old fixtures, and any
migration-only private bridge. There is no compatibility residue.

### Cut 8 — restore and validate the whole workspace

Only after all consumers are migrated and old forms deleted:

- restore workspace compilation;
- run all direct tests and quality commands;
- inspect dependency direction and public API;
- record exact commands/results in the implementation note;
- commit/merge the public direct replacement.

A knowingly uncompilable syntax/HIR public cut is not a reviewable push cut.

## 2. File/frontier order

Recommended edit order inside the unmerged series follows dependency direction:

```text
arcweft-lang-syntax kinds/attachments/surfaces
arcweft-lang-syntax parser and AST
arcweft-lang-hir identity/source-map/expr/line-plan/lowering
arcweft-lang-sema typed checking/resolution/project index
arcweft-runtime-plan and arcweft-verify executable consumers
arcweft-compiler accepted project handoff
arcweft-tooling and arcweft-lsp source-role consumers
arcweft-cli / agent / test support
old code and fixture deletion
documentation and implementation evidence
```

## 3. Required implementation validation commands

Run from the repository root on the final implementation diff, using the
workspace's exact toolchain:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax
cargo test -p arcweft-lang-hir
cargo test -p arcweft-lang-sema
cargo test -p arcweft-runtime-plan
cargo test -p arcweft-verify
cargo test -p arcweft-tooling
cargo test -p arcweft-lsp
cargo test -p arcweft-compiler
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just verify
```

Also run the repository's existing compile-fail/public-API suites and the exact
100 tests in `TEST_MATRIX.md`. If a listed package name is split by the accepted
proof-HIR implementation, run the corresponding owning crate target; do not
skip the behavior.

These commands are prescribed, not claimed as completed by this design-only
artifact.

## 4. Implementation notes required in the repository

The implementation change must record:

- final Git commit and printable Jujutsu change;
- exact test/command results;
- any current-main path movement while retaining the type/behavior owner;
- public direct-replacement boundary and proof no broken push cut occurred;
- dependency graph and structural audit result;
- confirmation that Cut 1 runtime/domain schemas and ordinary call behavior did
  not change;
- confirmation that no compatibility, source-search, CSS, or Takumi path was
  added.

## 5. Stop conditions

Stop and restructure the implementation series rather than merge when any of
these holds:

- more than two postfix interpretations are retained;
- parser/CST selection uses a name, Character fact, callable fact, `.say`, or
  source substring;
- a dialogue-only call AST/parser appears;
- HIR retains syntax `Expr`, `AuthoredExpr`, `LinePlan`, raw strings, or source
  ranges as semantic authority;
- callable bodies cannot contain the same application `ExprId` kind as Flow;
- a recovered/ambiguous unresolved node reaches runtime-plan/verifier/codegen;
- relative IDs are reconstructed from source;
- old and new public AST/HIR readers coexist;
- workspace compilation is intentionally deferred past a public push cut.
