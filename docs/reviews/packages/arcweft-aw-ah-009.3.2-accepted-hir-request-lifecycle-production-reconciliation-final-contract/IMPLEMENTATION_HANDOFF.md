# Implementation handoff

## 1. Preconditions

Implement from an Arcweft checkout at inspected `main` `8984661d5679efccf7a16255f921530cd0b7cacc` or a reviewed descendant. Read the then-current root `AGENTS.md` before editing. Apply the cuts below in order; each cut must compile before the next begins.

AW-AH-009.3.1 may be implemented in parallel in another worktree, but cut 6 below must wait for its exact authored-call carrier. This ordering dependency contains no open design choice in AW-AH-009.3.2.

## 2. Cut 1 - accepted HIR/source/module registry

### Files

- `crates/arcweft-launch/src/model.rs`
- `crates/arcweft-lang-hir/src/model.rs`
- `crates/arcweft-lang-hir/src/project.rs`
- all direct `HirProjectModule::new` call sites
- `crates/arcweft-project-loader/src/project.rs` and a focused limits module
- new `crates/arcweft-lsp/src/uri_key.rs`
- new `crates/arcweft-lsp/src/profiles/accepted_project.rs`
- `crates/arcweft-lsp/src/profiles/environment.rs`

### Work

1. Add `Hash` directly to the existing `ProfileId` derive list and use that owner in `AcceptedProfileKey`; add no LSP-local profile string wrapper.
2. Add inherent `HirModule::source_document`.
3. Replace `HirProjectModule::new` with `try_new`; update all callers and tests in the same cut; leave no wrapper.
4. Add typed `ProjectLoadLimits` and bounded source enumeration/read. LSP supplies 4,096 documents and 8,388,608 aggregate bytes. Use checked arithmetic and maximum + 1 evidence.
5. Add `LspUriKey` and migrate accepted-source construction for URI keys.
6. Add strict accepted source duplicate checks, `AcceptedProjectFootprint`, `AcceptedModuleKey`, typed lookup errors/methods, and `AcceptedProjectSnapshot::try_new`.
7. Wrap the one assembled `HirProject` in `Arc` before registration, borrow it for registration, and retain it in the snapshot.
8. Add direct tests for root/non-root/declaration-free modules, duplicate identity/URI/source-to-module mapping, exact HIR text/identity, dependency/generated source behavior, exact/one-over limits, and overflow.

### Gate

```bash
CARGO_INCREMENTAL=0 cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo test -p arcweft-launch --all-targets
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-launch --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-hir --all-targets
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-hir --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo test -p arcweft-project-loader --all-targets
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-project-loader --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp accepted_project --lib
```

## 3. Cut 2 - atomic accepted publication

### Files

- new `crates/arcweft-lsp/src/profiles/state.rs`
- new `crates/arcweft-lsp/src/profiles/caches.rs`
- `crates/arcweft-lsp/src/profiles.rs`
- migrate/delete `crates/arcweft-lsp/src/profiles/cache.rs`
- `crates/arcweft-lsp/src/profiles/environment.rs`
- `crates/arcweft-lsp/src/features/character_definition.rs` (mechanical import/accessor migration only)
- `crates/arcweft-lsp/src/session.rs`

### Work

1. Move candidate/environment/generation/lifecycle state to `profiles/state.rs`.
2. Move semantic cache implementations to `profiles/caches.rs`.
3. Change candidate/environment fields to one `Arc<AcceptedProjectSnapshot>`, remove the independently published source registry Arc, delete `AcceptedProfileEnvironment::sources()`, and migrate all callers to `project().sources()`.
4. Mechanically rehome `features/character_definition.rs` imports from the deleted `profiles::cache` module; do not redesign that feature's separate query in this cut.
5. Implement strict candidate/world/project/overlay cross-validation.
6. Implement checked generation replacement and fresh cache namespace.
7. Implement `try_from_unchanged_project` for identical bytes/new version; it may clone only exact world/project Arcs and must perform no parse/lower.
8. Recompute exact open-overlay coverage under session write lock before swap.
9. Delete `profiles/cache.rs`; do not retain a re-export module.

