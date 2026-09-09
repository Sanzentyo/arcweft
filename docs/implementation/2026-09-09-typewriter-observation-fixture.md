# Typed typewriter observation fixture — 2026-09-09

Inspected base: `5a2b46f09cbbb813859a54a8f71225d9a7e54c34`, existing `main`,
pushed and clean before this test-only cut. Supersedes the four stale Fx
metadata assertion failures in the
[expression ownership record](2026-09-09-expression-payload-ownership.md).
Final Git review resumed on 2026-09-10 with the same Rust diff and completed
validation logs; no Rust edit or additional test run was needed for resumption.

The shared native typewriter assertion now deserializes the observation through
`FxApplication` and builds its expected application with the owning builtin
definition/binding API. It selects the typed Content glyph-mask specialization,
the fixture's one-character-per-second rate, builtin delay/cursor defaults and
authored ordinal zero. The expected application carries the observation's
optional source range. Complete typed equality verifies definition identity,
parameter layout, runtime/static arguments and ordinal instead of an obsolete
function-name prefix and removed `parameters` field. Production behavior and
schemas are unchanged.

## Validation

Logs are ignored under
`.arcweft-local/validation/2026-09-09-typewriter-observation-fixture/`.

| Command | Result |
| --- | --- |
| `cargo fmt -p arcweft-cli` | Passed |
| `cargo test -p arcweft-cli --features native-capture --test check typewriter_ruby_capture_time_controls_ -- --nocapture` | 4 passed; 12.12 s. Both vertical directions, base/annotation geometry, visibility, mask and object-ID pixels were checked |
| `cargo test -p arcweft-cli --features native-capture --test check agent_observe_native::agent_observe_native_typewriter_capture_time_changes_visibility_without_relayout -- --exact --nocapture` | 1 passed; 3.17 s. Time-dependent glyph visibility without relayout |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; 4.62 s |
| `just test-workspace` | 856 passed / the same 18 callable failures in 84 reports; 31.85 s. Later workspace/CLI targets were not run after the compiler failure |
| Canonical structural audit with `--fail-on-blocking` | 95 packages, 2,250 Rust files, 309 review triggers, 0 blocking violations; 2.29 s |
| Documentation links and `git diff --check` | Passed; 2 documents / 34 local link targets, anchors not checked |

No production Rust/API/doc example changed. Separate workspace check, doctest,
MCP and exhaustive auxiliary/visual/proof Tier 2 runs are not repeated for this
fixture correction; the five directly affected native tests execute in full.
The prior image-animation sample parse failure remains unresolved.

## Structure and remaining work

`crates/arcweft-cli/tests/check/agent_observe_native/native_samples_effects.rs`
remains the native sample/effect capture integration-test owner: 5,791 → 5,803
physical LOC, 212,039 bytes, test classification and zero embedded test LOC.
Normal dependency fan-in/out is 0/53 and development fan-in/out is 0/3.
The size trigger retains a cohesion disposition: this edit replaces one shared
assertion already used by the same native time-sampling tests. It introduces
no production dependency, rendering fallback, extra test family or state
cluster. The typed builtin API replaces local identity reconstruction.

These five capture regressions are now closed. Recovered closure candidate
admission, image-animation source migration, sema 7/callable 18 failures and all
later [goal acceptance](2026-09-08-convergence-goal-plan.md) remain required.
There is no design deviation or external blocker.
