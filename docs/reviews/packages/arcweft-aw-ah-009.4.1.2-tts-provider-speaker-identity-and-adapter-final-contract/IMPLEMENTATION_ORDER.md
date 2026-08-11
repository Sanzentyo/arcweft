# Required implementation order

No implementation is part of this package. The following eight cuts are the
only accepted order. Each cut must compile at its push point, and no cut may
publish a transient legacy/final dual surface.

## Cut 1 — consume the final manifest and generated-metadata owners

### Entry gate

The final Lang-01.5.1 single span-preserving schema-1 manifest decoder and
immutable artifact-handle path are available, or this cut lands their already
selected final owners first. Lang-01.4 typed-resource public ownership is
consumed, not re-specified.

### Work

1. Add TTS nominal manifest records to `arcweft-manifest-model`.
2. Add `tts_providers` directly to schema-1 `arcweft-adapter-metadata` model,
   validation, canonical JSON, payload digest, and ABI digest.
3. Add source-map paths for every TTS manifest coordinate to the sole decoder.
4. Extend external module/artifact joining with provider/export/protocol/config
   checks; do not read provider metadata through another parser.
5. Reserve AWFB section kind codes 22 and 23 in the owning enum and container
   allow-list, but do not publish payloads before Cut 3.
6. Add `tts.synthesize`, deployment, transport, and secret capabilities to the
   existing typed capability owners.

### Exit gate

Canonical schema-1 metadata round trips and tamper tests pass; duplicate,
unknown, malformed, wrong-artifact, wrong-ABI, secret-bearing, and one-over
manifest inputs fail with ranges; existing manifests without `tts` decode with
an empty TTS spec; no runtime/source callable is visible yet.

## Cut 2 — add the lower nominal/request/result model with no I/O

### Work

1. Add workspace member and dependency entry for `arcweft-audio-tts`.
2. Implement identity, locale, profile, catalog record, intent, selected request,
   progress, result, error, protocol record, digest, and limits modules.
3. Put validation and behavior on the owning TTS types; do not create ad hoc
   endpoint helpers or extension traits.
4. Replace `arcweft-core::task::TtsRequest { voice, text }` with the distinct
   typed intent/final request variants; add inherent `TaskSpec::prepare_tts` and
   `TaskKey::for_tts`. Host adapters must not accept the intent variant.
5. Directly replace generic host task error strings with `RuntimePayload` in
   `TaskEventKind`, `HostTaskOutcome`, scheduler propagation, runtime
   suspension, native host, players, tests, and replay schema 1.
6. Keep `TaskClass::TtsSynthesis`, Task/Need/cancellation, scheduler ordering,
   and host-call ID `tts.synthesize`.

### Exit gate

The new crate has no forbidden dependencies and no I/O; all identity/limit/
serde/redaction/fingerprint tests pass; every task error path is typed with no
parallel string carrier; the workspace compiles with no source TTS function yet.

## Cut 3 — add accepted profile/provider catalogs and canonical codecs

### Work

1. Implement immutable `TtsProfileCatalog`, `TtsAcceptedProviderCatalog`, and
   their validation/selection APIs.
2. Implement Character/profile default/priority and provider binding conflict
   validation.
3. Implement exact profile/provider binary codecs and digest contexts.
4. Add AWFB section kinds 22/23 payload construction, strict decode, artifact
   binding, bundle limits, and product requirement checks.
5. Complete one atomic publication transaction that joins manifest revision,
   generated metadata/artifact handles, typed resources, capabilities, and
   catalogs.
6. Mark provider catalog projections restricted and exclude them from bundle
   summaries, debug symbols, Agent manifests, and MCP resources.

### Exit gate

Both catalogs round trip canonically; all duplicate/order/truncation/trailing/
unknown/noncanonical/oversized/digest/artifact tamper cases fail; no catalog is
partially visible; a bundle with executable TTS calls cannot omit section 23.

## Cut 4 — add host adapter capability and dispatch

### Work

1. Add `arcweft-host-adapter::tts` with `TtsHostAdapter`, executor registry,
   `TtsProviderExecutor`, `SecretResolver`, `CredentialLease`, queues, manual
   host-clock abstraction, and test doubles.
2. Register one owner for `tts.synthesize` through the existing builder.
3. Implement provider selection validation, artifact/ABI/protocol negotiation,
   AWTP codec/state machine, memory/spool buffering, audio-codec validation, and
   typed outcome mapping.
4. Implement exact global/per-provider queue limits, rate limiting, timeout,
   same-provider retry, cancellation, cleanup deadline, and late-event discard.
5. Add provider-specific Rust/Wasm/process adapter crate templates only when a
   concrete provider implementation is selected; each contains its SDK/I/O.
6. Wire deployment capability checks and secret leases. Never place secret
   values in AWTP.

### Exit gate

Scripted adapter success/failure/progress/chunk/retry/timeout/cancel/cleanup
matrices pass for typed Rust, typed Wasm, and canonical process AWTP vectors;
provider keys and secrets are absent from logs/diagnostics/Agent projections;
lower crates still have no SDK/network/process/secret dependencies.