### Gate

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp profiles --lib
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lsp --lib --all-features -- -D warnings
```

## 4. Cut 3 - URI/source/module/HIR acquisition

### Files

- `crates/arcweft-lsp/src/documents.rs`
- `crates/arcweft-lsp/src/session.rs`
- `crates/arcweft-lsp/src/features/character_definition.rs` (typed URI/source access only)
- new `crates/arcweft-lsp/src/requests/signature.rs`
- new `crates/arcweft-lsp/src/session/signature.rs`

### Work

1. Migrate every document/profile/overlay/accepted URI map to `LspUriKey`; remove String-key accessors.
2. Require exact `i32` versions for open snapshots/overlay entries.
3. Implement `AcceptedDocumentHirLease` with typed, non-panicking HIR lookup.
4. Implement the 14-step `prepare_signature_request` acquisition sequence.
5. Implement all acquisition errors and exact LSP disposition: null/not-applicable, `ContentModified`, `RequestCancelled`, `ServerCancelled`, `RequestFailed`, or invariant `InternalError`.
6. Update existing character-definition source/URI access to typed keys and `project().sources()` without adding a signature fallback or changing its independent result semantics.
7. Add pointer/value identity tests, including generated/dependency/no-module cases.

### Gate

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp signature_acquisition --lib
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lsp --lib --all-features -- -D warnings
```

## 5. Cut 4 - cancellation, deadline, executor, and final stamp

### Files

- new `crates/arcweft-lsp/src/requests.rs`
- new `crates/arcweft-lsp/src/requests/control.rs`
- new `crates/arcweft-lsp/src/requests/registry.rs`
- new `crates/arcweft-lsp/src/requests/executor.rs`
- `crates/arcweft-lsp/src/requests/signature.rs`
- `crates/arcweft-lsp/src/server.rs`
- `crates/arcweft-lsp/src/session.rs`

### Work

1. Add `SignatureRequestBinding` with typed URI/workspace/document plus weak state/environment references, then `RequestControl`, the single `AtomicBool`, cancellation reason, publication gate, and fixed 250 ms deadline.
2. Add `RequestRegistry` with exact active limit 32, duplicate-ID rejection, weak deadline registrations, exact token removal, and one scheduler thread.
3. Add `SignatureRequestRuntime` owning the registry, exact `Mutex<VecDeque<_>> + Condvar` FIFO executor, response sender, and four workers; queue close drains queued guards outside the lock, and every job uses `catch_unwind` so panic cleanup and one internal-error response are deterministic.
4. Change `run_connection` to share the session through `Arc<RwLock<_>>`, prepare signature requests under read lock, queue them, and continue reading notifications.
5. Route `$/cancelRequest` directly to the registry. Delete the session `BTreeSet<RequestId>` and all cancelled-ID tombstone behavior.
6. Add `LspProfileState::accepted_read/accepted_write`, the exact `SignatureRequestStamp`, and one validator used before any cache lookup/query construction, before cache-hit return, and post-compute publication.
7. Use `Arc<HirProject>` pointer plus `AcceptedModuleKey` as the complete HIR identity; add no borrowed/raw HIR pointer to the stamp.
8. Enforce lock order: session, profile state, control gate, cache. Release all four guards before long sema work and reacquire them for final validation; release registry/scheduler locks before control cancellation.
9. Enqueue the final response under session/profile/gate after validation; only then insert a cacheable computed result and mark `Finished`. A failed enqueue inserts nothing.
10. Add deterministic concurrency tests with barriers/hooks, not sleeps, for cancellation, deadline, every stamp field, response-enqueue/publication, panic cleanup, and publication races.

