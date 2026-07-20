# AW-AH-009.3 bounded native signature cache

Date: 2026-07-21

## Status

Implementation and focused validation are complete in isolated Jujutsu change
`puyrtmyz` at:

```text
D:\git\arcweft-ws-aw-ah-009-3-gap1
```

The change is rebased without conflicts onto current main commit `110253cc`
(`Stage private bound expression fragments`) and is not integrated. It closes
the separate bounded-cache cut from
`arcweft-aw-ah-009.3-character-nominal-signature-help-final-contract.zip` while
preserving the accepted-HIR lifecycle and publication rules from
`arcweft-aw-ah-009.3.2-accepted-hir-request-lifecycle-production-reconciliation-final-contract.zip`.

## Implemented contract

- Each accepted environment owns one typed signature cache. The removed
  placeholder string cache has no compatibility carrier.
- `SignatureCacheKey` contains exactly the original result identity:
  accepted generation, symbol world, symbol revision, character revision,
  character digest, accepted source identity, optional LSP version, and checked
  byte offset. Profile, module, position encoding, and presentation label are
  deliberately absent; accepted/profile/project/module pointer validation
  remains in the later lifecycle stamp gate.
- Cache values retain only final native `Help` or stable `NotApplicable`
  outcomes. Query errors, invalid positions, projection failures, cancellation,
  deadline expiry, stale stamps, and failed response enqueue never insert.
- Recovered help is reusable only under the exact same accepted source,
  recovery-bearing result, request stamp, version, and byte offset.
- Capacity is inclusively 512 entries. A `BTreeMap`-backed LRU uses checked
  access ticks, refreshes replacement/hit recency, evicts the least-recent tick,
  and breaks equal-tick ties by the lexicographically smallest full key.
- Access-clock overflow clears the cache. Insertion then restarts at access one;
  overflow during lookup becomes a miss so the ordinary query/publication path
  can repopulate it.
- Complete retained-entry representability is checked with checked
  conversions, additions, and multiplication. An unrepresentable entry is
  returned to the client but not cached.
- A poisoned cache mutex is recovered by clearing the inner cache and poison
  state. Semantic resolution continues through an ordinary miss and can
  repopulate after final validation.

## Publication and invalidation

Pre-work acquires locks in the established order:

```text
session read
-> accepted-environment read
-> request publication gate
-> signature cache
```

It validates before cache access and again around lookup. A miss releases all
four guards before the bounded sema query. Publication reacquires the same
order, performs the complete exact stamp validation, projects the semantic
outcome while that gate is held, then revalidates the complete stamp and
deadline immediately before enqueue. Only a successful enqueue may insert a
computed cache miss and transition the gate to `Finished`. Failed enqueue
leaves both cache and gate unchanged.

Lifecycle behavior is:

| Event | Cache behavior |
| --- | --- |
| production session accepted replacement, including identical facts | lifecycle publication clears the old cache during its checked callback; new generation starts empty |
| symbol world/revision or character digest/revision change | new generation/key namespace; old publication rejected |
| document open/change | cancel URI requests and evict the accepted project identity before mutating live bytes or attempting rebuild |
| failed changed-document rebuild | retain the prior accepted environment but keep the changed document's entries evicted |
| failed rebuild with unchanged accepted sources | retain the prior accepted pointer, generation, and cache atomically |
| document close | evict the accepted URI-to-source identity before unmapping |
| profile remap/workspace removal/session shutdown | cancel bound requests and clear the affected accepted caches |

The accepted project identity is used for document eviction. The live editor
snapshot identity is not substituted for it.

The low-level state replacement test deliberately retains an old reader Arc to
prove generation namespaces and final-stamp rejection. Production session
replacement additionally runs the accepted-lifecycle callback that cancels
bound requests and clears that old Arc's cache before the pointer swap.

## Position fixture migration

The broad
`entry_definition_protocol_dispatch_honors_utf8_utf16_and_utf32_positions`
fixture now uses the current manifest contract:

```toml
schema = 1

[package]
id = "org.arcweft.tests.entry-definition-position-encodings"
version = "0.1.0"

[profiles.agent]
kind = "agent"
entry = "@entry.agent.main"
source = "src/main.arcw"
```

The exact requested command now reports eight passing tests and zero failures.
No removed manifest spelling or compatibility reader was restored.

## Canonical external-metadata diagnostics

The generated-metadata hash and decode errors now retain the existing
owner-qualified `ProfileTopologyResourceId` instead of discarding it in favor
of a physical `PathBuf`. The decode variant boxes its structured codec source
to keep the public error enum below the strict Clippy large-error threshold.
The LSP projects the diagnostic resource through the typed logical path owner,
so Windows extended absolute paths cannot leak through this boundary.

