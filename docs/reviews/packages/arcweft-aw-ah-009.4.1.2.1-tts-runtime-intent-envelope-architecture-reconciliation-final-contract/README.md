# AW-AH-009.4.1.2.1 final contract

```text
SEQUENCE=AW-AH-009.4.1.2.1
STATUS=READY_FOR_IMPLEMENTATION
PRODUCTION_IMPLEMENTATION_PERFORMED=NO
PRODUCTION_FILES_CHANGED=0
CURRENT_MAIN_GIT_COMMIT=15cf571416245e1530c0d9902ab3ff6befbdb39e
REPOSITORY_JUJUTSU_EVIDENCE=zzrlxnsunyxl
CONSUMED_PREREQUISITE_SHA256=cb087cc2e4e137edde1732c11df579a1c71371769633bfdcf807fd367b30fdc1
```

## Purpose

This archive reconciles the accepted AW-AH-009.4.1.2 TTS identity, catalog,
provider, result, error, and adapter contract with the current Arcweft layering
rule that `arcweft-core` remains runtime/data core and has no audio dependency.
It freezes one implementation-ready typed path from ordinary callable lowering
to a prepared scheduler task, typed host dispatch, `Need` observation, save,
replay, and reload. It changes no lower TTS identity, catalog-selection,
fingerprint, provider protocol, or credential contract.

The final executable representation is a nominal `RuntimePayload` owned by the
new narrow composition crate `arcweft-audio-tts-runtime`. Generic intent and
outcome carriers remain in `arcweft-core`; all TTS knowledge stays above core.
Preparation occurs atomically in `arcweft-runtime-driver` before scheduler,
registry, generation-pin, replay-capture, host-dispatch, or provider-I/O
publication. The existing `RuntimeScheduler` remains the only scheduler.

## Consumed evidence

- sole request: SHA-256 `3f37ef7f45dd69cfe7ed70470e943a33ae4824569158243cba1cead38ba65e5e`;
- accepted prerequisite archive: SHA-256 `cb087cc2e4e137edde1732c11df579a1c71371769633bfdcf807fd367b30fdc1`;
- prerequisite archive ZIP integrity: 16/16 entries readable;
- prerequisite internal manifest: 15/15 declared content members matched exact
  byte lengths and SHA-256 values; `MANIFEST.txt` follows the accepted package
  convention and excludes itself;
- prerequisite `OPEN_QUESTIONS.md`: exact bytes `none`;
- current private repository: authenticated GitHub connector, `main` at Git
  commit `15cf571416245e1530c0d9902ab3ff6befbdb39e`;
- exact repository-authored Jujutsu evidence used: protected integration change
  ID `zzrlxnsunyxl`. It is not asserted to be the local change ID of the Git commit,
  because no local `.jj` store was available;
- root `AGENTS.md`, Rust skill, request, intake, current Cargo manifests, core
  payload/task/AWBC owners, runtime-driver, scheduler, host adapter, save,
  replay, bundle, and callable-registry owners were inspected.

## Normative members

- `FINAL_CONTRACT.md`: selected Rust-facing contract and closed decisions.
- `OWNERSHIP_AND_DEPENDENCY_GRAPH.md`: direct Cargo edges, type owners,
  visibility, constructors, and forbidden dependencies.
- `TYPED_LAYOUT_AND_CODEC.md`: exact nominal IDs, layout hashes, field ordinals,
  AWBC codec 8, runtime-value codec, limits, and typed codec errors.
- `EXECUTION_TRANSACTION.md`: source-to-host sequence, atomic preparation,
  scheduler joining, cancellation, progress, result, and error transactions.
- `SAVE_REPLAY_RELOAD.md`: schema-1 replay correction, existing save blockers,
  queued migration, and active-generation rules.
- `IMPLEMENTATION_HANDOFF.md`: ordered cuts, direct-replacement/deletion
  inventory, structural audits, and validation commands.
- `TEST_MATRIX.md`: complete positive, negative, tamper, limit, visibility,
  dependency, replay, reload, and Tier 2 matrix.
- `REQUIREMENTS_TRACEABILITY.md`: request and affected AW-AH-009.4.1.2 rows
  mapped to design and tests.
- `REPOSITORY_EVIDENCE.md`: exact repository and prerequisite evidence.
- `OPEN_QUESTIONS.md`: exact ready-output content.
- `FINAL_STATUS.md`: machine-readable status and verification boundary.

## Integrity convention

`MANIFEST.txt` is sorted by archive member name and hashes every other archive
member. A manifest cannot contain its own stable SHA-256; the adjacent
`.zip.sha256` sidecar authenticates the complete ZIP, including
`MANIFEST.txt`. ZIP member timestamps are fixed to 1980-01-01 and permissions
to `0644` for deterministic rebuilding.

## Handoff

Implementation must consume the accepted AW-AH-009.4.1.2 package first and then
apply this correction as a direct replacement in the order specified in
`IMPLEMENTATION_HANDOFF.md`. No production Rust, Cargo manifest, test, schema,
fixture, or stable chapter was edited while producing this archive. Repository
commands and Tier 2 checks listed here are required implementation gates; they
were not executed by this design-only assignment and are not claimed as passing.
