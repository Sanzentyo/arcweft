# Generic Match C1 HIR topology and View callable evidence

Date: 2026-08-23 (Asia/Tokyo)

Inspected base: `44bac19c0149a3630a232a884def7ec355bc4930`

Working-tree state at implementation and validation: dirty, containing only
the uncommitted C1 production, test, accepted-design erratum, and this evidence
note. `HEAD` and `origin/main` both named the inspected base before the cut.

## Result

C1 is implemented as one deletion-driven HIR/sema cut:

- HIR owns a fallible expression-owned edge inventory for the 19 Await,
  Choice, and dialogue non-expression root families. The 14 closed role
  variants reuse `HirNestedExpressionPath`; no raw group path, copied side
  table, or semantic-layer AST was added.
- Expression, statement, pattern, body, and Thread-body child projection now
  expose canonical fallible edge APIs. The semantic-path builder uses only
  those APIs and maps ordinal failures to one
  `HirSemanticPathError::OrdinalOverflow` result.
- Declaration-root paths cover Function, Predicate, Proof, Flow, trait Impl,
  inherent Impl, and View. View parameters/defaults and source-ordered values
  are reachable; extern capability functions and Trait requirements remain
  bodyless.
- View completes the existing nonbinding callable pipeline using
  `CallableDeclarationKey::Existing`. Its retained symbol remains the sole
  scope binding, while the callable row joins the same item/module/snapshot to
  registered and checked callable facts and `ProjectCallableKind::View`.
- Checked and HIR-local transcript path grammars append collision-free tags for
  declaration-body and expression-owned steps. Existing tags were not
  renumbered.

The accepted design erratum replaces the returned raw dialogue group-path
sketch with this typed shape and records that the baseline View callable types
existed but their publication pipeline required same-cut completion.

## Design decisions and precedence

- `HirNestedExpressionPath` is the sole nested Choice/line-plan coordinate
  authority. Choice expression children and expression-owned non-expression
  children are disjoint inventories that share the same typed path vocabulary.
- Choice and dialogue worklists use reverse push plus stack pop to emit
  deterministic source-order DFS. Nested option fields, sibling groups, and
  Start/Together group kinds have golden tests.
- Choice Cancel trigger patterns and body children share the same PlanCancel
  role; the child kind and following Body step distinguish them.
- Error precedence is active recursion (`CyclicPath`), then an already retained
  path (`DuplicatePath`), then owner resolution (`UnresolvedOwner`).
- View does not gain a second name binding, View-only catalog, synthetic key,
  site ID, module scan, or retained-symbol fallback. Normal callable lookup
  therefore continues to reject the retained View binding as non-callable.

## Validation performed

### Passed

- `cargo check -p arcweft-lang-sema`
- `cargo test -p arcweft-lang-hir`: 861 passed, 8 ignored; integration,
  trybuild, and doc-test targets passed.
- `cargo test -p arcweft-lang-sema`: 265 unit tests, 11 API compile tests, 4
  nominal mismatch tests, and doc tests passed.
- `cargo clippy -p arcweft-lang-hir --all-targets --all-features --no-deps -- -D warnings`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 2,187 files,
  2,059 Rust files, 1,030,474 Rust physical LOC, 95 workspace packages, 202
  review triggers, 0 blocking violations.
- `just structure-audit-gate`: 0 blocking violations.
- Accepted-design `validate_design.rs --design-only`: 21 files, inventories
  `27/8/7/5/38/13/35/5/13`, decisions `1-7`, repository gate intentionally
  not run against the dirty implementation tree.
- Accepted-design `negative_self_tests.rs --design-only`: 40 negative cases.
- Independent Sol final audit: APPROVE after the fallible-edge and complete
  declaration-root matrix corrections.

### Failed outside the C1 diff

- `cargo check --workspace --all-targets --all-features` reached the changed
  HIR/sema crates, then failed in unchanged `arcweft-player-native` code:
  two `GenerationId::new(1)` function calls used as patterns and two missing
  `TextInputFocusGeneration::get()` methods.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  failed first in unchanged `arcweft-lang-syntax` and `arcweft-core` warnings.
- Focused sema Clippy with `--no-deps` failed on 24 pre-existing warnings. The
  C1-induced `source_callable_shell` size crossing was removed by extracting
  the View projection helper; the final output contained no `items.rs` C1
  diagnostic.
- `just test-workspace` and `just test-doc` failed on the same unchanged
  `TextInputFocusGeneration::get()` errors.

The failing files have no C1 diff and were not repaired or committed as part of
this cut.

## Structural ownership review

The canonical audit reported no blocking dependency violation. Touched owners
already above review thresholds were inspected as follows:

- `arcweft-lang-sema::callable::builder` — 58,417 bytes / 1,533 LOC,
  production. It remains the single registered-callable construction owner;
  View adds one exhaustive callable family without new state or dependency
  direction.
- `arcweft-lang-sema::final_analysis::model` — 57,752 bytes / 1,964 LOC,
  production. It remains the central checked-fact vocabulary; C1 adds only two
  typed path-step variants.
- `arcweft-lang-sema::final_analysis::semantic_transcript` — 79,757 bytes /
  2,070 LOC, production. The new mapping is part of the existing sole
  transcript grammar and has no I/O or parallel model. Its tests were placed in
  the dedicated `semantic_transcript/tests.rs` responsibility rather than
  embedded in this owner. C3 is the planned transcript replacement cut, so a
  physical split that widens private APIs was not introduced in C1.
- `arcweft-lang-sema::final_analysis::tests` — 253,381 bytes / 7,345 LOC,
  integration-test owner. The added View test reuses its project-wide analysis
  fixture and stays below the 8,000 LOC upper review threshold. Role/tag unit
  tests and HIR topology tests were instead placed beside their production
  owners.

The new `owned_body_edges`, `semantic_paths/tests`, and
`semantic_transcript/tests` modules follow the production responsibility
boundaries. No public API was widened merely to split files.

## Non-goals and remaining work

- C2-C5 exact checked owners, complete transcript replacement, coverage
  matrix, and publication deletion remain unimplemented.
- The unrelated workspace/player-native and baseline Clippy failures remain
  outside this cut.
- No returned ZIP, request mirror, package archive, runtime executor, bytecode,
  or persistence format was changed.