Project-loader tests assert the exact workspace owner and canonical
`generated/truck.adapter.json` path for both hash and decode failures. The LSP
profile regression test retains its exact relative-resource assertion and its
absolute-path exclusions; no compatibility field, wrapper, or weakened
assertion was introduced.

## Owned files

Production ownership:

```text
crates/arcweft-lsp/src/features/signature.rs
crates/arcweft-lsp/src/profiles/accepted_project.rs
crates/arcweft-lsp/src/profiles/caches.rs
crates/arcweft-lsp/src/profiles/diagnostic.rs
crates/arcweft-lsp/src/profiles/state.rs
crates/arcweft-lsp/src/requests/executor.rs
crates/arcweft-lsp/src/requests/signature.rs
crates/arcweft-lsp/src/session.rs
crates/arcweft-lsp/src/session/signature.rs
crates/arcweft-project-loader/src/topology/loader.rs
crates/arcweft-project-loader/src/topology/model.rs
```

Focused test ownership:

```text
crates/arcweft-lsp/src/profiles/caches/tests.rs
crates/arcweft-lsp/src/profiles/state/tests.rs
crates/arcweft-lsp/src/profiles/tests.rs
crates/arcweft-lsp/src/session/signature_cache_tests.rs
crates/arcweft-lsp/src/session/tests.rs
crates/arcweft-project-loader/src/topology/tests.rs
```

The former embedded `profiles/state.rs` test module was moved to
`profiles/state/tests.rs` and expanded with the new cache contract fixtures
after the production file crossed the structural review threshold. The
production state boundary is now 569 physical lines and the child test module
is 1,109 lines.

The failed-source rebuild cache test now requires the exact typed
`ProjectSourceParse` diagnostic plus accepted pointer, generation, and cache
preservation. It no longer assigns that parse failure to the unrelated
character-catalog category or accepts an arbitrary nonempty diagnostic list.

## Structural audit

The canonical audit was run from Jujutsu change `puyrtmyz`, rebased onto
`110253cc`:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Result:

```text
files scanned: 3440
Rust files: 1792
Rust physical LOC: 825639
package manifests: 93
violations: 0 error(s), 131 warning(s)
```

The reports are under
`docs/implementation/structure-audits/aw-ah-009-3-gap2-bounded-signature-cache-2026-07-21/`.
This cut adds no Cargo dependency or feature edge. `arcweft-lsp` has fan-in 1
and fan-out 29; `arcweft-project-loader` has fan-in 2 and fan-out 20. The
project-loader public error contract changed directly to the final typed model,
with no provisional compatibility carrier.

Changed Rust file measurements:

| Path | Class | Bytes | Physical LOC | Embedded test LOC | Responsibility in this cut |
| --- | --- | ---: | ---: | ---: | --- |
| `crates/arcweft-lsp/src/features/signature.rs` | production | 8,767 | 253 | 73 | Borrowed semantic-outcome projection |
| `crates/arcweft-lsp/src/profiles/accepted_project.rs` | production | 38,842 | 1,144 | 0 | Accepted source-byte footprint authority |
| `crates/arcweft-lsp/src/profiles/caches.rs` | production | 19,028 | 623 | 0 | Typed key/value, checked size, bounded deterministic LRU |
| `crates/arcweft-lsp/src/profiles/caches/tests.rs` | unit test | 3,925 | 131 | 0 | Capacity, replacement, eviction, overflow, poison |
| `crates/arcweft-lsp/src/profiles/diagnostic.rs` | production | 12,580 | 327 | 0 | Typed logical-resource projection for profile diagnostics |
| `crates/arcweft-lsp/src/profiles/state.rs` | production | 18,400 | 569 | 0 | Accepted cache guard, eviction, test seam |
| `crates/arcweft-lsp/src/profiles/state/tests.rs` | unit test | 37,626 | 1,109 | 0 | Key isolation, rebuild/world/character/recovery evidence |
| `crates/arcweft-lsp/src/profiles/tests.rs` | unit test | 12,588 | 371 | 0 | Exact typed rebuild and relative-resource diagnostics |
| `crates/arcweft-lsp/src/requests/executor.rs` | production | 7,061 | 220 | 0 | Unlocked miss computation and final session reacquisition |
| `crates/arcweft-lsp/src/requests/signature.rs` | production | 31,276 | 868 | 85 | Cache work/result carriers and exact stamp-to-key projection |
| `crates/arcweft-lsp/src/session/signature.rs` | production | 21,090 | 522 | 0 | Lookup, unlocked query, projection revalidation, enqueue/insert publication |
| `crates/arcweft-lsp/src/session/signature_cache_tests.rs` | unit test | 23,612 | 661 | 0 | Hit/miss, cancellation, deadline, fault, enqueue, and invalidation races |
| `crates/arcweft-lsp/src/session/tests.rs` | unit test | 95,897 | 2,863 | 0 | Native hit/miss/stable-null path and migrated position fixture |
| `crates/arcweft-lsp/src/session.rs` | production | 41,799 | 1,021 | 0 | Accepted-identity document invalidation hooks |
| `crates/arcweft-project-loader/src/topology/loader.rs` | production | 39,522 | 1,008 | 0 | Preserve typed metadata resource identity through load failures |
| `crates/arcweft-project-loader/src/topology/model.rs` | production | 25,812 | 815 | 0 | Owner-qualified metadata error contract and boxed codec source |
| `crates/arcweft-project-loader/src/topology/tests.rs` | unit test | 40,910 | 1,190 | 0 | Exact metadata failure owner and logical-path evidence |

