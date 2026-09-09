# Flow effect publication — 2026-09-09

Inspected base: `96f11004e8b4210bce6b72d985b28e2777a4b05e`, existing `main`,
pushed and clean before this cut. The working changes described here belong to
this cut. Supersedes the unresolved Flow-effect diagnosis in the
[Agent Fx fixture record](2026-09-09-agent-fx-fixture.md), not its historical
validation results. The [convergence goal](2026-09-08-convergence-goal-plan.md)
remains active.

## Authority and behavior

Semantic analysis already calculated the complete inferred Flow body effects,
but discarded them after checking authored bounds. Final checked Flow items now
publish the closed row before accepted roots and semantic transcripts are built.
Authored permissions retain their scopes and unused members. Covered unscoped
operations do not widen an authored scoped permission; inferred effects and
implicit suspension are published when not already covered. Flows remain
structural declarations outside the ordinary checked-callable catalog.

Flow-bound diagnostics now use the same selected prepared-call graph as ordinary
callable-bound diagnostics. The late checked-call reader, reconstructed stable
site map, and two-reader trait are deleted. A nonterminal curried application
does not add a dependency on the callee's latent body effects; its evaluated
arguments still contribute their effects.

The runtime-plan projection admits `RuntimeFlowFact` with the exact Flow identity
and its effects together. Core's `RuntimeExecutableBody` pairs operations with
one `RuntimeEffectSet` for both Flow roots and structured function sites. The
previous function-specific names and public raw Flow operations field are
removed. Builder, native execution, AOT, inventories, compiler, player, host,
driver, accelerator, tooling, and test consumers use the same body owner.
The Agent controller wrapper gets the closed effects of the selected ordinary
controller instance it actually invokes. Production Flow construction requires
an explicit row; it does not invent an empty default.

AWBC publishes each Flow body's effects in its signature. Returning calls still
require the callee's effects to be covered by the caller. The verifier uses the
foundational `EffectId::covers` rule, including scopes, and validates canonical
effect identities at table admission. Canonical sorted tables permit exact and
path-based binary searches without a quadratic comparison of all row members.

Static and dynamic `goto` terminate the active Flow and unwind call frames;
the target runs in its own effect scope. Static goto previously reused a
returning-call effect check. It now shares argument ABI verification while
preserving exact Flow binding, initialized arguments, arity, and type checks.
The destination's own effects and program capability admission remain checked.
The maintained [control-transfer chapter](../01-language/control-transfer-return-out-yield.md)
and [runtime verifier chapter](../02-runtime/executable-runtime-core.md) now state
this distinction. This corrects the verifier's coupling to returning calls;
it does not make Flow destination effects transitively accumulate in the old
scope. No speculative Flow/callable graph, compatibility reader, version bump,
or Agent-specific exemption is retained. The [Core overview](../02-runtime/core.md)
also reflects the shared body and the distinction between accepted Flow identity
and public labels.

## Actual validation

Logs and result JSON are under
`.arcweft-local/validation/2026-09-09-flow-effects/`. Commands ran sequentially
with ordinary Cargo concurrency and the accepted test debug profile. No further
clean, operating-system memory change, or Cargo job override was used.
Per-command test counts overlap and must not be summed as unique coverage.

