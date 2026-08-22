# Lang-01.5.1.1.2.1.1.1.1.1.1.1.1 accepted final design

Status: `READY_FOR_IMPLEMENTATION`

Inspected production commit:
`61779d1432b902efc2d19041a7326f3c1319828a` (`main == origin/main`, clean
before this design cut).

This directory is the repository-native accepted resolution of
[the runtime launch-receipt, keyed-ordinal, and current-owner correction](../../requests/2026-08-22-lang-01.5.1.1.2.1.1.1.1.1.1.1.1-runtime-launch-receipt-keyed-ordinal-and-current-owner-correction.md).
It supersedes the implementation-readiness claim of the frozen returned
package without modifying that historical mirror.

The normative material is:

- [FINAL_DESIGN.md](FINAL_DESIGN.md) — selected behavior, ownership,
  transactions, snapshots, Match projection, and implementation cuts;
- [schemas/final_contract.rs](schemas/final_contract.rs) — complete Rust-shaped
  changed/new seams and projection shapes;
- [OWNERSHIP_MATRIX.md](OWNERSHIP_MATRIX.md) — exact Option/Result and nested
  `AgentBuiltinType` decisions;
- [MATCH_CHILD_EDGES.md](MATCH_CHILD_EDGES.md) — HIR-only edge inventory and
  sema enrichment;
- [CROSS_CRATE_REACHABILITY.md](CROSS_CRATE_REACHABILITY.md) — exhaustive
  create/read/mutate ownership and atomic coordinator transcript;
- [CUTS_TESTS_AND_DELETION.md](CUTS_TESTS_AND_DELETION.md) — compile-clean
  order, deletions, and executable acceptance tests;
- [SOURCE_EVIDENCE.md](SOURCE_EVIDENCE.md) — current owner and Git evidence;
  and
- [machine/final_contract.json](machine/final_contract.json) plus
  [tools/validate_design.rs](tools/validate_design.rs) — machine contract and
  repository-aware differential validator.

`OPEN_QUESTIONS.md` is exactly `none`. Numeric AWBC opcode, function-kind, and
flag allocation is outside this design and remains unchanged.

## Validation

Run from this directory:

```bash
cargo +nightly -Zscript tools/validate_design.rs --repository-root ../../../..
cargo +nightly -Zscript tools/validate_design.rs --repository-root ../../../.. --self-test
```

The validator parses Rust syntax trees for current enum/field/visibility
inventories and uses `cargo metadata` for dependency direction. It does not
accept substring agreement as implementation evidence. Future production
acceptance still requires the typed compile, behavior, codec, and compile-fail
tests listed in `CUTS_TESTS_AND_DELETION.md`; this design validator is not a
replacement for them.