`profiles/caches.rs` grows by more than 300 lines, which is the cache's one
cohesive responsibility and remains in the preferred 300–800 line range.
`session/tests.rs` was already over the 2,500-line test warning threshold; this
cut adds one bounded native cache scenario and the small manifest migration.
The new cache-specific lifecycle matrix is kept in its own 661-line child test
module. No changed production file is above the 1,200-line warning threshold.

The largest non-generated production Rust files remain existing repository
hotspots: sema `checker/module.rs` (93,423 bytes/2,482 LOC), core
`engine/eval/calls.rs` (89,488/2,481), core `value.rs` (83,366/2,465), CLI
`toolchain_profile.rs` (75,712/2,463), bundle `container.rs` (78,366/2,393),
and runtime-plan `expr.rs` (84,530/2,382).

## Validation

Passing final or focused gates:

```text
cargo fmt --all -- --check
  passed

cargo check -p arcweft-project-loader -p arcweft-lsp --all-targets
  passed

cargo clippy -p arcweft-project-loader -p arcweft-lsp \
  --all-targets --all-features -- -D warnings
  passed

cargo test -p arcweft-project-loader generated_metadata --lib -- --nocapture
  6 passed

cargo test -p arcweft-lsp profiles::caches::tests --lib -- --nocapture
  7 passed

cargo test -p arcweft-lsp profiles::state::tests --lib -- --nocapture
  12 passed

cargo test -p arcweft-lsp signature_cache --lib -- --nocapture
  15 passed

cargo test -p arcweft-lsp signature --lib -- --nocapture
  20 passed

cargo test -p arcweft-lsp \
  session::tests::signature_help_uses_native_registered_adapter_candidate \
  --lib -- --exact --nocapture
  1 passed

cargo test -p arcweft-lsp positions --lib -- --nocapture
  8 passed; 0 failed

cargo test -p arcweft-lsp profiles::tests --lib -- --nocapture
  8 passed; 0 failed

cargo +nightly -Zscript tools/structure-audit.rs --root . --write \
  docs/implementation/structure-audits/aw-ah-009-3-gap2-bounded-signature-cache-2026-07-21
  0 errors; 131 repository-wide warnings
```

The final `110253cc` rerun included the generated-metadata loader group,
profile group, signature/session group, affected check, and strict Clippy. The
complete cache/state/position matrix also passed after the preceding
syntax-only main cut and before the final rebase.

The exact ignored root font was copied only to this isolated workspace for
validation:

```text
web/assets/noto-sans-jp-vf.ttf
SHA-256 5113756F8A3B5D01B2211025E267C50121E3B36F465B7BBAF3CDAF4C3430BFD0
9,590,844 bytes
```

Its source and destination hashes matched, and `jj status` did not include the
ignored file. It is not part of this change and must not be committed.

Workspace-wide evidence was host-limited:

```text
cargo check --workspace
  passed after supplying the exact ignored font

just test-workspace
  parallel attempt stopped during test-binary compilation because Windows
  could not mmap an arcweft-bundle rlib: OS error 1455, paging file too small

CARGO_BUILD_JOBS=1 just test-workspace
  avoided OS error 1455 and advanced into later CLI subrecipes, but remained
  incomplete when the command reached its 1,204-second tool timeout; no test
  assertion failure was emitted
```

No paging configuration was changed and the workspace suite was not retried a
third time. Root integration will run `just test-workspace` once concurrent
slices settle. Tier 2 Agent/MCP/capture tests, native visual suites, and doc
tests were not run because this cut does not touch those risk surfaces.

## Remaining work and deviations

This change does not claim the separate semantic overload-selection,
active-signature, diagnostics-truncation, or production resource-accounting
cut. It does not alter CharacterDialogue surfaces, restore removed syntax,
change result ordering, or add compatibility/source gates.

There are no cache-contract design deviations. The later accepted-HIR
lifecycle contract refines where the original key is validated and when
publication linearizes; it does not add profile/module fields to the original
cache key.
