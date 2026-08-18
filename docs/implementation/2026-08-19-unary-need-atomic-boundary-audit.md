# Unary Need atomic boundary audit — 2026-08-19

## Inspected state

- Git revision: `7169fd6d7` (`Verify pure Try closure frames in AWBC`).
- Branch: clean `main`, matching `origin/main`.
- Maintained target:
  [Await, unary Need, carrier blocks, and `try`](../01-language/await-need-result.md).
- Sequencing authority:
  [Post-Try convergence implementation order](2026-08-18-post-try-convergence-order.md).

## Current owner inventory

A scoped Rust/stable-document search found approximately 140 binary-Need
references. The structural Rust owners span roughly 40 files across these
boundaries:

1. `arcweft-need::Need<T, E>` still owns `Err(E)` and `map_err`.
2. `arcweft-lang-sema::TypeKind::Need { ready, error }` and
   `EnvironmentTypeProjectionKind::Need { ready, error }` still publish a
   binary semantic type, including substitution, ordering, openness, digest,
   callable projection, nominal resolution, and entry contracts.
3. RuntimePlan type seeds/projections still retain separate Need ready/error
   coordinates, and compiler semantic projection preserves both.
4. `RuntimeNeedState` stores `Need<RuntimePayload, RuntimePayload>`.
5. Await checking still constructs a synthetic physical Result from those two
   coordinates and checks Error/Denied observer branches.
6. structured and Product AWBC suspension still turn Ready into `Result::Ok`
   and Error into `Result::Err`; AWBC task plans retain separate ready/error
   type rows.
7. adapter manifests, registration inputs, bundle codecs, topology, LSP, and
   tests still encode or display binary Need applications.

The producer-facing inventory includes at least:

- three standard system-info calls with `system.SystemError`;
- four native-file calls with `fs.FsError`;
- eight desktop owned-window/cursor source functions with `DesktopError`; and
- the standard environment's `load_bg`, `asset.image`, `voice.load`, and
  `content.ensure` declarations.

The adapter type algebra already has a closed `Result { ok, error }` node and
the registered system/file/desktop families already retain exact nominal error
owners. Their unary migration can therefore construct
`Need<Result<Success, Error>>` from typed manifest coordinates. It does not
need a capability-name or terminal-type-name switch.

One later outcome boundary is already partially present: core task events and
host completion distinguish authored domain `Error(payload)` from
infrastructure `Failed(message)`. That split is not unary Need by itself. In
the final model, any fallible asynchronous producer's admitted output type is
`Result<T, E>`, and temporal completion publishes that entire carrier as one
Ready payload.

## Required atomic transaction

The implementation cannot safely begin by changing only the `arcweft-need`
crate. Removing `Err(E)` there while retaining Await's Result synthesis would
either stop the workspace from compiling or create a second domain-error
authority. The compiling transaction must preserve this internal order:

1. Replace all semantic/environment/runtime-plan Need type nodes with one item
   coordinate. Update arity, substitution, ordering, openness, digest, codec,
   and verifier logic in the same transaction; retain no binary alias.
2. Rewrite fallible registered producer signatures from `Need<T, E>` to
   `Need<Result<T, E>>` using their accepted Result and nominal error
   identities. Do not reconstruct this from callable spelling.
3. Replace `arcweft_need::Need<T, E>` with `Need<T>` and remove Err/map_err.
   Replace `RuntimeNeedState` with the unary state and make Ready carry exactly
   the admitted `T`.
4. Make Await check and lower as `Need<T> -> T`. Delete synthetic Result
   materialization and Await Error/Denied branches; keep Pending observation
   and non-returning cancellation.
5. Evolve structured runtime, Product AWBC, task plans, codecs, snapshots,
   adapters, scheduler bridges, and persistence in place with every Arcweft
   version marker still `1`.
6. Migrate fixtures and all typed consumers, then require structured/AWBC
   parity for `Need<T>`, `Need<Result<T,E>>`, Pending, cancellation, and runtime
   fault paths.

If intermediate compilation cannot be preserved between these internal
steps, they land as one commit. A binary compatibility type, dual reader,
direct-host-only Await exception, or `Ready`-plus-`Error` core Need state is
not an acceptable checkpoint.

## Validation performed

- Read-only source and maintained-document inventory.
- `rg` owner scans over Rust and maintained language/runtime documentation.
- `cargo check --workspace --all-targets --all-features`: passed on the clean
  inspected revision without an explicit Cargo job count.
- `cargo test -p arcweft-need`: 3 passed. The baseline intentionally exposes
  the obsolete `Err` state through
  `terminal_need_states_are_exactly_ready_err_and_cancelled`; the unary cut
  must replace that assertion rather than preserve it.
- `cargo test -p arcweft-core --lib`: 213 passed. The baseline includes Product
  AWBC tests which currently materialize Ready as `Result::Ok`, Err as
  `Result::Err`, and select the first terminal binary-Need sequence. Those
  tests are migration targets: unary Ready must resume with its payload
  unchanged, including when that payload is already a Result.

Failed baseline outside this audit:

- `cargo test -p arcweft-cli --test arcw_fixtures_check_run`: 1 passed and 4
  failed on the clean inspected revision.
  - `current_check_fixtures_pass` first failed at
    `current_pass/check/008_let_else_diverge.arcw` with a HIR typed-arena
    transaction failure. Read-only follow-up found that syntax publishes
    `LetElseStatement`, but attached access and
    `lower_attached_statement` have no LetElse branch; the statement falls to
    `InvalidArenaCommit`, and `HirStmtKind::LetElse` has no production
    constructor. Its current statement-only `else_body` is also insufficient
    for a Flow's heterogeneous contextual body, so the final fix should reuse
    `HirContextualStmtBody` rather than adding a fixture-specific fallback.
  - `current_run_fixtures_pass` first failed at
    `current_pass/run/009_dialogue_line.arcw` because executable dialogue
    projection lacked the selected profile character-name localization policy.
  - `spec_should_pass_check_fixtures_pass_after_refactor` first failed at
    `spec_should_pass/check/022_multiline_trait_method.arcw`; direct execution
    reports that nominal resolution did not produce one complete type for the
    multiline trait method signature.
  - `spec_should_pass_run_fixtures_pass_after_refactor` first failed at
    `spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw`;
    direct checking reports that its HIR module is recovered and therefore not
    executable. The fixture contains the still-unconnected typed dialogue line
    plan/handle surface, so this is not a unary Need completion signal.

These four failures predate the unary Need source transaction and must not be
reported as regressions introduced by that cut. The directory-wide gate stays
active; this record is evidence, not an exclusion list.

## Explicit non-goals

- No Stream type migration.
- No timeout combinator implementation.
- No producer-name switch or error-label inference.
- No repair of the binary Need execution path before its deletion.