| Command | Result |
| --- | --- |
| `cargo test -p arcweft-core --lib --quiet` | Passed 361/361; 11.46 seconds. |
| `cargo test -p arcweft-compiler --test flow_effects -- --nocapture` | Passed 5/5, including native/AWBC execution for inferred/authored/scoped rows, latent and terminal prefix effects, and static/dynamic transfers; 36.79 seconds. |
| `cargo test -p arcweft-agent-repl --lib --quiet` | Passed 12/12, including the previously failing synthetic-source controller case; 11.18 seconds. |
| `cargo test -p arcweft-runtime-plan --lib --quiet` | Passed 58/58; 86.65 seconds. |
| `cargo test -p arcweft-lang-sema --lib --quiet` | 760 passed / 7 existing failures; 14.66 seconds. |
| `cargo check --workspace --all-targets --all-features` | Passed; 32.53 seconds on the final Rust code. |
| `cargo clippy --workspace --all-targets --all-features` | Passed with warnings; 44.32 seconds after local lint cleanup. Shared large-error, complexity, and other existing warnings remain, including the shared large-error warning at new Flow row collection. |
| `just test-workspace` | 789 passed / 1 failed across 81 reports; 525.06 seconds. Stopped at the compiler API test's four diagnostic-format mismatches. |
| `just test-doc` | Passed: 95 reports, 8 doctests; 232.16 seconds. |
| `just test-tier2` | MCP target: 3 passed / 1 failed; 43.53 seconds. Subsequent Tier 2 targets were not run after this failure. |
| `cargo test -p arcweft-core -p arcweft-runtime-plan --lib --tests --no-fail-fast --quiet` | 469 passed / 1 failed across 12 reports; 214.11 seconds. The only failure is the runtime-plan mark-handler API test's diagnostic-format mismatch. |
| `cargo test -p arcweft-compiler --test flow_effects --test project_cache_transaction --no-fail-fast --quiet` | Passed 24/24, including identity/effect pairing through cache projection; 38.23 seconds. |
| `cargo test -p arcweft-agent-repl -p arcweft-tooling --test runtime_assertion_diagnostic --no-fail-fast --quiet` | Passed 3/3; 12.71 seconds. |
| `cargo +nightly -Zscript tools/structure-audit.rs --root . --write .arcweft-local/validation/2026-09-09-flow-effects/structure-audit --fail-on-blocking` | Passed: 95 packages, 2,239 Rust files, 309 review triggers, zero blocking violations; 6.66 seconds. Measured source/graph reports are retained locally; the relevant measurements and dispositions follow below. |
| `cargo fmt --all -- --check` | Passed; 10.33 seconds. |
| Documentation links and whitespace | All 55 relative targets across the five changed documents exist; anchors were not checked. `git diff --check` passed. |

The migration check iterations failed on old test consumers until all callers
were migrated; the final full check passes. The first Core run passed 359/360:
canonical effect admission exposed a fixture whose hard-coded string index
pointed at `main`. The fixture now interns its actual effect identity. The
first compiler regression run failed all five tests in the harness's public-ID
lookup after successful AWBC lowering. It now selects the source label's
accepted Flow and retains its exact identity. The first sema run was 759/8:
the prepared diagnostic reader omitted returned-call provenance. Joining that
origin through the same prepared graph restored the full diagnostic trace and
the 760/7 baseline without reintroducing the deleted reader.

Compiler and runtime-plan compile-fail mismatches retain the intended rejection
codes and names. The compiler output changes underline/annotation placement;
runtime-plan changes a similar-name suggestion. The
[subsequent fixture cut](2026-09-09-ui-diagnostic-format.md) updates those expected
outputs and records the workspace retry. Core's public
boundary suite passes. Later workspace targets and the CLI workspace recipe
commands were not run after the compiler API failure.

The MCP trace sample now passes the previous caller-effect check, then fails
AWBC verification with `type mismatch at pattern binding: expected type 14,
found 21`. These are artifact-local type indices, not a diagnosis of the source
type cause. The script fails compilation before it writes its trace, so this
run does not establish MCP trace-resource success. The remaining type/ABI
boundary must be investigated through the
[active callable request](../reviews/requests/2026-09-08-aw-ah-009.4.2.1.1.1-function-scheme-specialization-and-callable-value-execution.md);
the verifier's type check remains intact.

The subsequent [host-call type repair](2026-09-09-host-call-type-authority.md)
identifies and removes result-type identity erasure and preserves typed host
arguments. The Agent script then passes AWBC verification and reaches a
separate CLI observation-response admission failure. This is later evidence,
not a rewrite of the failed MCP result recorded above.

The seven sema failures remain the three contextual constructor cases and four
application-specific inferred callback-row cases. The broader known callable
execution failures and retained View/task-plan, nominal C1–C6, and
scheduler/restore obligations remain part of the convergence goal. This Flow
contract cut does not award completion credit for those boundaries.