### Gate

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp requests --lib
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp signature_request_lifecycle --lib
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings
```

## 6. Cut 5 - direct invalidation hooks

### Files

- new `crates/arcweft-lsp/src/session/lifecycle.rs`
- `crates/arcweft-lsp/src/session.rs`
- `crates/arcweft-lsp/src/profiles/caches.rs`
- `crates/arcweft-lsp/src/profiles/state.rs`
- `crates/arcweft-lsp/src/server.rs`

### Work

1. Add synchronous `didOpen` metadata publication or pending-rebuild state, then document-change cancellation/invalidation before rebuild.
2. Add exact document-close orchestration and disk/remaining-overlay rebuild scheduling; closed URIs cannot use historical overlay entries.
3. Add workspace-removal orchestration with unique profile-state pointer handling.
4. Add accepted replacement cancellation, old-cache clearing, checked swap, and empty new cache.
5. Add observable failed-replacement no-op with expected-current pointer check.
6. Add session `begin_shutdown`, then server-owned `SignatureRequestRuntime::shutdown` after releasing the session lock: queue drain, worker join, scheduler join, empty-registry assertion.
7. Prove old Arc readers remain memory-safe and cannot publish after replacement.

### Gate

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp invalidation --lib
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp shutdown --lib
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings
```

## 7. Cut 6 - connect the AW-AH-009.3 sema query

This cut starts only after AW-AH-009.3.1 has landed. Do not alter its authored call/range carrier.

1. Convert the protocol position through the existing exact `LineIndex`.
2. Build the 009.3.1 carrier from accepted document/HIR ranges.
3. Run the pre-work gate/deadline/stamp check, then pass lease document, lease HIR, lease registered world, exact call carrier, and `RequestControl::cancellation_flag()` to the original `SignatureQuery::try_new`.
4. Preserve the original result construction, deterministic ordering, cache key, cache limits, and error semantics.
5. Use the shared validator before cache lookup/query construction, before cache-hit return, and post-computation response/cache publication.
6. Delete the word-at-position and `arcweft_verify_lsp` signature fallback in the same compiling cut. Do not leave an alias, feature flag, or secondary successful path.
7. Do not parse a source substring on acquisition failure or cache miss.

### Gate

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema signature --lib
CARGO_INCREMENTAL=0 cargo test -p arcweft-lsp signature --lib
CARGO_INCREMENTAL=0 cargo clippy -p arcweft-lang-sema -p arcweft-lsp --all-targets --all-features -- -D warnings
```

## 8. Cut 7 - full validation and structural audit

Run from the production checkout with all required test assets present:

```bash
CARGO_INCREMENTAL=0 cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo test --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-3-2-accepted-hir-request-lifecycle
```

Also run typed dependency evidence through Cargo metadata, for example by deserializing:

```bash
cargo metadata --format-version 1 --all-features
```

The dependency test shall assert crate/package IDs and dependency edges from metadata values. It shall not inspect checked-in source text, module paths, symbol spellings, or documentation.

Record exact commands, exit codes, Rust/Cargo versions, commit, changed file metrics, and structural audit output in:

```text
docs/implementation/2026-07-16-aw-ah-009-3-2-accepted-hir-request-lifecycle.md
```

## 9. Completion deletion list

The implementation is incomplete until all are gone:

- panicking `HirProjectModule::new`;
- String-keyed document/profile/overlay/accepted URI authorities;
- independently published accepted source registry Arc and the old environment-level `sources()` projection;
- session `cancelled: BTreeSet<RequestId>` and unknown-ID tombstones;
- synchronous signature feature execution on the message-intake thread;
- signature word-at-position/Rust-adapter fallback after sema integration;
- any cache-miss parse/lower/project build;
- any compatibility module or deprecated accessor introduced only to preserve the old internal shape.

## 10. Review checklist

- No `unsafe`, `Box::leak`, or `mem::forget`.
- No new macro or unstable feature.
- No new external dependency.
- New enums receive inherent behavior on their owner types.
- Public visibility is limited; request/control/stamp/URI key types remain crate-private.
- Every counter uses checked arithmetic and exact/one-over tests.
- Concurrency tests use deterministic synchronization.
- All changed large files are split according to `PRODUCTION_RECONCILIATION.md`.
