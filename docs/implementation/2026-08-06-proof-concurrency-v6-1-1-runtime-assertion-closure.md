# Proof-concurrency v6.1.1 runtime assertion closure

- Date: 2026-08-06
- Inspected Git revision: `454ca646d37b0e0e1226e181c5a501c9c8e8de15`
- Working tree: dirty protected Proof public-switch WIP on
  `codex/proof-public-switch`
- Status: `RUNTIME_ASSERTION_ROWS_VALIDATED_PROOF_CUT_IN_PROGRESS`
- Cargo validation: the exact runtime-assertion owner rows passed; final
  workspace gates for the wider Proof cut remain pending

This note records the current working-copy implementation and the exact
remaining validation surface for the runtime-assertion portion of
Proof-concurrency v6.1.1. It does not award completion credit to uncommitted or
unvalidated code.

The design authority is the accepted
[Proof-concurrency v6.1.1 package](../reviews/packages/arcweft-proof-concurrency-v6.1.1-typed-ast-proof-block-hir-runtime-identity-final-contract.zip),
SHA-256
`1B7DE5F2C10A5B29D67C72011E4272DF9A76AF8907FD21FE162DE54809FC69EF`.
The repository-visible
[runtime assertion guard-seed decision](2026-07-26-proof-runtime-assertion-guard-seed-decision.md)
fixes the canonical seed encoding and the actual persisted-owner boundary.
Its historical validation status is not reused as evidence for this public
switch.

