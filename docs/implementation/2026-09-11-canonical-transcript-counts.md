# Canonical schema and value count encoding — 2026-09-11

Status: implemented and validated; known workspace/Tier 2 failures are retained
below. This is completion of the count-encoding correction only.

The inspected base is `main` at
`7cfb6727a714c5db06a1e8c83eb6a1d1e9df4822`. This correction completes the
shared count-encoding boundary required by the
[accepted nominal wire contract](../reviews/designs/lang-01.5.1.1.2.1.1.1.1.1.1.1.1.1-accepted-structural-nominal-runtime-carrier/WIRE_AND_RESTORE.md).
It does not complete nominal C1-C6 or the
[convergence goal](2026-09-08-convergence-goal-plan.md).

## Changed behavior and authority

Canonical schema and runtime-value encoders now use the existing core
`canonical_varint::encode_u32` authority already used by AWBC. Schema version
`1`, schema string/collection lengths, runtime-value string/collection lengths,
and variant ordinals use shortest unsigned base-128 encoding. A 300-byte string
now has the three-byte prefix `07 ac 02` and fits exactly 303 encoded bytes.
A 300-element unit tuple starts `0b ac 02`; ordinal 300 is `ac 02`.

The one exhaustive runtime-value visitor continues to serve both byte output
and direct BLAKE3 output. Its bounded write operation charges the actual varint
length. A limit one byte below the complete transcript returns an error from
both consumers and exposes no result. Numeric payload bits retain explicit
fixed-width writes, including F32/Progress, character scalar values and
viewport coordinates. The schema layout hash consumes the same canonical
schema bytes used by its byte-level tests.

The obsolete fixed-u32 count and ordinal paths are replaced in place. The
canonical opaque-value golden now uses the shortest producer-name length.
Dialogue's exact-size fixtures measure candidate payloads through the public
encoder, including varint width transitions. Their original exact-limit and
one-over assertions and production limits are preserved.
All contract version markers remain `1`. This changes canonical bytes and
their derived hashes; the unreleased shapes evolve under the repository's
version-1 policy.

## Verification and exact cut

Logs are under `.arcweft-local/validation/2026-09-11-canonical-transcript-counts/`.

- `red.log`: two new count/schema tests failed against the old encoder;
  fixed numeric payloads and the existing custom-sink test passed.
- `core-tests.log`: 367 library tests passed, with the old opaque producer-name
  golden failing after the encoding change. Its exact expected bytes were
  migrated to the accepted count encoding.
- `core-tests-fixed.log`: **400 core tests passed** (368 library and 32
  integration tests, including the compile-fixture runner); zero doctests.
  Four new tests cover exact schema/value fragments, length and ordinal 300,
  fixed numeric bits, BLAKE3 parity, and exact/one-under byte limits.
- `dialogue-tests.log`: 28 passed and two exact-size fixtures failed because
  they assumed fixed-width string headers. After replacing that assumption,
  `dialogue-tests-fixed.log` records **38 passed** (30 library, 4 integration,
  4 doctests).
- `nominal-projection-tests.log`: **12 passed** through sema's direct nominal
  schema consumer. `test-doc.log`: **8 workspace doctests passed**.
- `workspace-check-final.log` and `workspace-clippy-final.log`: isolated
  workspace all-target/all-feature check and Clippy **passed**, with warnings.
- `test-workspace.log`: **failed** at compiler `callable_execution`, with
  **57 passed / 24 failed**. Exact failure names match the preceding accepted
  borrowed-driver cut; `failure-comparison.json` records no additions or
  removals. Later workspace binaries and the recipe's subsequent CLI steps
  were **not run** after that failure.
- `test-tier2.log`: MCP **4 passed**, Agent observe **1 passed**; native
  auxiliary capture **failed** at
  `agent_observe_read_uri_preserves_animated_image_object_frame_metadata`.
  The unchanged `samples/image-animation.arcw` uses `pub image`, leading to
  required HIR recovery before semantic analysis. Subsequent Tier 2 targets
  were **not run**. This is the previously recorded image-grammar failure,
  not a successful full Tier 2 run.
- Final structure audit and gate **passed**: **95 packages, 2,270 Rust files,
  310 review triggers, zero blocking violations**. Measurements include the
  Dialogue fixture correction. Formatting and diff checks passed.

The only change after the first workspace/Tier 2 runs was the Dialogue test
fixture correction. Its crate suite and workspace check/Clippy/structure gates
were rerun; production Rust remained identical. `just verify` and generated
JLREQ validation were not selected for this encoder correction. Cargo commands
ran sequentially with normal concurrency and no explicit job count.

The three Rust files were explicitly staged and their cached diff inspected.
Rust validation uses index tree `ca4b9491dc77885b51d292be6034b11b317c9612` on the
base above. Thirty unfinished callable/scope files were preserved separately
with complete local byte copies, Git blobs, a reversible patch and a SHA-256
manifest. After validation, all 30 unfinished files were restored; the complete
33-file manifest, including the three Rust files in this cut, passed raw-byte
hash verification. No additional checkout,
branch or worktree was created.

## Structural disposition

[Current measurements](structure-audits/2026-09-11-canonical-transcript-counts/changed-files.csv)
cover complete files, not diff additions:

| Owner | Bytes | Physical LOC / growth | Embedded test LOC |
| --- | ---: | ---: | ---: |
| Core schema validation and canonical transcript | 58,507 | 1,679 / +99 | 141 |
| Exact opaque owner and value admission | 94,269 | 2,441 / -1 | 644 |
| Dialogue exact-size test fixtures | 33,722 | 958 / +8 | 0 |

The core crate's workspace dependency fan-in/out is 29/6; development
fan-in/out is 3/6. No dependency, feature, public type, facade export or I/O
boundary is added. The count encoder is the existing core primitive, not a
second schema or codec authority. Both sinks retain the same visitor and
budget checks. The byte-level tests belong with those private encoders; the
opaque golden remains with the opaque owner's existing tests. The touched
size/test review triggers retain these cohesive responsibilities. Splitting
the files solely for this count correction would widen private test access
without changing an ownership boundary.

The Dialogue crate's normal fan-in/out is 8/9 and development fan-in/out is
8/0. Its changed file is an existing dedicated test module. The sizing helper
constructs valid boundary fixtures through the public encoder and does not
duplicate varint encoding or weaken a runtime limit.

## Required continuation

The typed reachable nominal schema graph, four record shapes, exact Rust ADT
catalog join, executable plan domains, layout-bearing AWBC rows,
program-bound restore and structural ownership admission remain required by
the accepted C1-C6 plan. The existing incomplete callable component remains
in progress separately. No structural Rust ADT success or runtime scheme
completion is established by this encoder correction.
