# Validation report

Date: 2026-08-23

Inspected production commit:
`9168c8ac7285c6b44f29018626a0e7c1b0059796`

Design status: `READY_FOR_IMPLEMENTATION`

## Commands

From this design directory:

```powershell
cargo +nightly -Zscript tools/validate_design.rs --repository-root ../../../..
cargo +nightly -Zscript tools/negative_self_tests.rs
rustfmt +nightly --check tools/validate_design.rs tools/negative_self_tests.rs tools/validation_support.rs
```

## Result

The final positive run passed:

```text
PASS sequence=Lang-01.5.1.1.2.1.1.1.1.1.1.1.1.2
head=9168c8ac7285c6b44f29018626a0e7c1b0059796
artifacts=PASS request_mirror=PASS manifest=PASS source_blobs=PASS cargo_metadata=PASS
```

The final negative corpus passed:

```text
PASS negative_self_tests=13/13
```

The final `rustfmt +nightly --check` run passed with no output.

## Failed and corrected development runs

The first formatting check returned exit code `1` and printed formatting
diffs. The three Rust files were formatted and the package hashes regenerated.

The first positive validator compile failed with Rust `E0106` because the
shared `string_array` helper omitted the lifetime tying returned `&str` values
to the parsed JSON. The lifetime was made explicit.

The next positive invocation compiled but could not derive the design root
from Cargo script-mode's `file!()` path and returned `missing
machine/final_contract.json`. Both scripts now prefer the current directory
when it contains the machine contract and retain an explicit `--design-root`
override. The successful runs above used that corrected behavior. These were
validator-development failures; no production validation failure was hidden.

## Validator scope

The positive validator is read-only. It validates required members,
`FINAL_STATUS`, exact `OPEN_QUESTIONS`, the byte-identical request mirror and
hash, the machine contract, decision/schema mirrors, the single event
queue/drain rule, the final apply transcript, manifest hashes, current Git
HEAD/source blobs, maintained request bytes, and Cargo metadata parsing. It
does not use future production source spelling as evidence that an unlanded
type is implemented.

The negative script mutates candidates in memory only. It must reject changed
status/open questions/request bytes, a missing decision or schema owner, a
second event drain, reordered apply, enabled WAL, a version bump, stale source
evidence, a missing member, and a bad manifest digest.

## Intentionally not run

No production source, test, fixture, codec, generated artifact, or manifest
changed. Workspace check, Clippy, workspace tests, Tier 2, and structural audit
are implementation-cut gates and are not claimed by this docs-only design.
