# AW-AH-009.4.1.2.1 TTS runtime-envelope package intake

Date: 2026-07-24

## Intake identity

The returned archive is
[`arcweft-aw-ah-009.4.1.2.1-tts-runtime-intent-envelope-architecture-reconciliation-final-contract.zip`](../reviews/packages/zips/arcweft-aw-ah-009.4.1.2.1-tts-runtime-intent-envelope-architecture-reconciliation-final-contract.zip).

```text
outer bytes: 51,138
outer SHA-256: CCF4DA80B64D4C2246EF652C035A46E088505A4DFC1DE702CFD59BCF45A3BB30
members: 13
manifest-covered members: 12/12
TEST_MATRIX rows: 316 unique IDs
OPEN_QUESTIONS.md: exact four bytes "none"
```

The archive has no duplicate or traversing member path. Every manifest length
and SHA-256 matches. The recorded parent request SHA-256 and the prerequisite
AW-AH-009.4.1.2 archive/request SHA-256 values also match the repository files.

The requested external summary, status, and `.zip.sha256` sidecars were not
delivered. The outer hash above is therefore an Arcweft intake measurement, not
an independently supplied producer sidecar. The package records Git
`15cf571416245e1530c0d9902ab3ff6befbdb39e`, which is the inspected `main`, but
its recorded Jujutsu change ID identifies a different Git commit locally. The
package calls out that mismatch rather than hiding it, but it does not satisfy
the parent request's same-checkout Git/Jujutsu evidence requirement.

## Selected architecture retained

The package selects a narrow Sans-I/O `arcweft-audio-tts-runtime` bridge above
`arcweft-core` and `arcweft-audio-tts`. Generic core task intent/outcome and
nominal payload contracts remain audio-agnostic. Ordinary TTS callables lower
one typed pre-selection intent; runtime-driver prepares a fully selected
request before scheduler, pin, replay, or host publication; host code receives
only the selected request and credential slot; progress/result/error return as
typed Need state; AWBC codec 8 directly replaces codec 7; replay schema 1 and
the existing save blockers remain the only persistence paths.

That direction is compatible with the repository's core boundary and does not
introduce a string/JSON envelope, compatibility alias, dual reader, parallel
scheduler, second replay log, removed-syntax diagnostic, or source gate.

## Readiness verdict

The archive's internal `READY_FOR_IMPLEMENTATION` status is not accepted.
Production implementation is blocked by the independently throwable
[`AW-AH-009.4.1.2.1.1 correction request`](../reviews/requests/2026-07-24-aw-ah-009.4.1.2.1.1-tts-runtime-envelope-transaction-and-validation-closure.md).

The exact blocking contract gaps are:

1. `RootReplayError::InvalidExternalTaskPayload` retains only
   `RuntimePayloadContractError`, while nested TTS ordinal/shape/semantic-limit
   and content-digest failures are `TtsPayloadDecodeError`. `REPLAY-016`
   requires digest corruption to stop replay with no Need result, but no typed
   replay carrier can retain that failure.
2. `SAVE-010` requires restored TTS assets to be fully validated before state
   publication. The declared save dependency graph and restore ownership expose
   no bridge decoder callback/API or typed pre-publication failure path.
3. `TtsAcceptedCatalog::rebind_queued_request` receives the candidate catalog,
   previous request, availability, and generation, but the previous request's
   selection evidence lacks three required old coordinates: profile semantic
   digest, credential-ref canonical text, and protocol ID.
4. The driver allocates `TaskSequence` at P9, while `TaskSpec` and
   `RuntimeScheduler::submit_one` have no sequence input. P10 is fallible even
   though P9-P12 are declared one infallible commit, leaving scheduler,
   registry, generation-pin, and sequence atomicity unspecified.
5. Exact outer-cap success rows are not reachable for several domain payloads.
   `TtsProgress` cannot reach 1,024 canonical bytes with three bounded fields;
   a 32 MiB audio body plus bounded metadata cannot reach the declared
   32 MiB + 128 KiB asset cap. The matrix must separate generic codec cap
   evidence from the largest valid domain value.
6. `HostAdapterRegistrationId`, `TtsPreparationSnapshot`, and
   `TtsRuntimeTaskPreparer` have private fields but no authority-preserving
   allocator/constructor path across their owning crates.
7. Core is required to reject exact TTS nominal/outcome mismatches before VM
   execution while also owning no TTS constants or dependency. The generic
   schema/contract registration link that reconciles those requirements is not
   specified.

Each gap changes a public type, dependency edge, serialized identity, error
taxonomy, or publication transaction. It is therefore not safe to infer a
local implementation.

## Dependency and implementation state

The lower AW-AH-009.4.1.2 package remains behind its recorded Lang-01.4 and
Lang-01.5.1 entry gates, and the current checkout does not yet contain the
proposed `arcweft-audio-tts` substrate. Even after those predecessor gates
close, this runtime-envelope package must not enter production until the
AW-AH-009.4.1.2.1.1 correction returns and is re-intaken.

The retained implementation order is therefore:

1. complete the named Lang-01.4/Lang-01.5.1 predecessors;
2. receive and verify AW-AH-009.4.1.2.1.1;
3. reconcile its corrected replay/save/reload/admission/layout APIs with the
   lower TTS package;
4. then implement the complete TTS chain without a provisional envelope or
   compatibility surface.

## Non-goals while blocked

Do not add a TTS-specific core variant, audio dependency in core, guessed
replay error conversion, save callback, compatibility seal, scheduler wrapper,
padding-only payload field, forgeable registration constructor, codec-7 dual
reader, compatibility spelling, CSS/Takumi path, or source gate. The received
archive is retained as inspected design evidence, not as authorization to
implement its unresolved boundaries.
