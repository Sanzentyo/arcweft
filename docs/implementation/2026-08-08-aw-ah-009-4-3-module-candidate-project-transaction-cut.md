# AW-AH-009.4.3 module-candidate and project-transaction implementation cut

- Date: 2026-08-08
- Inspected Git base: `85063671dab898885502222b8c2d93fce16ef16d`
- Working tree: dirty with the implementation and documentation changes
  described here; this note records pre-commit evidence
- Builds on:
  `2026-07-21-aw-ah-009-4-3-source-site-line-identity-intake.md`
- Completion credit: partial AW-AH-009.4.3 Frontiers 4 and 5 plus the project
  constructor portion of Frontier 6; not package completion

## Performed

The cut adds one HIR-owned module candidate transaction and one
project-acceptance transaction. It does not add a second successful line model.

- `HirModule` now owns a bounded private `HirDialogueLineCandidates` inventory
  derived only from final typed AW-AH-009.4.2 expression, coordinate,
  source-component, scope, and item facts.
- Candidate construction retains typed Flow/callable owner, named scopes,
  source order, exact application/coordinate spans, typed ID/key origins,
  checked ordinals, diagnostics, and work/limit failures.
- Candidate construction reads the transaction's prepared arenas and slots.
  This is required because `HirModule::try_new` runs before the snapshot is
  published. The first implementation accidentally queried only published
  iterators and therefore produced an empty inventory during construction even
  though a post-publication rebuild found the line. Focused tests now fail if
  this boundary regresses.
- Synthetic postfix-bracket interpretations do not become successful line
  candidates. Only source-origin final `DialogueContentApplication` expressions
  enter the inventory.
- Module line diagnostics are structured `HirDiagnostic::LineIdentity`
  payloads with AW-CD-013 and AW-CD-020 through AW-CD-028 codes and shared
  `arcweft_source::Diagnostic` projection.
- `HirProjectBuilder` is the sole successful public project constructor. It
  owns package-qualified module keys, current database/lineage/source
  validation, deterministic module order, and atomic project dialogue-line
  acceptance. The old `HirProject::try_new` and `HirProjectError` surface was
  deleted and all workspace callers were migrated.
- `AcceptedDialogueLineInventory` owns canonical records plus indexes by typed
  `DialogueLineId`, source `ExprId`, and source order. Duplicate IDs reject the
  complete project transaction; text-key facts are not published on failure.
- AW-CD-020 collision labels use the exact explicit ID coordinate when present,
  falling back to the application span only for generated identities. A new
  compiler test exposed and corrected the initial whole-application label.
- Recovered HIR modules remain in the one project tooling view with empty line
  candidates, while `HirProject::executable_view` rejects them. Rejecting them
  in `HirProjectBuilder::finish` would have destroyed the accepted
  Proof-concurrency tooling lease and created pressure for a second project
  model. Focused compiler tests prove that recovered modules cannot reach the
  runtime plan or compile cache.
- The former free Flow symbol helper was deleted. The accepted publication
  projection is now behavior on `HirFlowIdentity`, its legitimate owner.

## Passed validation

All Cargo commands used the normal shared target and
`CARGO_BUILD_JOBS=4`. No second worktree, alternate target directory, or
parallel Cargo process was used.

- `cargo test -p arcweft-lang-hir --lib project::tests:: -- --test-threads=2`
  — 15 passed, 0 failed.
- `cargo test -p arcweft-compiler --lib project::tests::recovered_ -- --test-threads=2`
  — 2 passed, 0 failed.
- exact HIR and compiler cross-module collision tests — 1 passed in each crate;
  both retain the exact `@say.shared` primary/secondary spans.
- `cargo test -p arcweft-lang-hir --lib --all-features -- --test-threads=4`
  — final run 845 passed, 0 failed, 8 ignored.
- `cargo check --workspace --all-targets --all-features` — passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` —
  passed after extracting one oversized compiler test setup helper.
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` — 1,995 Rust
  files and 982,314 physical Rust LOC inspected; 181 review triggers and 0
  blocking violations.
