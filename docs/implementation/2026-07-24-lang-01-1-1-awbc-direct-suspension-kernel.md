# Lang-01.1.1 AWBC direct-suspension kernel

Date: 2026-07-24

## Outcome

The existing ABI-1/codec-7 AWBC frame substrate now has the typed terminal and
cleanup behavior needed by direct suspension without introducing a provisional
ordinary-function wire kind:

- `AwbcTerminator::Await` accepts both `TaskHandle` and the already allocated
  `NeedHandle` runtime type;
- `FiberStatus`, `FiberTerminalValue`, and `VmExit` retain terminal
  cancellation separately from return and trap;
- `cancel_fiber` detaches a live suspension, drains lexical scopes and frame
  roots from callee to caller in LIFO order, and records cancellation once;
- normal return and tail transfer drain the active frame's scopes as well as
  its root cleanups;
- trap paths drain the whole stack before publishing the trap;
- in-memory `FiberState` snapshots validate and round-trip cancelled and
  nested suspended stacks without replaying a cleanup; and
- the effect-free compiled-region dispatcher keeps its running-only entry
  invariant and rejects an already-cancelled fiber without reclassifying the
  terminal as return or trap.

Product AWBC task cancellation no longer becomes a
`HostAbiMismatch` trap. The maintained structured runtime uses the same
non-error terminal projection, preserving the existing parity harness while
the compact fiber retains the exact `Cancelled` terminal.

## Completion classification

| Boundary | State | Evidence |
|---|---|---|
| same-fiber call frame, exact return cursor/destination | `LANDED_VALIDATED` | focused AWBC direct-suspension tests |
| nested suspension snapshot/restore validation | `LANDED_VALIDATED` | JSON logical-state round trip and wrong-owner rejection |
| cancellation/trap cleanup LIFO and exactly once | `LANDED_VALIDATED` | three-frame, cancelled-registration, owned-drop, and late-terminal tests |
| `NeedHandle` await verifier acceptance | `LANDED_VALIDATED` | the direct-suspension program verifies without a task-handle surrogate type |
| authored ordinary `fn` to AWBC callable/frame lowering | `DESIGN_BLOCKED` | the final ordinary AWBC function-kind allocation belongs to pending codec-8 reconciliation |
| Ready/Err `Need<T,E>` same-step materialization | `MISSING` | current Product `Await` still resolves task-plan events and does not own a typed in-memory Need value |
| non-Need `await`, exact borrow range, and effect-trait diagnostics | `MISSING` | these semantic rows are outside this core runtime cut and remain implementation work |
| project/LSP callable execution publication | `MISSING` | tooling must consume canonical checked identities instead of synthesizing a hover-only callable identity |
| StreamFactory/runtime/wire/save projection | `DESIGN_BLOCKED` | Lang-01.3.1.2.2.1 correction is pending |

The core tests intentionally use an internal synthetic callee to exercise the
already existing frame contract. They do not project an authored ordinary
function as `Synthetic`; doing so would create a temporary public authority.

## Explicit non-goals

- No AWBC ABI, codec, opcode, save-schema, or `AwbcFunctionKind` change.
- No new `RuntimeValue` Need surrogate or string-to-Need compatibility wrapper.
- No provisional ordinary-function lowerer and no reuse of `Synthetic` as its
  public kind.
- No changes to the blocked Stream runtime/wire work.
- No claim that Lang-01.1.1 Cut 3 is complete: typed Need behavior, semantic
  negative rows, effect traits, tooling publication, and the final
  authored-function lowering remain open under their owners.

## Validation

- `cargo test -p arcweft-core --tests`: 231 passed across the library and two
  integration targets, including all 8 direct-suspension rows;
- `cargo test -p arcweft-runtime-codegen --lib`: 10 passed, including the
  cancelled compiled-entry rejection;
