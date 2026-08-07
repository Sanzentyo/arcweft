# Structural audit policy

Repository structure is part of the implementation result. Compilation and
test success alone do not prove that ownership, dependency direction, and file
responsibilities are acceptable.

## Audit triggers

Run the canonical audit when any of the following applies:

- the task requests architecture, dependency, duplication, naming,
  test-structure, or maintainability review;
- a production Rust file exceeds 1,200 physical LOC or grows by more than 300
  physical LOC in one coherent cut;
- a `lib.rs` or `main.rs` exceeds 1,000 physical LOC;
- an integration-test file exceeds 2,500 physical LOC;
- a maintained production owner above 1,200 LOC contains embedded test-module
  source that may indicate test/production responsibility coupling;
- a workspace dependency, public contract, root re-export, Cargo feature, or
  crate boundary changes materially;
- one cut combines orchestration with transport, persistence, rendering,
  protocol conversion, pixel processing, or platform I/O;
- the same boundary type, identifier, payload, or conversion appears in
  multiple crates; or
- a manual field projection, statistics delta, descriptor inventory, schema
  mapping, or equivalent repeated mapping is added or extended.

Run the audit at reviewable Rust push cuts even when only warnings are expected.

## Required measurement

Use exact values from the current checkout, not diff additions. For the
300-LOC growth trigger, compare the complete physical LOC of the file at the
cut base with the complete physical LOC in the current checkout. Record:

- full Git commit SHA and dirty/clean state;
- path and owning crate;
- byte size and physical LOC;
- production/test/generated/benchmark/example/tool/facade classification;
- embedded test LOC where applicable;
- major responsibilities; and
- dependency fan-in and fan-out when relevant.

The default scanner scope is the workspace package set and dependency graph
reported by `cargo metadata`, plus repository-owned Rust tools. Exclude
`target/`, Git internals, vendored upstream source, retained audit reports, and
historical documentation unless the task explicitly audits them. Mark generated
source rather than mixing it into production hotspot rankings.

## Ownership review

`SIZE001`, `SIZE002`, and `TEST001` start an ownership review. They do not prove
a structural error. For each touched or newly crossed trigger, inspect and
record:

- the named owner and its cohesive responsibility;
- state ownership and whether unrelated state clusters have accumulated;
- dependency fan-in/fan-out and cross-layer direction;
- change coupling with unrelated concerns;
- duplicated authority, schema projection, or AST/HIR traversal;
- public API widening needed only to support physical file splitting; and
- whether tests follow the same production responsibility boundary.

The disposition is either decomposition along a real state/dependency/API/test
boundary or an explicit repository-visible cohesion justification. Existing
review triggers outside the cut do not block an unrelated cut merely because
their LOC remains above a threshold. A touched blocking dependency violation
does block the cut.

## Review thresholds

- production Rust file: ownership review above 1,200 LOC; above 2,500 LOC,
  decomposition or an explicit cohesion justification is required;
- `lib.rs` or `main.rs`: warning above 1,000 LOC; post-split facade target at or
  below 250 LOC;
- integration-test file: ownership review above 2,500 LOC; above 8,000 LOC,
  decomposition or an explicit cohesion justification is required; and
- ordinary responsibility module: preferred range 300–800 LOC.

These values trigger ownership review; they are not a reason to split a
cohesive algorithm arbitrarily. A generated table or genuinely cohesive
algorithm may be exempted with a module-level explanation and implementation
audit. LOC alone is never a blocking structural finding. Blocking requires
independent evidence such as incompatible responsibilities, dependency
direction, mixed state or I/O ownership, duplicated authority or traversal, or
test coupling that does not follow the production boundary.

The canonical tool represents `SIZE001`, `SIZE002`, and the source-layout-only
`TEST001` as typed review triggers. They do not make `--fail-on-blocking` fail.
An upper-trigger finding must still receive the ownership-review disposition
above. Blocking violations require independent structured evidence, such as a
forbidden edge in the Cargo metadata dependency graph. Do not promote a source
spelling or file location to blocking acceptance evidence.

## Canonical command

```bash
just structure-audit
just structure-audit-gate
```

The first command performs non-writing screening. The second additionally exits
nonzero when a typed blocking violation exists. LOC review triggers remain
visible in both commands without becoming numeric gates.

Use `--write docs/implementation/structure-audits/<task>` when retained report
files materially support the cut. Retained output includes file owner and
classification, embedded test LOC, Cargo dependency edges, and workspace-package
fan-in/fan-out. Responsibility, change-coupling, and duplicated-traversal
judgments remain review decisions rather than source scans. The audit must not
become a source-spelling or file-placement gate.