## Ownership review

The source pin is the inspected base plus this cut's working changes. The
canonical audit found 95 workspace packages, 2,239 Rust files, 309 review
triggers, and zero blocking violations. No Cargo manifest, feature, or dependency
edge changed. Core remains Sans I/O and depends on the foundational effect
identity, never on HIR, sema, or compiler.

Core's plan/construction/seed/entry-inventory owners still admit and validate
one plan. The shared executable-body module is a real ownership extraction
from the function-site table: both structural Flow and ordinary function sites
need the same closed body contract. Function-site reservation and input/result
ABI checks remain in their existing owner. The large core verifier modules
retain separate table admission and executable dataflow responsibilities;
argument checking is shared at the call/transfer ABI boundary. No second
verifier, effect registry, or I/O state cluster was added.

Sema's analyzer/items owner closes and validates selected callable/body evidence
before publication. Its checked reader and reconstruction map are removed.
The final model owner documents the admitted Flow row; the compiler projection
and runtime-plan semantic-fact owners carry it with the existing generation
and exact identity. The small Flow fact module is the domain row, while the
existing facts aggregate still owns admission. Runtime-plan's final-flow and
AWBC inventory/flow owners lower those admitted rows and bodies through their
existing paths. This is one connected projection, not duplicated inference or
a new traversal that rebuilds source facts.

The large AWBC test module keeps shared verified-program fixtures for codec,
call, transfer, and fiber boundaries; the added tests exercise those boundaries
and reuse the existing fixtures. The new compiler integration module tests
source admission plus both executors, separate from production analysis.
Agent runner, accelerator, player, driver, and host test/fixture changes only
supply the newly explicit Flow row or read the admitted body. Their unrelated
transport, persistence, rendering, and mutable runtime state are unchanged.
The existing embedded builder, driver, host, and final-flow tests continue to
test their own admission/execution seam. These are the cohesion dispositions
for the touched upper size/test-coupling triggers; this cut does not claim a
repository-wide decomposition merely from reducing LOC.

Measured owners above 1,200 LOC and new Rust files follow. The classification
is from the canonical audit. Base/current values are whole-file physical LOC;
embedded test LOC is included in the current total. Additional test files below
the integration-test trigger are shown for context.

