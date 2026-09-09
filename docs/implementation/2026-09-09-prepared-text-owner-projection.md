# Prepared-text ownership in Agent observation — 2026-09-09

Inspected base: `db6537bd8af40559d0bdb6e3b21335d01201be14`, existing `main`,
pushed and clean before this cut. Supersedes the prepared-text ambiguity
recorded after the [Ruby HIR repair](2026-09-09-dialogue-source-projection.md).

## Selected ownership

The native observe fixture reached rendering but rejected
`object.dialogue.0.0.ruby.0` because two prepared-text owners matched it. The
renderer already distinguishes `CharacterDisplayName` and `Content` roles.
Observation selected Content, while capture and painter ordering matched only
the dialogue/entry prefix and also selected the character name.

`PreparedTextObservationOwner` is a private borrowed adapter context over the
existing renderer owner. It admits the Agent object namespace once: dialogue
children belong to Content, View text retains its encoded semantic ID plus
mount occurrence, and controls retain exact semantic IDs without a child
namespace. Character display names remain ordinary semantic View text.

Dialogue and View object production, capture selection and capture ordering
use this same context. The previous View-only root helper and capture-local
owner-family match are deleted. Dialogue root construction no longer rebuilds
its ID from separately passed numbers. Observation and capture share the
unique-owner selector and its typed missing/ambiguous errors, so duplicate
Content owners cannot silently publish the first match.

The prepared frame remains the renderer authority. The adapter borrows its
owner and projects transport identity; it does not create a renderer model,
mutable side catalog, alternative pixel source, or new wire field. Existing
public IDs and the [capture contract](../04-tooling/agent-observe-capture-contract.md)
are preserved. No design deviation or contract-version change is introduced.

## Validation

Logs and local captures are ignored under
`.arcweft-local/validation/2026-09-09-prepared-text-ownership/`.

| Command or evidence | Result |
| --- | --- |
| Initial `cargo check -p arcweft-cli --all-targets --features native-capture` | Passed; two now-unused root-construction parameters were subsequently deleted |
| `cargo test -p arcweft-cli --lib --features native-capture app::agent::native::prepared_text_observation -- --nocapture` | 4 passed, 26.83 s: content selection is independent of prepared order; a nearby entry and the speaker cannot satisfy a missing body; duplicate bodies fail for both observation and capture queries; View mount/ID separation remains intact |
| `just test-slow-agent-observe` | 0 passed / 1 failed, 44.39 s including rebuild. The duplicate-owner error is resolved; the test progresses through layer, object-ID, raw object and mask assertions, then fails its final Ruby URI image assertion because `content_pixels` is zero |
| Direct Ruby URI read on the rebuilt binary | Exit 0; metadata reports a 26 × 48 crop with 0 content pixels. This is reproduction of the remaining failure, not capture conformance |
| `cargo fmt --all` | Passed; final run 10.54 s |
| `cargo check --workspace --all-targets --all-features` | Passed with existing warnings, 8.87 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed; one needless borrow in the new test was removed, then the final run passed with existing warnings in 6.14 s |
| `cargo test -p arcweft-cli --lib --features native-capture --quiet` | 163 passed / 3 failed, 18.23 s. The newly observed HTTP adapter fixture failures are listed below |
| `just test-workspace` | 856 passed / the same 18 callable failures, 84 reports, 33.16 s. The recipe stopped in compiler `callable_execution`; later workspace and CLI recipe commands were not run |
| `just test-slow-mcp` | 4 passed, 20.72 s including rebuild |
| `just test-doc` | 95 reports, 8 passed, 0 failed, 35.82 s |
| Structural audit with `--fail-on-blocking` | 95 packages, 2,249 Rust files, 309 review triggers, 0 blocking violations, 2.26 s |
| Documentation and whitespace | Local link targets and `git diff --check` passed; anchors were not checked |

The first focused test build used a private import of `HitRect`; it was
corrected to its existing owning `arcweft-presentation` API. No API was widened.
The final focused tests also ran in the complete CLI library suite. All
commands ran sequentially with Cargo's normal concurrency; no job count,
stack limit or profile was changed.

This is a private ownership correction in one adapter crate. The matching
native-observe and MCP Tier 2 routes ran. Exhaustive auxiliary capture,
visual-golden and proof Tier 2 routes are not claimed; the Ruby pixel failure
remains the next capture correction. No golden or expected pixel count was
weakened or refreshed to hide that failure.

## Structure

The audit measured this cut dirty against the inspected base. All paths below
are relative to `crates/arcweft-cli/src/app/agent/`. The CLI's normal dependency
fan-in/out remains 0/53 and its development fan-in/out remains 0/3. No dependency,
Cargo feature or public facade export was added.

| Path | Classification | Base → current physical LOC | Bytes | Embedded test LOC |
| --- | --- | ---: | ---: | ---: |
| `native.rs` | Production | 334 → 333 | 15,608 | 0 |
| `native/player_observation.rs` | Production | 1,111 → 1,113 | 39,061 | 0 |
| `native/player_observation/capture.rs` | Production | 461 → 428 | 15,656 | 0 |
| `native/prepared_text_observation.rs` | Production | 1,155 → 1,140 | 39,319 | 0 |
| `native/prepared_text_observation/view.rs` | Production | 479 → 489 | 17,897 | 46 |
| `native/prepared_text_observation/owner.rs` | Production | New → 85 | 2,973 | 0 |
| `native/prepared_text_observation/owner/tests.rs` | Test | New → 67 | 2,513 | 0 |

The shared ownership admission is extracted into the observation domain; its
tests cover that boundary separately. Existing observation modules continue
to own geometry and object projection, and capture continues to own prepared
glyph selection and resource retention. The borrowed context does not move
rendering policy into a lower-level crate or mix transport with renderer state.
No touched production owner exceeds the structural LOC review trigger.
Generated metrics remain in the local validation directory.

## Remaining work

The full native observe test still fails on Ruby pixel coverage. Its resulting
PNG and metadata are retained locally for tracing the prepared-glyph coverage
and selected-capture mask path. This cut does not establish that Ruby capture
is complete.

The three HTTP adapter tests fail before request handling, in the unmodified
`plan_with_flow` helper at `server_adapter.rs:555`: `MissingFlowSchema` for
`health` or `assertion`. They are
`native_http_adapter_requires_respond_host_call_manifest`,
`native_http_adapter_routes_request_to_flow`, and
`native_http_adapter_reports_runtime_assertion_without_changing_flow_status`.
The helper pushes a typed flow seed but omits its required schema. These
failures were newly observed in this cut; a separate base-revision test run
was not performed. Their fixture migration remains required work.

The recovered closure candidate, sema 7, callable 18 and all remaining
Match/View/task-plan/nominal/scheduler acceptance stay in the active
[convergence goal](2026-09-08-convergence-goal-plan.md).