Validation update: the executed commands, counts, and PASS classifications in
[the full base-package matrix ledger](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md#6-runtime-assertion-fault-and-serialization)
supersede this note's original pre-execution `Cargo not run` table and
validation checklist. Those sections are retained below as the audit plan that
selected the legitimate owners; they are no longer the current execution
state.

## Contract precedence and exact-name authority

`TEST_MATRIX.md` is authoritative for acceptance test names and behavior.
`VERIFICATION_PLAN.md` supplies invocation guidance, including its explicit
permission to split the combined persisted-boundary row among legitimate
owners. It does not override a TEST_MATRIX owner or exact name.

Static inspection found one real root integration test for each of the 16
non-distributed runtime-assertion rows. The combined row 12 name is
intentionally absent because that row is split among actual codec and
persistence owners. Apart from a pre-publication focused pass of row 11, these
are static occurrence and owner checks only. That earlier pass is not promoted
to final-copy PASS evidence.

| Row | Exact TEST_MATRIX name | Legitimate owner | Current evidence |
|---:|---|---|---|
| 1 | `check_failure_retains_exact_session_identity` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 2 | `enabled_debug_failure_retains_exact_session_identity` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 3 | `condition_indices_follow_authored_zero_based_order` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 4 | `condition_index_validation_rejects_invalid_count_and_bounds` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 5 | `prove_has_no_runtime_mode_or_guard` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 6 | `release_plan_omits_debug_evaluation_and_inventory` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 7 | `guard_derivation_uses_typed_seed_and_is_deterministic` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 8 | `invalid_guard_and_fingerprint_zero_values_are_rejected` | `arcweft-core/tests/runtime_assertion_identity.rs` | one root test; Cargo not run |
| 9 | `runtime_fault_invalid_guard_is_typed_error` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 10 | `runtime_fault_artifact_mismatch_is_typed_error` | `arcweft-runtime-plan/tests/assertion_identity.rs` | one root test; Cargo not run |
| 11 | `runtime_assertion_core_codec_has_no_session_identity` | `arcweft-core/tests/runtime_assertion_identity.rs` | pre-publication focused pass; final-copy rerun pending |
| 12 | `awbc_bundle_save_checkpoint_cache_round_trip_without_session_ids` | distributed actual owners below | no combined test by design; Cargo not run |
| 13 | `core_dependency_graph_excludes_compiler_layers` | `arcweft-runtime-host/tests/dependency_direction.rs` | one root test; Cargo not run |
| 14 | `runtime_host_normal_graph_excludes_hir_and_runtime_plan` | `arcweft-runtime-host/tests/dependency_direction.rs` | one root test; Cargo not run |
| 15 | `runtime_projection_emits_stable_diagnostic_without_message_parsing` | `arcweft-tooling/tests/runtime_assertion_diagnostic.rs` | one root test; Cargo not run |
| 16 | `reloaded_artifact_uses_fresh_inventory_without_old_stmt_equality` | `arcweft-compiler/tests/assertions.rs` | one root test; Cargo not run |
| 17 | `reloaded_artifact_without_exact_source_association_stays_unassociated` | `arcweft-tooling/tests/runtime_assertion_diagnostic.rs` | one root test; Cargo not run |

The focused exact invocation pattern for rows 1--11 and 13--17 is:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --test assertion_identity --all-features <exact-name> -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-core --test runtime_assertion_identity --all-features <exact-name> -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-host --test dependency_direction --all-features <exact-name> -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-tooling --test runtime_assertion_diagnostic --all-features <exact-name> -- --exact
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --test assertions --all-features <exact-name> -- --exact
```

Only the command whose owner contains the named row applies. These commands are
recorded for the pending validation pass; they have not yet been run.

## Row 12 distributed persisted-boundary evidence

`VERIFICATION_PLAN.md` explicitly permits the combined persisted-boundary row
to be split among owning crates when the implementation note maps every
invocation back to that single row. The working copy therefore does not add a
monolithic test crate, coordinator test, alias, or wrapper bearing the combined
name.

| Owner boundary | Actual owner test | Pending exact invocation | What it proves |
|---|---|---|---|
| canonical AWBC | `arcweft-core::awbc::tests::canonical_awbc_assertion_payload_round_trips_as_typed_identity` | `CARGO_INCREMENTAL=0 cargo test -p arcweft-core --lib --all-features awbc::tests::canonical_awbc_assertion_payload_round_trips_as_typed_identity -- --exact` | canonical AWBC round trip retains the checked typed guard and assertion effect kind |
| AWFB bundle | `arcweft-bundle::tests::awfb_round_trip_retains_typed_runtime_assertion_payload` | `CARGO_INCREMENTAL=0 cargo test -p arcweft-bundle --lib --all-features tests::awfb_round_trip_retains_typed_runtime_assertion_payload -- --exact` | the real AWFB owner retains the canonical AWBC assertion payload and typed guard |
| save envelope | `arcweft-save/tests/runtime_assertion_identity.rs::save_round_trip_retains_the_real_core_runtime_assertion_payload` | `CARGO_INCREMENTAL=0 cargo test -p arcweft-save --test runtime_assertion_identity --all-features save_round_trip_retains_the_real_core_runtime_assertion_payload -- --exact` | the existing strict typed save envelope round trips the actual core failure type |
| compile cache | `arcweft-project-loader::cache::persistent_query::tests::persistent_query_verified_bytecode_unit_is_actual_hit` | `CARGO_INCREMENTAL=0 cargo test -p arcweft-project-loader --lib --all-features cache::persistent_query::tests::persistent_query_verified_bytecode_unit_is_actual_hit -- --exact` | a verified reusable cache hit retains canonical AWBC bytes and the typed guard |
| fiber checkpoint | `arcweft-core::awbc::tests::fiber_checkpoint_and_serde_preserve_cleanup_stacks` | `CARGO_INCREMENTAL=0 cargo test -p arcweft-core --lib --all-features awbc::tests::fiber_checkpoint_and_serde_preserve_cleanup_stacks -- --exact` | the actual generic checkpoint owner round trips `FiberState`; it does not gain an assertion-session carrier |
| session-ID exclusion | runtime-plan and core compile-fail suites | `CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-plan --test api_compile --all-features removed_runtime_plan_apis_are_unavailable -- --exact` and `CARGO_INCREMENTAL=0 cargo test -p arcweft-core --test api_compile --all-features runtime_assertion_identity_boundaries_are_compile_time_closed -- --exact` | runtime assertion site/inventory/fault types are non-Serde, and core cannot name HIR IDs |

Checkpoint and replay are structurally not assertion-payload owners:

- `FiberCheckpoint` contains only a boxed `FiberState`. Its runtime frames,
  suspension, and cleanup records use AWBC IDs and runtime values; they do not
  contain `RuntimeAssertionFailure`, `RuntimeAssertionSite`, a HIR ID, or a
  syntax ID.
- `RootReplayTraceV1` owns the stable artifact, entry, binding, state and event
  identities, runtime payloads, digests, root outcomes, and external host
  outcomes. A trapped root outcome stores a failure digest. It does not store a
  runtime-assertion failure or fresh-session assertion identity.

Adding assertion-specific checkpoint or root-replay fields would invent a new
owner and violate the accepted non-goal boundary. Their row-12 evidence is the
real generic owner plus typed absence, not a fabricated assertion codec.

## Row 11 actual codec boundary

The exact row-11 integration test uses no test-only persisted wrapper. It
round trips the actual `RuntimeAssertionFailure` and the actual
`RuntimeArtifactFingerprint` separately through JSON, CBOR, MsgPack, and
ArcweftBinary, then compares the decoded typed values and the guard,
condition, message, profile, and fingerprint bytes directly.

The existing compiler owner test
`runtime_diagnostics::tests::runtime_artifact_fingerprint_copies_the_canonical_artifact_key_digest`
provides the separate required traceability from the existing runtime-plan
`ArtifactKey` digest to `RuntimeArtifactFingerprint`. Its pending exact
invocation is:

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --lib --all-features runtime_diagnostics::tests::runtime_artifact_fingerprint_copies_the_canonical_artifact_key_digest -- --exact
```

The production Agent/debug path writes the fingerprint and the actual core
failure into the existing `DebugDiagnostic` payload. No second persisted
runtime-assertion carrier was introduced.

## CLI and Agent publication boundary

The current working copy carries the typed core failure through each native
adapter without converting it into an Agent assertion request or using the
failure message as an identity key:

- direct CLI summaries retain `Vec<RuntimeAssertionFailure>` as the sole
  assertion-presence authority; `NoAssertionFailures` checks that typed field;
- direct CLI text/JSON and native HTTP presentation derive the stable
  `runtime.assertion_failed` diagnostic through
  `project_persisted_assertion_failure`; these paths have no canonical
  `ArtifactKey` or fresh-session inventory and therefore report
  `PersistedOnly` rather than fabricating session identity;
- player-backed native Agent observation projects
  `BundleSessionStep::assertion_failures` through the same persisted-only
  projector and leaves the fiber status unchanged; and
- Agent controller bytecode records runtime-language assertion failures in
  `AgentControllerRunReport::assertion_failures` before host-request
  conversion. `AgentHostRequest::Assert` remains owned only by the Agent DSL's
  `expect`/`deny` task operations.

Agent Script text/JSON/debug metadata likewise derive their presentation from
the typed controller report while retaining the controller's successful final
status. This is a deletion-driven authority split: the runtime assertion no
longer falls through the generic Agent effect-to-host-request conversion. No
compatibility request, message parser, fallback identity resolver, or second
runtime-failure carrier was added. Focused adapter tests exist but remain
unexecuted while the coordinated semantic-validation process owns Cargo.

## Package-local verification-plan deviation

The package contains two inconsistent focused-command entries:

- `VERIFICATION_PLAN.md` places
  `runtime_projection_emits_stable_diagnostic_without_message_parsing` under
  `arcweft-runtime-plan`; and
- its tooling section instead names the obsolete
  `agent_runtime_assertion_projection_uses_session_capability`.

`TEST_MATRIX.md` and `RUNTIME_ASSERTION_FAULT.md` make presentation an
`arcweft-tooling` owner and give
`runtime_projection_emits_stable_diagnostic_without_message_parsing` as the
exact acceptance name. The working copy follows those higher-precision
authorities. The one real test is therefore the tooling integration test. No
runtime-plan alias, obsolete tooling-name alias, compatibility wrapper, or
duplicate acceptance test is retained.

## Deletion and duplicate audit

Static review found the following obsolete or superseded test names at zero
Rust occurrences:

- `checked_identity_constructors_reject_reserved_zero_values`;
- `condition_index_validates_authored_order_and_limit`;
- `persisted_failure_joins_only_the_exact_artifact_guard_and_profile`;
- `runtime_host_and_adapter_context_keep_language_pipeline_dependencies_out`;
  and
- `agent_runtime_assertion_projection_uses_session_capability`.

The combined row-12 name also has zero Rust occurrences by design. The removed
test-only `PersistedRuntimeAssertionFailure` and the inspected assertion
persistence-envelope spellings have zero Rust occurrences. Private supporting
tests remain only for private invariants such as runtime-capable mode
conversion, duplicate guard rejection, strict AWBC profile decoding, actual
Agent/debug projection, and the canonical artifact-key byte copy. They do not
duplicate an exact acceptance name.

This name scan is a review aid, not acceptance evidence. The final structural
and behavioral acceptance still depends on the pending typed tests, metadata
graph checks, compile-fail suites, workspace gates, and structural audit.

## Pre-execution validation snapshot (superseded)

### Performed and passed

- Focused core assertion materialization/effect-mapping tests, including true
  omission, false typed emission, and non-Boolean typed failure.
- `bundle_runner_wraps_emitted_assertion_without_condition_parsing` in
  runtime-host and the AWBC product-session assertion transport rows in
  runtime-driver.
- Changed-crate `cargo check` for core, runtime-host, and runtime-driver before
  the later CLI/Agent publication edits. Final coherent-copy checks remain
  pending.
- Read-only extraction and comparison of the accepted package's
  `TEST_MATRIX.md`, `RUNTIME_ASSERTION_FAULT.md`, and
  `VERIFICATION_PLAN.md`.
- Static owner and exact-name inspection for all 17 runtime-assertion rows.
- Static persistence-owner inspection for AWBC, AWFB, save, compile cache,
  fiber checkpoint, and root replay.
- `rustfmt --edition 2024
  crates/arcweft-core/tests/runtime_assertion_identity.rs` after removal of the
  test-only combined persistence carrier.
- `git diff --check -- .` for tracked changes, plus
  `git diff --no-index --check -- /dev/null <new-file>` for the new core
  integration test and this note; all emitted no whitespace errors.

### Not run

- every Cargo command recorded above;
- focused runtime-plan, core, tooling, compiler, runtime-host, bundle, save,
  project-loader, CLI, LSP, and Agent tests;
- public API compile-fail suites;
- workspace check;
- strict workspace Clippy;
- `just test-workspace`;
- applicable Tier 2; and
- the final structural audit.

Those commands remain **NOT RUN**, not passed or blocked. They will be run only
after the coordinated semantic-validation process releases Cargo. Their exact results,
counts, failures, and any intentionally skipped tier must be appended here
before this note can claim a validated coherent cut.

## Explicit non-goals

- no compatibility alias, wrapper, dual reader, source-string parser, source
  gate, or fallback assertion resolver;
- no assertion-specific checkpoint or root-replay schema field where the
  production owner does not transit assertion data;
- no serialization of `StmtId`, syntax/HIR database or snapshot IDs,
  assertion condition-index identity, runtime assertion mode, session site,
  inventory, fault identity, or execution diagnostic context; and
- no completion claim for the wider Proof public switch before the shared
  compiler/LSP and workspace gates pass.
