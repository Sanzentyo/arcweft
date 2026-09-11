# Agent constructor type contracts — 2026-09-12

Base: `690a406023b1a1822bff6bad17588254b762a1b8`, equal to `origin/main`
in the existing main checkout. The 125 preserved Rust/Cargo WIP manifest rows
were rechecked; this cut changes only its own hunks in `value.rs` and
`plan/construction/lower.rs` among those rows. Their nominal/schema hunks and
all other callable/effect and nominal work remain outside the commit.

## Result and authority

The native plan checked a comparison's value against the result of `Probe<T>`;
AWBC checked only the first operand's Probe family. Consequently AWBC could
verify, for example, a Bool Probe compared with a String. Native choice-action
admission had a different defect: it retained the accepted choice identity
directly but counted only authored expressions against the one-operand ABI,
rejecting every such expression.

`RuntimeAgentConstructor` now owns one signature for its result family, fixed
operand requirements, dependent Probe-result requirement and nonempty flat
collections. Its public result/arity methods project that signature. The
native builder and AWBC program supply type-table observations through a
crate-private context; they no longer repeat constructor-specific type rules.
Type references stay in their original tables, preserving exact Probe result
identity without reification through the narrower checked-value image.

Native admission carries the accepted choice ID as its bound target operand.
It needs no synthetic String type or authored expression. Both native
evaluators and AWBC lowering already materialize that identity into the same
runtime ABI; those producers remain the consumers of the admitted expression.
The expression constructor counts the retained identity, and value
construction uses the same signature's materialized arity rule.

All six comparison constructors require their second operand's exact type to
equal the Probe result type. The broad AWBC `Dynamic` successes for constructor
operands, collection elements and destinations are deleted. This does not
change general AWBC Dynamic compatibility outside constructor admission.

`all` and `any` accept scalar Predicates and one level of Predicate sequences,
fixed arrays or tuples, matching the existing runtime collection expansion.
Nested collections and wrongly typed elements reject. Known empty tuples and
zero-length arrays contribute no elements; a call whose entire static input
is empty rejects. Empty containers may accompany a Predicate. Unsized
sequences retain execution-time cardinality checking: an empty actual sequence
traps through the existing VM runtime-error path without writing a result.
The nonempty `AgentPredicateOperands` value owner remains in place.

No codec row, tag, contract version, dependency, source syntax, feature or
public type is added. All versions remain 1. The old duplicated native and
AWBC validation helpers and the obsolete fixed-only arity helper are removed.

## Validation

Commands ran in the existing checkout with preserved WIP, not in an isolated
copy of the staged tree. Logs are under the ignored prefix
`.arcweft-local/validation/2026-09-11-effect-row-formulas/` with
`agent-signature-` filenames.

- Passed: `cargo check -p arcweft-core --all-targets --all-features`.
- Passed: `cargo test -p arcweft-core --all-features --lib --tests`, 494 tests
  (461 library plus 33 integration), `final-core-tests.log`.
- Passed after the final lint cleanup and stronger choice fixture:
  `cargo test -p arcweft-core --all-features --lib agent_`, 23 tests,
  `final-focused.log`. The choice fixture has no String type row. The eight
  new tests cover native admission and actual AWBC encode/decode/re-encode,
  verification and execution, including every comparison operation, wrong
  types, retained choice identity, collection shapes/cardinality and Dynamic.
- The first full test run failed because the new empty-sequence test expected
  a returned VM error instead of the existing typed Trap exit. The focused
  diagnostic run confirmed that exit; the test now checks the Trap and absence
  of a result write. No production VM error behavior was changed for the test.
- Passed with existing warnings: final
  `cargo clippy -p arcweft-core --all-targets --all-features`,
  `final-core-clippy.log` (122 library / 142 library-test warnings).
  The new signature warning was fixed; native semantic lowering and typed
  constructor admission now have distinct methods within the builder owner.
- Passed: changed-crate `cargo fmt`, working/staged diff checks, structural
  screening and the final blocking gate. The final gate reports 95 packages,
  2,310 Rust files, 1,277,123 physical Rust lines, 311 review triggers and zero
  blocking violations, `final-structure-gate.log`.
- Failed at compilation: workspace all-target/all-feature check and Clippy,
  `just test-workspace`, `just test-doc`, and `just test-tier2`. The first
  Tier 2 recipe is `test-slow-mcp`; subsequent Tier 2 recipes did not run.
  These commands stop at the preserved missing production nominal Variant
  layouts in Dialogue `character_dialogue/schema.rs:83` and
  runtime-accelerator `external.rs:543`. They are not successful workspace,
  source-to-runtime, doctest or Tier 2 evidence.

## Structure and remaining work

Complete current file measurements include WIP outside this cut:

| Core owner under `crates/arcweft-core/src/` | Physical lines | Bytes |
| --- | ---: | ---: |
| `value.rs` | 3,693 | 132,334 |
| `value/agent.rs` | 2,232 | 88,576 |
| `value/agent/signature.rs` | 223 | 8,682 |
| `plan/construction/lower.rs` | 5,186 | 214,996 |
| `plan/construction/lower/agent_tests.rs` | 227 | 7,241 |
| `awbc/verify/code.rs` | 3,711 | 148,067 |
| `awbc/tests.rs` | 5,015 | 179,025 |
| `awbc/tests/agent_constructors.rs` | 297 | 10,707 |

The signature is an immutable Agent-owned contract, independent of verifier
state and type-table storage. Each adapter observes its own existing type
graph; neither owns a second per-constructor catalog. The value module adds
only private visibility for those consumers. Agent materialization retains its
typed value algebra. The large native lowering and AWBC dataflow modules keep
their existing state, error and type-table responsibilities; the constructor
rules were decomposed into their actual shared owner. There are 205 embedded
test lines in Agent values and 209 in native lowering; the new tests live with
their native/AWBC owners. No facade, dependency direction or Sans-I/O boundary
changes are needed, and unrelated dispatch families are not split for LOC.

This closes deterministic constructor type admission. It does not bind the
actual Probe target and T to an active program/generation, establish complete
Agent protocol record descriptors, grant snapshot ownership, or finish nominal
schema producers. The existing
[nominal producer request](../reviews/requests/2026-09-11-lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1.1-nominal-variant-layout-producer-and-schema-closure.md)
and C3/C5 correlation/restore remain required repository work. Callable,
Match, View, RuntimePlan and scheduler/restore convergence also remain open.
No new design deviation or broader completion is claimed.
