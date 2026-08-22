# Runtime convergence Cut 2 — ownership projections

Date: 2026-08-22
Inspected Git commit: `b25e0a86bdf09fd1fb51e317c282f3034476dafb`
Working tree during completion: dirty with the restored, staged Cut 2 WIP plus
the unstaged ownership completion changes recorded by this note. The existing
index was preserved without restaging.

## Result

- Cut: `2 — ownership projections`
- Accepted design:
  `docs/reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner/`
- Implementation result: `PASS WITH EXPLICIT FAIL-CLOSED DEVIATION`
- Production commit/push at the time of this note: not yet performed

## Performed

- Added the crate-private exhaustive `TypeKind` producer-argument ownership
  classifier with typed first-error paths, copy/snapshot dispositions, current
  core checked carriers, canonical digest validation, and AWBC snapshot
  validation.
- Classified every nested Agent builtin, array length, iterator family, map
  kind, handle state, borrow/lifetime, character-dialogue, and character
  nominal case without family wildcards.
- Reused core `RuntimeCheckedType::variant_case` authority for Option and
  Result. Added exact sequence/array constraints and recursive tuple paths.
- Added the private unary Need ownership certificate containing exact Need and
  payload semantic identities. Handle retention does not recursively require
  payload retainability; no public live Need carrier is constructible before
  Cut 5, and the public ownership summary rejects Need until that carrier cut.
- Published the opaque `CheckedOwnershipCertificate` boundary through
  `RegisteredSemanticWorld::checked_ownership`. Its private construction
  commits only exact consulted Project nominal, accepted opaque, core Agent DTO,
  or stable callable evidence rows under the accepted version-1 grammar;
  rows are deduplicated and sorted before hashing.
- Published `CheckedOwnershipLimits` with the accepted production ceilings.
  Type-node and recursion counters charge before descent with checked `u64`
  arithmetic, return the public `WorkLimit` result transactionally, and have
  exact/one-over public-boundary tests. Nominal-edge, active-nominal-depth, and
  evidence-row work are bounded by the same traversal owner. The
  `max_value_certificate_nodes`, `max_function_captures`, and
  `max_producer_arguments` fields are not charged by the Cut 2 type traversal
  and do not claim active enforcement; they are retained for their Cut 3/Cut 5
  certificate consumers.
- Changed Project nominal ownership to classify struct fields and enum payloads
  recursively in accepted declaration order before runtime schema projection.
  Affine nested members therefore reject at their exact typed ordinal, and a
  later schema failure can no longer mask the required first-error precedence.
- Moved project nominal runtime identity/schema/layout projection into final
  sema authority and made compiler lowering and runtime-plan facts consume it.
  Runtime-plan no longer derives a nominal identity from HIR declaration text.
- Removed the opaque producer copied into each `AcceptedNominalType`.
  Accepted nominal runtime evidence is rejoined from the exact accepted-world
  catalog and now uses one opaque carrier owner with private fields and typed
  getters.
- Preserved Rust enum declaration order through metadata projection,
  substitution, and digest; duplicate names still reject atomically.
- Deleted the duplicate compiler runtime type-schema projection and routed it
  through sema's canonical projection.
- Added the independently throwable structural accepted-nominal correction
  request:
  `docs/reviews/requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier-correction.md`.

## Passed

- `cargo fmt --all -- --check`
- `cargo check -p arcweft-lang-sema -p arcweft-compiler -p arcweft-runtime-plan -p arcweft-adapter-sema --all-targets --all-features`
- `cargo +nightly -Zscript tools/structure-audit.rs --root .
  --fail-on-blocking`: blocking violations 0
- `git diff --check` and `git diff --cached --check`
- `cargo test -p arcweft-core --lib`: 219 passed, 0 failed
- `cargo test -p arcweft-lang-sema --lib --all-features`: 255 passed, 0 failed
- `cargo test -p arcweft-runtime-plan -p arcweft-adapter-sema -p
  arcweft-compiler --lib --all-features`: runtime-plan 49, adapter-sema 11,
  compiler 55 passed; 0 failed
