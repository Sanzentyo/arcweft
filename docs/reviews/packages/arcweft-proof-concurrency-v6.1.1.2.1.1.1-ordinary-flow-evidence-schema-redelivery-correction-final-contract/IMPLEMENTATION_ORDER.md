# Deletion-driven implementation order

The implementation is one final-model series. Private compiling checkpoints
may be committed only when they expose no second public reader. The public
authority switch is a single workspace-compiling cut.

## Cut 0 — current-owner re-intake

1. Fetch current `main`.
2. Re-read `AGENTS.md` and applicable Rust skills.
3. Compare the inspected commit in this package with current owners.
4. Reverify selected predecessor package/intake identities.
5. Inventory actual compile consumers from the current checkout. Package paths
   are descriptive evidence, not a source gate.
6. Keep the old Flow path frozen. Do not fix a defect in it before replacement.

## Cut 1 — central attached syntax

1. Extend the original `SyntaxKind`, `SyntaxRole`, classifiers, marker inventory,
   family predicates, and grammar budget in place.
2. Add the exact `FlowItemNode`, identity, signature, contract, and statement-only
   body records with private fields and checked constructors.
3. Add the context-specific ThreadExpression statement-only body owner.
4. Emit one heterogeneous clause sequence and one body-item sequence from the
   parser event transaction.
5. Replace `DialogueCallExpression` with the accepted dialogue-content
   application node.
6. Make detached Flow identity/header/body accessors and family-specific
   requires/ensures vectors unavailable. Fix immediate syntax consumers toward
   the attached owner.
7. Do not publish an attached-to-detached projection.

Checkpoint evidence: focused syntax success/recovery/limit tests, attachment
invariant tests, compile-fail private-construction tests, format, check, and
strict syntax Clippy.

## Cut 2 — final HIR, scopes, locals, and source freeze

1. Extend the original scope/local/source role/query enums in their owner
   modules.
2. Install `HirItemKind::Flow(HirFlowItem)`, the four identity states,
   `HirFlowReturn`, all nine contract variants, and the sixteen body variants.
3. Preflight all syntax/HIR limits and exact source manifests before reserving.
4. Reserve ItemId; four Flow scopes; generics/types/patterns; parameter locals;
   optional result local; clauses; body children; source rows; diagnostics and
   facts in the specified order.
5. Re-derive and compare every source role and ordered child from the immutable
   attached snapshot.
6. Publish only through the accepted HIR module transaction.
7. Delete `HirFlow`, `HirFlowItem`, `HirThread`, `HirDialogue`, raw
   `ContractClause`, `AuthoredExpr`, copied ranges, and `lower_flow` clone
   construction as consumers move.

Checkpoint evidence: exact schemas, scope/local visibility, omitted Unit,
source-query, stale/foreign, source substitution/reorder, panic/cancel,
rollback/retry, and exact/one-over tests.

## Cut 3 — project and semantic authority

1. Publish coherent Flow identities through the module-preserving accepted
   project transaction.
2. For name-only Flow, perform the maintained module-scoped public-ID derivation
   exactly once. Missing or mismatched identities create no candidate.
3. Register the exact accepted `Arc<CallableRecord>`; use the accepted nominal
   resolver for generics, parameters, result, and `where`.
4. Check contracts from typed HIR expression IDs:
   - the accepted effect catalog resolves `Effects` and `NoEffect`;
   - the existing place/effect facts validate `Reads` and `Modifies`;
   - the accepted proof/callable/project authority resolves proof references;
   - one result local is visible only in Ensures.
5. Switch semantic traversal and project index to `ItemId`, `StmtId`, `ExprId`,
   typed scopes, and typed source queries.
6. Delete syntax-AST clause readers, string effect labels, copied symbols,
   simple-name/Flow-local resolution, and source reconstruction.

Checkpoint evidence: all contract rows, checked callable identity, same-name and
ID/name collisions, multi-module, accepted generation, incremental reorder,
effect conflicts, and project rollback.

## Cut 4 — verifier, compiler, runtime-plan, formatter, LSP, CLI, and Agent

Migrate in dependency order:

1. verifier and solver facts;
2. runtime-plan Flow and line-task lowering;
3. compiler and persistent/cache facts;
4. formatter over the attached syntax snapshot;
5. LSP diagnostics, cascade, inlay, navigation, and source edits;
6. CLI check/build/bundle/runtime expectation projections;
7. Agent REPL/debug/native observation and tooling dialogue projections.

Every consumer keeps typed IDs or derived final output. No consumer stores a
parallel Flow body, string contract, copied source range, or consumer-local
side table.

## Cut 5 — public authority switch and deletion

In one unmerged, workspace-compiling series:

1. publish the final attached syntax and accepted HIR project;
2. route every source-backed compiler/project/LSP request through the same
   accepted parse/HIR generation;
3. delete detached `Flow`, `FlowItem`, `ContractClause`, `ThreadBlock`, raw
   recovery, clone-HIR, legacy SpeakerLine/ContentCall/HirDialogue,
   value-tail Flow body, source-string effect reconstruction, old source-range
   helpers, and obsolete exports/constructors;
4. update tests to typed APIs and compile-fail absence;
5. leave no adapter, alias, wrapper, deprecated export, dual reader, fallback,
   migration map, V2 API, or removed-syntax recognizer.

A compile error after deletion is migration inventory. It is fixed toward the
final owner; the deleted API is never restored to make an intermediate build
green.

## Cut 6 — validation and push gate

Run, at minimum:

```bash
cargo fmt --all -- --check
cargo test -p arcweft-lang-syntax --all-features
cargo test -p arcweft-lang-hir --all-features
cargo test -p arcweft-lang-sema --all-features
cargo test -p arcweft-verify --all-features
cargo test -p arcweft-runtime-plan --all-features
cargo test -p arcweft-compiler --all-features
cargo test -p arcweft-lsp --all-features
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
just test-tier2
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

`just test-tier2` is required because the final switch spans multiple crates,
changes a public contract, and affects runtime-plan and Agent observation.

Before push, verify:

- every `TEST_MATRIX.tsv` row has direct evidence;
- the old public types/constructors fail to compile through trybuild;
- architecture evidence uses Cargo metadata, not source text scans;
- no ZIP/request package was silently superseded;
- all failures are fixed or recorded without redefining completion.
