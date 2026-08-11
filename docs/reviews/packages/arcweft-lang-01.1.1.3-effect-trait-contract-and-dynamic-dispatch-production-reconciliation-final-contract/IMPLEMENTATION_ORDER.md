# Implementation order

## 0. Landing rule

This is one deletion-driven public authority switch. The numbered steps may be
reviewed as compile-clean commits in one stack, but no release/tag/main push
point may expose two callable/effect authorities. Steps 3-9 must land together
if splitting them would leave a public old/new reader pair.

At every reviewable cut:

```bash
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Focused tests precede broad gates. No source-text gate is permitted.

## 1. Characterize current typed boundaries

Add behavior/API characterization tests only:

- ordinary contract clauses and exact source ranges;
- current trait/impl signatures and static witness resolution;
- current direct Await source/effect behavior;
- current resolver candidate/work accounting;
- current LSP projection from `TypeCheckError::diagnostic`;
- current rollback behavior.

Do not add compatibility fixtures or a permanent assertion for an obsolete
spelling.

## 2. Replace contract-clause source ownership atomically

Owning files/symbols:

- `arcweft-lang-syntax/src/ast/flow.rs::ContractClause`;
- `arcweft-lang-syntax/src/ast/items.rs::{TraitMember, ImplMember}`;
- `parser/headers.rs::{parse_contract_clause, parse_contract_clauses}`;
- `parser/items.rs::{parse_trait_member, parse_impl_member,
  function_signature_source}`;
- `parser/declaration.rs::emit_contract_clauses` and the shadow function grammar.

Work:

1. Replace source-less `ContractClause` with the source-backed struct/kind.
2. Migrate every ordinary/flow consumer in the same commit.
3. Make trait/impl member parsing use the same signature/contract parser.
4. Retain exact method, clause, item, and body ranges.
5. Delete the subset shadow clause reader.

Exit: one parser/AST contract owner, all syntax tests compile, no sema reparse.

## 3. Add typed trait/impl declaration keys and checked context

Owning files/symbols:

- `arcweft-lang-hir/src/symbol/identity.rs`;
- `symbol/table.rs` and `symbol/table/publication.rs`;
- `model.rs` source attachment.

Work:

1. Add project structural trait/impl/method IDs and
   `CallableDeclarationKey` in HIR; add detached/standard declaration IDs,
   `CheckedCallableDeclaration`, `CheckedCallableContext`, `CheckedCallableId`,
   and digest ownership in sema.
2. Extend `CallableDeclarationOwner` inherent behavior in its original impl
   and add the sole `CallableDeclarationKey::owner()` inherent match there.
3. Add exact `StandardTraitCatalogVersion(1)` and the single append-order
   `StandardCallableDeclarationId` builder; increment the version on every
   installed-catalog semantic or order change.
4. Publish trait declarations and impl records during project link.
5. Publish every method through
   `ProjectDeclarationId::Callable(CallableDeclarationKey)` without a module
   value binding or parallel method map.
6. Bind project IDs to exact world/revision; assign detached unified source
   ordinals only for exact source-bound HIR and emit the exact existing
   readiness error for source-less checked-catalog requests.
7. Migrate symbol queries, import/reexport trait lookup, and project callable
   identity consumers atomically.

Exit: two same-named traits in different modules have distinct typed IDs;
stale revisions fail typed lookup.

## 4. Build final callable shells before bodies

Owning sema modules:

- `callable/catalog.rs`, `callable/schema.rs`, and new responsibility-sized
  checked-catalog submodules if required;
- `effect_contract.rs`;
- `checker/module.rs` registration order.

Work:

1. Introduce private pending-shell builder and final
   `CheckedCallableCatalog`/`CheckedCallableFacts`.
2. Register every ordinary callable, trait requirement, trait impl method, and
   inherent method before checking bodies.
3. Lower effect clauses/tail/source once.
4. Construct omitted bodyless trait contracts as closed empty with name anchor.
5. Keep final records unpublished until rows/conformance are complete.

Do not expose a second public row map. Temporary pending shells are a builder
state, not a second authority.

## 5. Migrate trait catalog to IDs only

Owning files:

- `arcweft-lang-sema/src/traits.rs`;
- `traits/builder.rs`;
- `traits/catalog.rs`;
- `traits/standard_iter.rs`.

Work:

1. Store typed trait/impl declaration IDs.
2. Add checked callable IDs to requirement/impl method records.
3. Replace string `by_name` authority with project-aware typed trait lookup.
4. Preserve original IDs through supertrait traversal.
5. Return method IDs/references from resolution instead of cloned/synthesized
   method effect owners.
6. Route programmatic standard traits through the same checked catalog with a
   typed standard-catalog version.
7. Defer effect conformance until fixed-point rows exist.

Exit: trait catalog contains no effect row/copy and no requirement-as-impl
projection.

## 6. Migrate resolver and method values

Owning files:

- `callable/identity.rs`;
- `callable/resolver.rs`;
- `callable/schema.rs`;
- `callable/facts.rs`;
- `checker/expr/member.rs`;
- `crates/arcweft-compiler/src/trait_methods.rs` call-target inputs.

Work:

1. Replace `TraitCallableId` candidate/validator payloads with checked IDs and
   conformance/witness IDs.
2. Make effect schema ID-only and query the pending/final catalog.
3. Delete the hard-coded empty method row.
4. Replace `reject_method_value_reference` for project trait/inherent methods
   with `BoundMethodValue`.
5. Preserve receiver, signature, curried groups, target ID, witness, and latent
   substituted row.
6. Make direct method calls and bound-method final application converge on one
   lowering target.
7. Keep environment/non-project method-value behavior outside this contract
   unchanged unless it already has typed target evidence.

Exit: resolver work accounting remains exactly once; no new precedence phase or
fallback.

## 7. Replace string effect identities in the existing collector

Owning files:

- `effect_model.rs`;
- `effect_collector.rs`;
- `effect_analysis.rs`;
- `checker.rs` string callable/closure constructors;
- current `TypedLoweringEvidenceKind` effect callable payloads.

Work:

1. Key current/inferred/known/program/journal maps by
   `CheckedEffectCallableId`.
2. Record call edges with checked IDs and `SourceSpan`.
3. Use typed binding lookup rather than `BTreeMap<String, ...>`.
4. Check trait impl/inherent bodies in the existing body traversal exactly once.
5. Preserve checkpoint/commit/rollback semantics.
6. Run the existing least fixed point once.

Exit: no source callable or closure effect fact uses `CallableId(String)` or
`EffectSite { path, line, column }`.

## 8. Add inherent subset operation and finalize conformance

Owning files:

- `effect_row.rs`;
- `traits/builder.rs` or a responsibility-sized conformance submodule;
- checked catalog builder.

Work:

1. Add `EffectRow::check_subset` and typed errors in `effect_row.rs`.
2. Resolve actual rows after fixed point.
3. Validate each body's own contract.
4. Build and validate every `TraitMethodConformance` with typed substitutions.
5. Freeze one immutable `CheckedCallableCatalog`.
6. Publish it in `TypeCheckReport`.

Exit: A023/E014/E015/E016 and open-row tests pass without a second body pass.

## 9. Replace diagnostics and source traces

Owning files:

- `effect_diagnostics.rs`;
- `diagnostics/effect_trace.rs`;
- `diagnostics/error.rs`;
- `arcweft-lsp/src/diagnostics.rs` only where typed payload coverage requires
  no new branch.

Work:

1. Add the exact three diagnostic variants/codes.
2. Build typed shortest traces per missing effect.
3. Make `EffectDiagnostic::diagnostic()` the sole renderer.
4. Delete `UpperBoundExceeded`/`AWF-EFX-001` and text-only trace rendering.
5. Assert CLI/LSP projection equality.

Exit: E015/E016/E022/E023 exact typed diagnostics and ranges pass; no old/new
compatibility mapping exists.

## 10. Publish project index and tooling from the same catalog

Owning files:

- `project_index.rs`;
- compiler semantic-index construction;
- CLI check diagnostics;
- LSP hover/signature/navigation/diagnostics consumers.

Work:

1. Share `Arc<CheckedCallableCatalog>` from `TypeCheckReport`.
2. Add method `ProjectCallableKind` variants and inherent `as_str` arms.
3. Store checked IDs in project callable symbols; do not store authoritative
   rows there.
4. Derive Agent/typecheck effect surfaces from catalog queries.
5. Use checked target facts for method signature help and hover.
6. Preserve source revision validation.

Because this changes a multi-crate public semantic contract and project/Agent
index path, treat the completed stack as Tier 2 risk under `AGENTS.md`.

## 11. Replace runtime trait-method identity and lookup atomically

Owning files:

- `crates/arcweft-core/src/entry/identity.rs` (`RuntimeCallableId` inherent
  digest constructor only);
- `crates/arcweft-core/src/plan.rs::RuntimeTraitMethodIdentity`;
- `crates/arcweft-runtime-plan/src/trait_methods.rs`;
- `crates/arcweft-compiler/src/trait_methods.rs`;
- directly affected iterator/static-witness evidence construction.

Work:

1. Add `CheckedCallableId::semantic_digest()` with the exact canonical encoding
   and `RuntimeCallableId::from_checked_digest()` in the owning inherent impls.
2. Replace `RuntimeTraitMethodIdentity` with implementation and optional
   requirement `RuntimeCallableId` projections.
3. Sort lower inputs by checked IDs before assigning `RuntimeTraitMethodId`.
4. Change `ForIterationEvidenceFamily` witness arms to carry exact method
   `TraitMethodConformanceId`s and return a compiler-only typed lowering index
   keyed by conformance/inherent checked IDs.
5. Pass direct runtime method IDs into iterator/static-witness evidence.
6. Delete `by_witness_method`, local-index/string identity fields,
   `format!("{:?}", self_ty)`, witness/method-name lookup, and monomorph display
   labels used as identity.
7. Update the directly owned runtime-plan schema/fingerprint in the same cut if
   its serialized shape changes. Do not add a compatibility decoder.

Exit: runtime execution consumes direct plan-local IDs; it cannot select a
method or row by trait/method strings or local sema indices.

## 12. Delete all obsolete paths before landing

Delete in the same authority switch:

- `TraitCallableId` and constructors/accessors;
- trait-name/local-impl-index candidate identity;
- resolver-created empty method row;
- `CallableEffectSchema::Project.declared` row copy;
- synthesized requirement `TraitMethodImpl` projection;
- source callable/closure `CallableId(String)` paths;
- text/path/line/column `EffectSite` for source diagnostics;
- project trait/inherent `reject_method_value_reference` path;
- generic `UpperBoundExceeded` and `AWF-EFX-001`;
- duplicate contract parser branches;
- local-index/string fields in `RuntimeTraitMethodIdentity`;
- `RuntimeTraitMethodInventory::by_witness_method`;
- compiler witness-plus-method-name runtime lookup; and
- any tests whose only evidence is source spelling/file location.

Compile-fail/API/behavior tests in `TEST_MATRIX.md` prove deletion.

## 13. Final gates

Run, in order:

```bash
cargo fmt --all --check
cargo check -p arcweft-lang-syntax --all-targets --all-features
cargo check -p arcweft-lang-hir --all-targets --all-features
cargo check -p arcweft-lang-sema --all-targets --all-features
cargo test -p arcweft-lang-syntax
cargo test -p arcweft-lang-hir
cargo test -p arcweft-lang-sema
cargo test -p arcweft-core
cargo test -p arcweft-runtime-plan
cargo test -p arcweft-compiler
cargo test -p arcweft-lsp
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
cargo +nightly -Zscript tools/structure-audit.rs --root .
just test-tier2
```

Use the actual workspace crate names/checked-in just targets at implementation
time; if a listed package name differs, invoke the owning workspace member by
its manifest name without changing the semantic gate. Record any directly
relevant additional compiler/CLI focused target. Do not run a source gate.