- `cargo test -p arcweft-runtime-plan --test awbc_product_parity`: 58 passed;
- `cargo check --workspace --all-targets --all-features`: passed. Its first
  two attempts exposed and then closed the two exhaustive `VmExit::Cancelled`
  consumers in runtime-codegen and a runtime-plan test helper;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-workspace`: the first quiet low-parallel attempt was stopped after
  2,404 seconds by the command timeout. A non-quiet retry of its first
  workspace-wide command then passed in 115.8 seconds. The complete recipe
  passed through the workspace and preceding CLI targets and stopped only at
  the two parent-reproducible external-capability fixtures documented in the
  [parent note](2026-07-22-lang-01-1-1-direct-style-suspension-generator.md):
  `010_capability_fs_read.arcw` and `002_file_read_task.arcw`. The exact target
  reported 3 passed and 2 failed. The final persistent-cache target, skipped by
  recipe fail-fast, was run separately and passed 2 tests;
- `just test-tier2`: 46 passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`: 3,656 files,
  1,937 Rust files, 908,866 physical Rust LOC, 94 manifests, 0 errors, and 146
  warnings; and
- ZIP intake audit: all 26 recursive `docs/reviews/**/*.zip` hashes have a
  package-specific implementation record and the direct inbox contains 0 ZIPs.

## Structural audit

The canonical audit ran on Jujutsu change `sokxnnqkqruy`. No Cargo dependency,
feature, facade export, or crate-edge change was made. `VmExit` gained one
terminal variant, and its existing runtime-codegen and runtime-plan consumers
were migrated directly; no compatibility projection was added.

| Changed Rust file | Class | Bytes | LOC | Embedded test LOC | Responsibility |
|---|---:|---:|---:|---:|---|
| `arcweft-core/src/awbc/fiber.rs` | production | 70,992 | 1,998 | 203 | fiber/frame state, snapshots, terminal validation, cleanup ownership |
| `arcweft-core/src/awbc/verify/code.rs` | production | 72,064 | 1,952 | 0 | AWBC code dataflow and opcode typing |
| `arcweft-core/src/awbc/vm.rs` | production | 58,319 | 1,606 | 0 | compact VM execution and terminal observation |
| `arcweft-core/src/awbc/product_step.rs` | production | 43,439 | 1,154 | 0 (external test module) | Product AWBC step orchestration |
| `arcweft-core/src/engine/suspend.rs` | production | 36,627 | 921 | 0 | maintained structured suspension/task-event path |
| `arcweft-core/tests/direct_suspension.rs` | integration test | 23,311 | 742 | n/a | direct frames, snapshot, cancellation, cleanup, and trap matrix |
| `arcweft-core/src/awbc/product_step/lifecycle.rs` | production | 25,338 | 646 | 0 | Product terminal and lifecycle projection |
| `arcweft-runtime-plan/src/awbc_lower/tests.rs` | unit-test module | 21,281 | 611 | n/a | AWBC lowerer execution fixtures |
| `arcweft-core/src/awbc/product_step/suspension.rs` | production | 22,914 | 563 | 0 | Product await/await-many event consumption |
| `arcweft-runtime-codegen/src/tests.rs` | unit-test module | 14,887 | 422 | n/a | compiled-region contract tests |
| `arcweft-runtime-codegen/src/awbc_region.rs` | production | 11,194 | 322 | 0 | effect-free baseline region lowering |
| `arcweft-core/src/awbc/parity.rs` | production | 8,375 | 254 | 0 | normalized structured/Product trace |
| `arcweft-core/src/awbc/product_step/execution.rs` | production | 9,342 | 236 | 0 | pure helper execution boundary |

The three changed production files above the 1,200-LOC warning remain below
the 2,500-LOC error threshold and did not grow by 300 LOC. Their additions stay
inside the existing fiber-state, verifier, and compact-VM owners; extracting
the terminal/cleanup rules into cross-owner helpers would make the authority
less discoverable. The largest workspace Rust file remains the generated
Unicode `vertical_orientation.rs` table (357,456 bytes, 12,399 LOC, explicitly
marked generated). The next four largest files are unrelated CLI integration
tests (`cli_runtime_bench.rs` at 7,019 LOC, `native_vertical.rs` at 6,712 LOC,
`published_jlreq_class_mix.rs` at 6,109 LOC, and
`native_samples_effects.rs` at 5,901 LOC); this cut does not extend them.
