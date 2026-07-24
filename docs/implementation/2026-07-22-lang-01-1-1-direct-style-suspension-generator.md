# Lang-01.1.1 direct-style suspension and generator classification

Date: 2026-07-22

## Outcome

Ordinary `fn` declarations now publish a typed semantic execution fact instead
of relying on the removed author-facing `task fn`, `dialogue fn`, or
`stream fn` roles:

- `CheckedCallableExecution` is keyed by canonical
  `CallableDeclarationId`;
- `CallableExecutionMode::DirectFrame` covers ordinary direct execution and
  `Stream<T, E>` passthrough functions with no own-scope `yield`;
- `CallableExecutionMode::StreamFactory` covers an ordinary function whose
  resolved return type is `Stream<T, E>` and whose own body suspends through
  `yield`;
- `StreamGeneratorFacts` records the resolved element/error contract and the
  retained suspension sites; and
- an own-scope `yield` contributes the direct `control.suspend` effect.

The suspension walk crosses ordinary expression/control-flow containers but
does not steal `yield` from another execution owner such as a closure,
`Seq`/nested stream body, thread, event handler, dialogue body, or source
owner.

Maintained design chapters, examples, and positive fixtures now use ordinary
`fn`. Historical design packages, implementation records, and negative
removed-syntax fixtures remain historical evidence and were not rewritten.

## Deletion-driven public switch

The 2026-07-24 follow-up removed the provisional authored-role authority
instead of repairing it:

- `FunctionKind`, its syntax/HIR fields, parser prefix stripper, cache tag, and
  callable-contract digest byte were deleted;
- only ordinary `fn` enters the function grammar. `task fn`, `dialogue fn`,
  and `stream fn` now use ordinary top-level recovery and cannot produce a
  `FunctionItem` or `HirFunction`; no spelling-specific compatibility
  diagnostic or AST kind remains;
- generator body checking consumes only
  `CallableExecutionMode::StreamFactory`, derived from a checked
  `Stream<T, E>` result and final own-scope `yield` classification;
- effect rows, Fx validation, entry-role contracts, runtime function values,
  pure-helper lowering, SMT contract lowering, and persistent HIR facts no
  longer branch on an authored function role; and
- the provisional runtime-plan `stream fn` lowerer was deleted together with
  its tests. The existing runtime-internal Stream/Source and AWBC categories
  are not aliases for the removed syntax and remain until their separately
  specified atomic runtime/wire replacement.

This switch deliberately does not connect `StreamFactory` to a provisional
Stream ABI. That final consumer remains behind the accepted
Lang-01.3.1.2.1/.2 contracts and the unresolved codec-8 allocation recorded in
[the curried Stream intake](2026-07-24-lang-01-3-1-2-2-curried-stream-intake.md).

## Remaining completion boundary

The ordinary-function parser/HIR switch and semantic execution classification
are complete. The sequence itself is not complete, however. Its remaining
rows are classified by final owner rather than being collapsed into a single
Stream dependency:

| Boundary | State | Remaining work or owner |
|---|---|---|
| ABI-neutral same-fiber frames, terminal cancellation, and whole-stack cleanup | `LANDED_VALIDATED` | [the 2026-07-24 direct-suspension kernel note](2026-07-24-lang-01-1-1-awbc-direct-suspension-kernel.md) |
| direct `Need<T, E>` Ready/Err materialization and same-step `await` | `MISSING` | replace the current task-plan-only Product `Await` reader with the final typed in-memory Need owner; do not add a wire surrogate |
| non-Need `await`, exact borrow range, and `ThreadHandle` negative evidence | `MISSING` | semantic diagnostic and negative-matrix closure |
| effect-trait requirement/implementation facts and diagnostics | `MISSING` | typed semantic owner and its direct tests |
| callable execution facts in project/LSP indexes | `MISSING` | publish canonical checked facts; do not synthesize callable IDs from hover text |
| authored ordinary-function AWBC kind and public lowering | `DESIGN_BLOCKED` | final codec-8 kind allocation and opcode interleave reconciliation |
| `StreamFactory` runtime/wire/save projection | `DESIGN_BLOCKED` | Lang-01.3.1.2.2.1 correction and the final Stream authority switch |
| detached external-capability `FsError` fixtures | `DESIGN_BLOCKED` | Proof attached syntax/HIR public switch, not a raw-body repair in this sequence |
| old RuntimeCallable/StreamPlan wire rows and source-spelling scans | `SUPERSEDED` | final Lang-01.3 contracts and the repository-wide source-gate prohibition |

The ABI-neutral kernel does not make the blocked authored-function wire
projection, typed Ready/Err Need behavior, semantic negative rows, effect
traits, or tooling publication appear complete. No provisional
`CheckedReturnTarget`, compatibility alias, dual reader, synthetic authored
function kind, or source gate is introduced while those final owners are
pending.

## Baseline validation boundary

The normal workspace recipe still stops in
`arcweft-cli --test arcw_fixtures_check_run` on the two existing
`spec_should_pass` capability fixtures that declare `type FsError` inside an
`extern capability` block. The detached public `ExternCapabilityItem` retains
capability functions plus a raw body, but does not publish the capability
`type` members to HIR/sema. The CLI therefore reports
`sema.nominal.unknown_type` for `FsError`.

This is not a regression from the authored-role deletion. Running the exact
test on the unchanged parent `22a3c9e8` produced the same two failures:

- `spec_should_pass_check_fixtures_pass_after_refactor` at
  `010_capability_fs_read.arcw`; and
- `spec_should_pass_run_fixtures_pass_after_refactor` at
  `002_file_read_task.arcw`.

This cut does not repair that detached carrier by reparsing its raw body and
does not hard-code `FsError` into the standard nominal environment. The
one-pass grammar already owns typed capability type/function members, as
recorded in
[Proof Stage 1 external capability grammar](2026-07-17-proof-concurrency-v6-1-1-stage-1-extern-capability.md).
Those members reach HIR/sema when the old public syntax reader is deleted by
the Proof public authority switch.

## Verification

- `cargo check -p arcweft-lang-syntax -p arcweft-lang-hir`: passed;
- `cargo check -p arcweft-lang-sema -p arcweft-runtime-plan -p arcweft-verify`:
  passed;
- `cargo test -p arcweft-lang-syntax --lib`: 486 passed;
- `cargo test -p arcweft-lang-sema --lib`: 1,115 passed;
- `cargo test -p arcweft-runtime-plan --tests`: 226 passed across the library
  and integration targets;
- `cargo test -p arcweft-compiler --lib`: 92 passed;
- `cargo test -p arcweft-verify --lib`: 40 passed;
- focused CLI source-plan/runtime-state tests: 2 passed;
- `cargo check --workspace --all-targets --all-features`: passed;
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed;
- `just test-workspace`: the workspace and preceding CLI targets passed, then
  the recipe stopped on the two parent-reproducible capability fixture
  failures documented above. The remaining persistent-cache golden target was
  run separately and passed 2 tests;
- the first default-parallel workspace attempt exhausted the Windows paging
  file (`OS error 1455`). A one-job retry ran for 904 seconds without a test
  failure before its command timeout; a two-build-job/four-test-thread retry
  avoided the resource failure and exposed only the baseline fixture boundary;
- `just test-tier2`: 46 passed;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .` on Jujutsu
  change `sptysupxpsulrksyqzzrpwuxyxlpluly`: 3,654 files, 1,936 Rust files,
  907,967 physical Rust LOC, 94 manifests, 0 errors, and 146 warnings; and
- format check and diff check: passed.
