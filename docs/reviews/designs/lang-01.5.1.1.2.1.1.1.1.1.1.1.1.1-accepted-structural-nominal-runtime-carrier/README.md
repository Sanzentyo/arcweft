# Accepted structural nominal runtime carrier

Sequence: `Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1`

Status: `READY_FOR_IMPLEMENTATION`

This is the repository-local accepted design for source-backed Rust ADTs at
Git commit `9a5d30d25620541c3f2975d31e04e04e3bc9514c`. It resolves the maintained
request without reusing the rejected returned package's invented runtime
crate, `AcceptedRuntimeCarrier`, per-value version, or parallel value algebra.

The selected path is one-way and deletion-driven:

1. adapter registration preserves Rust declaration order;
2. `arcweft-lang-sema` atomically joins an exact `RustAdt` nominal row with
   the matching source-backed Rust metadata row;
3. final analysis constructs one checked, reachable nominal-schema graph;
4. the compiler projects that graph into the existing core/runtime-plan
   nominal type, record-domain, and variant-domain owners;
5. AWBC lowers those existing plan owners with the existing runtime-type and
   constant tags; and
6. AWBC snapshot restore reconstructs values only through the current
   program's accepted type descriptors.

Live values remain exactly `RuntimeValue::NominalRecord` and
`RuntimeValue::Variant`. `RuntimeValue::Tuple` and `RuntimeValue::Record` are
used only for enum payload structure. The core schema graph is an inert
validation/digest input; it is consumed and discarded when the existing
`RuntimePlan` tables are sealed, so it is not a copied runtime catalog.

Normative files:

- [FINAL_DESIGN.md](FINAL_DESIGN.md)
- [SCHEMAS.md](SCHEMAS.md)
- [DEPENDENCIES.md](DEPENDENCIES.md)
- [WIRE_AND_RESTORE.md](WIRE_AND_RESTORE.md)
- [CUTS_TESTS_AND_DELETION.md](CUTS_TESTS_AND_DELETION.md)
- [SOURCE_EVIDENCE.md](SOURCE_EVIDENCE.md)
- [DECISION_REGISTER.md](DECISION_REGISTER.md)
- [FINAL_STATUS.md](FINAL_STATUS.md)
- [OPEN_QUESTIONS.md](OPEN_QUESTIONS.md)

`REQUEST.md` is a byte-identical mirror of the maintained request. The
machine contract, checksum manifest, validator, negative mutation corpus,
and validation report make the readiness claim reproducible.
