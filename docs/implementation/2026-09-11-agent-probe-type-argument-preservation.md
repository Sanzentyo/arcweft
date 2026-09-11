# Agent Probe type argument preservation — 2026-09-11

Base: `35ba257b7505aa2e88bc05ecc708ebdf8e75b522`, equal to `origin/main`
when inspected in the existing main checkout. Callable/effect and nominal
schema/producer changes remain dirty and are preserved outside this cut.

## Result and selected authority

The core plan and normalized Agent shapes already retain `Probe<T>`, but
`RuntimeCheckedType::Agent` stored only an operational kind. Sema discarded
the checked result after classifying its ownership, core checked projection
rejected every Probe, and normalized checked projection rejected all Agent
families. AWBC already has a Probe row with a result reference, but reification
rejected it and an argument-free Leaf(Probe) row passed structural verification.

The checked type now reuses
`RuntimeAgentTypeProjection<Box<RuntimeCheckedType>>`. Its existing Probe
variant owns the recursive result; ordinary Agent leaves remain parameterless.
The owning projection exposes its existing fallible child mapper and a
`try_leaf` constructor that rejects a kind requiring a child. Serde preserves
the same typed shape, so an argument-free Probe cannot deserialize as a
checked type. No second Agent type algebra or value carrier is introduced.

Core plan projection, normalized projection, sema ownership projection, AWBC
reification and the checked-type interner retain the same result child.
Normalized shape mapping now has one generic projection function for both
plan identities and checked descendants. The duplicate operational-kind
mapper, blanket Agent rejection category and Probe-rejecting core helper are
deleted. Unsupported checked descendants still reject at the child, with the
existing `AgentProbeValue` path, instead of producing a result-erased type.

The checked semantic transcript keeps Agent tag 21 and the existing kind tag;
Probe appends its recursive result transcript. Every ordinary Agent leaf keeps
its previous transcript. AWBC reuses its existing Agent Leaf/Probe rows and
result reference, and whole-program structural verification now rejects a
malformed leaf through the same core leaf constructor, even when unused.
All contract versions remain 1; no legacy Probe reader or fallback is added.

## Validation and limitations

These commands ran with preserved WIP in the existing checkout. They are not
isolated executions of the staged tree. Logs are under the ignored prefix
`.arcweft-local/validation/2026-09-11-effect-row-formulas/`.

- Passed: core all-target/all-feature check, `agent-probe-first-core-check.log`.
- Passed: final core all-feature library/integration tests, 486 tests
  (453 + 33), `agent-probe-final-core-tests.log`.
- Passed: focused Agent projection tests before and after the verifier change,
  including actual plan projection, canonical AWBC encode/decode/re-encode and
  verification, serde round trip, distinct Bool/String result identities,
  malformed argument-free Probe, cyclic/missing/unsupported result rejection,
  and preservation of every existing Agent leaf transcript.
- Passed with existing warnings: core Clippy, `agent-probe-core-clippy.log`.
  Changed-crate formatting and ordinary/staged diff checks passed.
- Passed: structural screening and blocking gate, 95 packages, 2,307 Rust
  files, 1,276,590 physical Rust lines, 311 review triggers, zero blockers,
  `agent-probe-structure-audit.log` and `agent-probe-structure-gate.log`.
- Failed: workspace all-target/all-feature check and Clippy, and
  `just test-workspace` / `just test-doc`, corresponding `agent-probe-*` logs.
  They stop at the missing production nominal Variant layouts in Dialogue
  `character_dialogue/schema.rs:83` and runtime-accelerator `external.rs:543`.
- Failed before test execution: the direct Agent runner all-feature library
  test attempt, `agent-probe-runner-tests.log`, at the Dialogue prerequisite.
  The migrated runner fixtures, sema ownership test and normalized/interner
  tests have not executed and are not passed evidence.
- Not run: Tier 2 while workspace prerequisites remain uncompilable.

This is type-argument preservation and type-row admission. It does not prove
that a runtime Probe target returns T in the active program, seal generation
provenance, or validate complete Agent protocol record payloads. In particular,
the existing checked Agent record predicate is still only a carrier-family
check. Snapshot ownership remains governed by its existing classifier; a
checked Agent type alone does not grant SnapshotClone.

## Structure and inspected producer scope

Current complete file measurements include preserved work outside the cut:

| Owner under `crates/` | Physical lines | Bytes |
| --- | ---: | ---: |
| `arcweft-core/src/pattern.rs` | 2,911 | 106,352 |
| `arcweft-core/src/plan.rs` | 1,414 | 50,006 |
| `arcweft-core/src/plan/type_kind.rs` | 737 | 29,053 |
| `arcweft-core/src/awbc/type_projection.rs` | 538 | 21,868 |
| `arcweft-core/src/awbc/verify/structure.rs` | 2,869 | 112,663 |
| `arcweft-core/src/value/agent.rs` | 2,300 | 91,299 |
| `arcweft-core/src/awbc/tests.rs` | 5,014 | 179,001 |
| `arcweft-core/src/awbc/tests/agent_projection.rs` | 136 | 5,161 |
| `arcweft-lang-sema/src/ownership.rs` | 2,306 | 90,717 |
| `arcweft-runtime-plan/src/semantic_facts.rs` | 10,453 | 396,777 |
| `arcweft-runtime-plan/src/semantic_facts/tests.rs` | 2,947 | 106,507 |
| `arcweft-runtime-plan/src/awbc_lower/inventory.rs` | 2,165 | 87,098 |
| `arcweft-runtime-plan/src/awbc_lower/pattern.rs` | 692 | 28,345 |
| `arcweft-runtime-plan/src/awbc_lower/pattern/tests.rs` | 43 | 1,606 |
| `arcweft-agent-runner/src/tests.rs` | 3,566 | 128,997 |

Embedded test counts are 680 in pattern, 205 in core Agent values and 358 in
sema ownership. Test paths are test owners; other paths retain their existing
type, projection, verification or ownership responsibilities. The shared
projection remains core-owned, with sema/runtime-plan consumers depending
downward. No dependency, feature, facade, I/O owner or mutable catalog changes.

The producer inspection also established that the source-visible Agent field
registry is not a complete protocol record descriptor: the RAG runtime payload
uses summary, item_count, truncated and json fields outside that member enum.
BytesBase64 uses the actual two-field encoding/data record, and BinaryData is
a dedicated core Agent value. Those facts must guide the remaining complete
schema/descriptor work; a loose record predicate, copied partial member table
or fixture Bool type cannot serve as the missing producer authority.

The existing nominal producer request, C3/C5 program correlation and restore,
and all other convergence acceptance remain open. No broader schema or
producer closure is claimed by this cut.