- Focused ownership tests (18 passed, 0 failed) cover primitive live/digest carriers, core
  Option/Result cases, recursive first-error paths, sequence/array length,
  successful Agent carriers, private Need evidence, public Need fail-closed
  behavior, sorted/deduplicated evidence, unrelated accepted-catalog
  invariance, opaque carrier sensitivity, typed rejection families, public
  exact/one-over work limits, and declaration-order Project nominal record and
  variant recursion.
- The standard accepted `ImageHandle` integration exercises exact catalog
  lookup, live opaque construction, canonical digest, snapshot round trip, and
  foreign-world rejection.
- Rust metadata tests cover source-order enum cases/digest and generic
  instantiation without publishing instantiated rows.

## Baseline-known lint findings

- Strict sema-only Clippy was attempted with `--no-deps -- -D warnings` and
  remains failed on 24 pre-existing or separately owned sema diagnostics. A
  path-filtered audit reports no diagnostics in `ownership.rs` or the Cut 2
  core files; the Cut 2 `nominal_schema.rs` needless borrow was removed. This
  is not reported as a clean crate-wide Clippy gate.

## Not run yet

- Workspace-wide tests were not run. Changed-crate full tests, the structural
  blocking gate, formatting, and staged/unstaged diff checks were run.
- Tier 2 Agent/MCP/native capture/visual tests are not applicable: Cut 2
  changes semantic ownership and compile-time projection only.

## Structural review

- `arcweft-lang-sema::ownership` is the sole semantic producer-argument
  classifier and current-carrier validator. Its digest/certificate summary is
  public with private construction; runtime projections and the Need
  certificate remain private until Cut 5.
- `arcweft-lang-sema::final_analysis::nominal_schema` is the one project
  nominal identity/schema/layout projection consumed by compiler and runtime
  plan.
- Accepted nominal runtime evidence remains catalog-owned; instantiated
  `TypeKind` values contain declaration identity and arguments only.
- Rust metadata retains source-backed declaration semantics and does not confer
  executable runtime ownership by itself.

## Accepted structural nominal admission deviation

The accepted ownership matrix permits structural `AcceptedNominal` success
only after an exact accepted nominal/layout/field-or-case carrier exists; it
also requires `MissingRuntimeSnapshotOwner` when any such identity is absent.
At this revision the accepted nominal catalog owns complete opaque
producer/value-class/persistence evidence, while Rust ADT metadata owns
source-backed declaration shape only. It does not own a runtime layout hash,
executable record layout, exact variant payload carrier, or snapshot restore
join. `RuntimeCheckedType` also has no anonymous-record payload predicate, and
the current schema vocabulary cannot express every Rust tuple/Result payload.

Cut 2 therefore publishes no structural `AcceptedNominal` success. The
speculative Record/Variant carrier markers and their metadata-kind join were
deleted rather than promoted into a parallel type algebra. Accepted nominals
succeed only through the exact catalog-owned opaque carrier. `ProjectNominal`
remains the implemented structural record/variant path through final sema's
canonical runtime schema and layout projection.

This is fail-closed under the matrix's missing-layout/case/field rule and a
deliberate deviation from the schema sketch that listed constructible accepted
ExactRecord/ExactVariant projections. Those two projections receive no Cut 2
implementation credit. The linked correction request must close record enum
payloads, tuple normalization, recursion, nested opaque shapes, canonical
layout hashing, compiler/AWBC lowering, and restore together before that path
is implemented.

## Remaining work and non-goals

- Cuts 3–5 remain pending.
- Cut 2 does not publish task, scheduler, adapter transaction, Need handle, or
  snapshot persistence APIs.
- Structural accepted Rust ADT ownership is an explicit non-goal until the
  linked design request is returned and accepted.
- No compatibility reader, version bump, fallback resolver, source-spelling
  lookup, copied catalog, or second checked-type algebra was added.
