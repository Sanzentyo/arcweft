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
- a production file above 1,200 LOC contains embedded `#[cfg(test)]` tests;
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

Use exact values from the current checkout, not diff additions. Record:

- full Git commit SHA and dirty/clean state;
- path and owning crate;
- byte size and physical LOC;
- production/test/generated/benchmark/facade classification;
- embedded test LOC where applicable;
- major responsibilities; and
- dependency fan-in and fan-out when relevant.

Exclude `target/`, Git internals, vendored upstream source, generated artifacts,
and historical documentation unless the task explicitly audits them. Mark
generated source rather than mixing it into production hotspot rankings.

## Review thresholds

- production Rust file: warning above 1,200 LOC; error above 2,500 LOC;
- `lib.rs` or `main.rs`: warning above 1,000 LOC; post-split facade target at or
  below 250 LOC;
- integration-test file: warning above 2,500 LOC; error above 8,000 LOC; and
- ordinary responsibility module: preferred range 300–800 LOC.

These values trigger ownership review; they are not a reason to split a
cohesive algorithm arbitrarily. A generated table or genuinely cohesive
algorithm may be exempted with a module-level explanation and implementation
audit. An error-level exception requires an explicit repository-visible
rationale.

## Canonical command

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Use `--write docs/implementation/structure-audits/<task>` when retained report
files materially support the cut. The audit may measure sizes, responsibilities,
and structured dependency edges. It must not become a source-spelling or
file-placement gate.