| Owner/path (under `crates/`) | Class | Bytes | Base → current LOC | Embedded tests |
| --- | --- | ---: | ---: | ---: |
| [arcweft-agent-runner/src/tests.rs](../../crates/arcweft-agent-runner/src/tests.rs) | test | 128347 | 3541 → 3547 | 0 |
| [arcweft-compiler/src/lower.rs](../../crates/arcweft-compiler/src/lower.rs) | production | 332142 | 7841 → 7855 | 0 |
| [arcweft-compiler/tests/flow_effects.rs](../../crates/arcweft-compiler/tests/flow_effects.rs) | test | 6732 | 0 → 177 | 0 |
| [arcweft-compiler/tests/project_cache_transaction.rs](../../crates/arcweft-compiler/tests/project_cache_transaction.rs) | test | 57436 | 1620 → 1620 | 0 |
| [arcweft-core/src/awbc/tests.rs](../../crates/arcweft-core/src/awbc/tests.rs) | test | 178603 | 4882 → 5025 | 0 |
| [arcweft-core/src/awbc/verify/code.rs](../../crates/arcweft-core/src/awbc/verify/code.rs) | production | 149486 | 3738 → 3753 | 0 |
| [arcweft-core/src/awbc/verify/structure.rs](../../crates/arcweft-core/src/awbc/verify/structure.rs) | production | 112293 | 2842 → 2861 | 0 |
| [arcweft-core/src/engine/flow.rs](../../crates/arcweft-core/src/engine/flow.rs) | production | 61440 | 1566 → 1566 | 0 |
| [arcweft-core/src/plan.rs](../../crates/arcweft-core/src/plan.rs) | production | 49394 | 1394 → 1402 | 0 |
| [arcweft-core/src/plan/construction.rs](../../crates/arcweft-core/src/plan/construction.rs) | production | 109362 | 2758 → 2756 | 256 |
| [arcweft-core/src/plan/construction/seed.rs](../../crates/arcweft-core/src/plan/construction/seed.rs) | production | 78922 | 2524 → 2526 | 0 |
| [arcweft-core/src/plan/entry_inventory.rs](../../crates/arcweft-core/src/plan/entry_inventory.rs) | production | 56225 | 1492 → 1492 | 0 |
| [arcweft-core/src/plan/executable_body.rs](../../crates/arcweft-core/src/plan/executable_body.rs) | production | 2579 | 0 → 93 | 0 |
| [arcweft-lang-sema/src/final_analysis/analyzer/items.rs](../../crates/arcweft-lang-sema/src/final_analysis/analyzer/items.rs) | production | 60713 | 1536 → 1448 | 0 |
| [arcweft-lang-sema/src/final_analysis/model.rs](../../crates/arcweft-lang-sema/src/final_analysis/model.rs) | production | 90941 | 2792 → 2794 | 0 |
| [arcweft-player-native/tests/support/windowed_live_patch_fixtures.rs](../../crates/arcweft-player-native/tests/support/windowed_live_patch_fixtures.rs) | test | 56724 | 1575 → 1580 | 0 |
| [arcweft-runtime-accelerator/src/tests.rs](../../crates/arcweft-runtime-accelerator/src/tests.rs) | test | 103881 | 2697 → 2698 | 0 |
| [arcweft-runtime-driver/src/session.rs](../../crates/arcweft-runtime-driver/src/session.rs) | production | 63018 | 1581 → 1582 | 228 |
| [arcweft-runtime-driver/tests/awbc_product_session.rs](../../crates/arcweft-runtime-driver/tests/awbc_product_session.rs) | test | 47580 | 1325 → 1327 | 0 |
| [arcweft-runtime-host/src/bundle_runner.rs](../../crates/arcweft-runtime-host/src/bundle_runner.rs) | production | 44511 | 1219 → 1220 | 484 |
| [arcweft-runtime-plan/src/awbc_lower/flow.rs](../../crates/arcweft-runtime-plan/src/awbc_lower/flow.rs) | production | 126874 | 3247 → 3248 | 0 |
| [arcweft-runtime-plan/src/awbc_lower/inventory.rs](../../crates/arcweft-runtime-plan/src/awbc_lower/inventory.rs) | production | 86960 | 2170 → 2170 | 0 |
| [arcweft-runtime-plan/src/final_flow.rs](../../crates/arcweft-runtime-plan/src/final_flow.rs) | production | 282431 | 6875 → 6896 | 374 |
| [arcweft-runtime-plan/src/semantic_facts.rs](../../crates/arcweft-runtime-plan/src/semantic_facts.rs) | production | 394981 | 10378 → 10380 | 0 |
| [arcweft-runtime-plan/src/semantic_facts/flow.rs](../../crates/arcweft-runtime-plan/src/semantic_facts/flow.rs) | production | 766 | 0 → 28 | 0 |
| [arcweft-runtime-plan/src/semantic_facts/tests.rs](../../crates/arcweft-runtime-plan/src/semantic_facts/tests.rs) | test | 103708 | 2855 → 2865 | 0 |

Current workspace dependency fan-in/fan-out for every touched crate:

| Crate | Normal in/out | Development in/out |
| --- | ---: | ---: |
| arcweft-agent-repl | 3/18 | 0/0 |
| arcweft-agent-runner | 4/4 | 1/4 |
| arcweft-cli | 0/53 | 0/3 |
| arcweft-compiler | 3/23 | 1/5 |
| arcweft-core | 29/6 | 3/6 |
| arcweft-lang-sema | 8/14 | 3/0 |
| arcweft-player-native | 1/22 | 0/5 |
| arcweft-runtime-accelerator | 3/11 | 0/0 |
| arcweft-runtime-driver | 6/13 | 2/4 |
| arcweft-runtime-host | 4/12 | 0/6 |
| arcweft-runtime-plan | 5/9 | 5/1 |
| arcweft-tooling | 3/5 | 0/1 |