- `cargo fmt --all` and `git diff --check` — passed.

## Failed and corrected validation

- The first full HIR run reported 844 passed, 1 failed, and 8 ignored. The stale
  expression-lowering fixture expected an ownerless generated dialogue site to
  leave the module clean. The final contract requires AW-CD-021. The fixture now
  asserts the structured ownerless diagnostic while continuing to validate the
  selected E33 payload and exact source roles. The final full run passed.
- The first workspace Clippy run failed only on the new compiler test's
  109-line body. Fixture construction was extracted without an allow attribute;
  the final strict run passed.

## Structural review

The new cohesive owners are below the production size trigger:

| Path | Physical LOC | Responsibility |
|---|---:|---|
| `crates/arcweft-lang-hir/src/line_identity.rs` | 319 | Typed candidate/source/diagnostic vocabulary |
| `crates/arcweft-lang-hir/src/line_identity/builder.rs` | 483 | Bounded module-local ID/key transaction |
| `crates/arcweft-lang-hir/src/line_identity/diagnostic.rs` | 403 | Structured line diagnostics and source projection |
| `crates/arcweft-lang-hir/src/line_identity/module_candidates.rs` | 388 | Final-HIR adapter and owner/scope evidence |
| `crates/arcweft-lang-hir/src/final_project/dialogue_lines.rs` | 456 | Project collision transaction and immutable indexes |
| `crates/arcweft-lang-hir/src/final_project.rs` | 634 | Sole package-qualified project builder/view |

Two touched existing owners remain above the review trigger:

- `arcweft-lang-hir/src/module.rs` (1,958 LOC) remains the immutable HIR
  snapshot owner. This cut adds only candidate attachment/publication fields
  and delegates candidate construction to the new module; moving snapshot
  invariants would split one atomic publication owner.
- `arcweft-compiler/src/project.rs` (1,598 LOC) remains the project compilation
  transaction owner. This cut replaces its HIR project-construction block and
  diagnostic projection; line construction and collision logic remain in HIR.

The audit reported no blocking dependency, facade, shared-state, build-script,
or production/test-coupling violation introduced by the cut.

## Not run

- `just test-tier2` was not run. The returned package requires it for the full
  Frontier 6 consumer replacement reaching runtime-plan, LSP, Agent, and MCP.
  Those consumers and TM-100 are not claimed by this partial cut.
- The complete returned 100-row AW-AH-009.4.3 matrix and `just test-workspace`
  were not run and are not claimed complete.

## Blocked boundary and throwable request

The returned package stores a callable source owner as
`Callable(CallableDeclarationId)`. Current accepted main instead uses
`CallableDeclarationKey::{TraitRequirement, ImplMethod}` for structural method
families. Those identities cannot be losslessly projected into
`CallableDeclarationId`, and the returned contract does not define stable
generated prefix segments for Trait requirements, trait Impl methods, or
inherent methods.

No prefix was guessed and no fallback owner was added. Full method-owner matrix
closure is blocked by the independently throwable request:

`docs/reviews/requests/2026-08-08-aw-ah-009.4.3.1-callable-key-method-owner-line-prefix-reconciliation.md`

## Remaining work and non-goals

AW-AH-009.4.3 remains incomplete. Required later work includes the returned
canonical inventory cache bytes/fingerprint and exact no-op project reuse;
typed line-reference indexing; generated/explicit rename; sema, localization,
runtime-plan, verifier, LSP, Agent/MCP/CLI consumers on the same accepted
project Arc; complete limits/one-over/property/API tests; full matrix; Tier 2;
and deletion proof.

This cut does not implement CharacterDialogue runtime wire/AWBC/save behavior,
View projection, source reparsing, a secondary LSP inventory, compatibility
aliases, dual readers, removed-syntax diagnostics, a source gate, or any
method-owner spelling not selected by a returned correction contract.

At the review cut, 39 ZIPs existed under `docs/reviews/`. The three attached
Lang-01.5.1 ZIPs in `D:/sanze/Downloads` exactly matched the retained package
copies by byte length and SHA-256; no new inbox ZIP required intake.