## Cut 5 — integrate typed source/resource/function surfaces

### Entry gate

The final Lang-01.4 public typed-resource/HIR/sema/descriptor reference contract
exists. This cut consumes its exact `ResourceRef<T>` and retained
`CharacterRef` rules.

### Work

1. Publish descriptor `std.audio.TtsVoiceProfile` with exact ordinals and limits.
2. Lower resource values to `TtsVoiceProfile` plus optional
   `CharacterTtsProfileBinding`.
3. Publish the three ordinary standard callables and effect `tts.synthesize`.
4. Add the typed `TtsSynthesisIntentTemplate` variant and lower/evaluate it
   through the existing ordinary await/suspension path without the stringly
   host-call argument parser. The emitted TaskSpec remains internal until Cut 6
   prepares it.
5. Add HIR/sema/project-wide duplicate/default/priority diagnostics, AWBC/runtime
   nominal layouts, signature help, hover, completion, go-to-definition, and
   format support.
6. Ensure source cannot construct provider IDs/keys or credentials.

### Exit gate

Positive type-check/lowering/signature-help tests pass; wrong reference family,
unknown Character/profile, missing/ambiguous mapping, unsupported locale/
option, old argument names, and old declarations fail through typed APIs and
compile-fail fixtures; no source gate exists.

## Cut 6 — integrate runtime cancellation, save, reload, replay, debug, privacy

### Work

1. Add `arcweft-runtime-driver::tts::TtsPreparationContext` using one accepted
   catalog generation and availability snapshot.
2. In the existing `BundleSession::dispatch_requested_tasks` path, invoke
   `TtsAcceptedCatalog::prepare_request`, then inherent
   `TaskSpec::prepare_tts`, before task-registry publication or
   `HostTaskDispatch`; preparation failure queues a typed error and no dispatch.
3. Construct the deterministic request fingerprint and `TaskKey::for_tts`, pin
   the generation through existing task pinning, and convert typed Task
   progress/result/error to the exact Need state.
4. Reuse existing `HostTasks` and `TaskGenerationPins` save blockers; enforce
   completed-result save/replay budgets with no active TTS save state.
5. Record/inject complete external outcomes under corrected replay schema 1.
6. Implement queued reload compatibility tuple, active generation completion,
   and `CatalogChanged` cancellation.
7. Add sanitized ordinary debug, privileged audio debug, metrics, Agent/MCP
   projection, and content-sensitive result policy.
8. Add a non-interference test showing dialogue/View projection runs without
   TTS capability, provider, catalog, or credential.

### Exit gate

Cancellation, timeout, replay, save blocker, hot reload, deterministic TaskKey,
nondeterministic output digest, privacy, and dialogue non-blocking tests pass in
native/headless and applicable Web paths.

## Cut 7 — delete ambiguous provisional and historical surfaces

### Work

1. Delete the stringly core `voice` field and `tts.synthesis` operation spelling.
2. Delete `EntityDeclKind::Voice`, `voice profile`, and `voice` top-level grammar
   only as part of the already selected Lang-01.4 direct reduction; retain no
   syntax recognizer or migration node.
3. Delete every provider-valued `speaker` field and every Character-valued
   parameter named `speaker` found during typed API migration; replace call
   sites with `tts_speaker` or `character` before the deletion lands.
4. Update audio/TTS docs, examples, schemas, generated fixtures, and standard
   adapter descriptions to the final model.
5. Preserve `CharacterDialogueVoiceId` only as the independent presentation
   reference selected by its owning projection contract; do not map it to TTS
   or rename it in this sequence.

### Exit gate

The workspace has one final TTS surface. Old inputs get ordinary unknown
current-syntax errors, not dedicated removed-name diagnostics. There is no
alias, shim, dual reader, source gate, CSS, or Takumi path.

## Cut 8 — complete validation and evidence

Run, in order:

1. focused identity/catalog/codec/source/runtime/adapter tests;
2. `cargo fmt --all --check`;
3. `cargo check --workspace --all-targets --all-features`;
4. `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
5. `cargo test --workspace --all-targets --all-features`;
6. adapter simulation and process/Wasm protocol vectors;
7. canonical codec tamper and one-over tests;
8. capability, privacy, save, replay, reload, and dialogue non-interference tests;
9. `cargo metadata` dependency-direction assertions and trybuild/API rejection;
10. applicable Tier 2, native/Web/headless parity, and repository `just` gates.

Source-text search may be used only as navigation while implementing. It is not
an acceptance gate. Structural acceptance uses Cargo metadata, typed APIs,
canonical codec behavior, compile-fail fixtures, and runtime observations.

### Final exit gate

Every applicable row in `TEST_MATRIX.md` is green; generated artifacts are
reproducible; no untracked result-changing choice remains; implementation notes
record exact commands/revisions/results and any non-applicable Tier 2 row with a
specific owner-based reason.
