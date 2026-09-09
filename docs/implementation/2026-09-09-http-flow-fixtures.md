# HTTP adapter Flow fixtures — 2026-09-09

Inspected base: `970ad79ad87354cb82b5b5cd68d925ce45315049`, existing `main`,
pushed and clean before this cut. Supersedes the three HTTP fixture failures
in the [prepared-text ownership record](2026-09-09-prepared-text-owner-projection.md).

The `plan_with_flow` test helper now admits the required `RuntimeFlowSchema`
before its Flow seed. Its empty parameter list is the complete invocation
schema for these parameterless fixtures. The existing builder still validates
the final plan; no production fallback, inferred schema, compatibility reader
or adapter behavior is added. Request routing, response-manifest rejection and
runtime-assertion reporting again execute their intended assertions.

Logs are ignored under
`.arcweft-local/validation/2026-09-09-http-flow-fixtures/`.

| Command | Result |
| --- | --- |
| `cargo test -p arcweft-cli --lib --features native-capture server_adapter::tests -- --nocapture` | 3 passed |
| `cargo test -p arcweft-cli --lib --features native-capture --quiet` | 166 passed, 0 failed; 2.01 s |
| `cargo fmt -p arcweft-cli` | Passed; 0.90 s |
| `cargo clippy --workspace --all-targets --all-features` | Passed with existing warnings; 7.71 s |
| `just test-workspace` | 856 passed / the same 18 callable failures, 84 reports; 32.17 s. The recipe stopped in compiler `callable_execution`; later workspace and CLI recipe commands were not run |
| Structural audit with `--fail-on-blocking` | 95 packages, 2,249 Rust files, 309 review triggers, 0 blocking violations; 2.21 s |
| Documentation and whitespace | Local link targets and `git diff --check` passed; anchors not checked |

Only a private test import and fixture construction changed. A separate
workspace check, doctests and Tier 2 are not repeated for this test-only
correction; Clippy checked the changed test target and the complete CLI library
suite ran. Commands used sequential execution and normal Cargo concurrency.

The changed owner is `crates/arcweft-cli/src/server_adapter.rs`: 557 → 563
physical LOC, 19,180 bytes, production classification with 169 embedded test
LOC. The added lines remain inside the existing adapter test module. CLI
dependency fan-in/out remains 0/53, development fan-in/out 0/3. No manifest,
public API or contract version changes; no structural threshold was crossed.

The frame sampling/reveal defect, recovered closure candidate, sema 7,
callable 18 and remaining Match/View/task-plan/nominal/scheduler work stay in
the active [convergence goal](2026-09-08-convergence-goal-plan.md). This fixture
repair does not establish global workspace or capture conformance.
