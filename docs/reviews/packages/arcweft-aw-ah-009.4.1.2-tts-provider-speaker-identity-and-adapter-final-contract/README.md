# AW-AH-009.4.1.2 final contract package

```text
STATUS=READY_FOR_IMPLEMENTATION
OPEN_QUESTIONS=0
IMPLEMENTATION_PERFORMED=NO
PRODUCTION_CHANGES=0
GIT_REF=main
GIT_COMMIT=e6e8cce33d4c09a9f9efa9ba2169fc5c6b0b7139
REQUEST_SHA256=4cd740e664528ac2a033f02245e6e0c5f4d887fdfdbbb877584b9fe742727b99
```

This archive is the independent, implementation-ready contract for TTS
provider-speaker identity and adapter dispatch. The attached request Markdown is
the sole normative request. Repository documents are evidence about current
production, not competing requirements.

## Final selection

The production design adds one lower Sans-I/O crate, `arcweft-audio-tts`, for
nominal TTS identities, accepted catalog records, request/result/error data, and
canonical codecs. It does not perform I/O. Existing Task/Need/cancellation,
manifest publication, generated adapter metadata, host-adapter dispatch,
audio decoding, mixing, device output, save blocking, hot-reload generation
pinning, and replay injection remain the execution substrate.

Provider SDK, process, network, credential, clock, retry, and rate-limit work is
owned only by host adapters. Dialogue/View character projection remains valid
and executable with no TTS provider, catalog, credential, or adapter.

The current provisional `TtsRequest { voice: Option<String>, text: String }`
and the `tts.synthesis` spelling are unreleased stringly substrate. They are
replaced directly; no alias, compatibility shim, dual reader, source gate,
removed-name diagnostic, CSS path, or Takumi path is retained.

## Archive map

| File | Purpose |
|---|---|
| `FINAL_CONTRACT.md` | Normative decision summary and invariants. |
| `OWNERSHIP_AND_DEPENDENCY_GRAPH.md` | Crate/module owners and allowed dependency directions. |
| `IDENTITY_AND_MAPPING_MODEL.md` | Exact nominal types, mappings, selection, conflicts, and reload rules. |
| `SOURCE_RESOURCE_AND_MANIFEST_MODEL.md` | Typed `res`, ordinary functions, sole manifest path, and generated metadata extension. |
| `REQUEST_RESULT_AND_ADAPTER_PROTOCOL.md` | Sans-I/O intent/request/result/error records and host adapter behavior. |
| `WIRE_VERSION_AND_LIMITS.md` | Canonical wires, discriminants, budgets, rejection rules, bundle/save/replay behavior. |
| `CAPABILITY_PRIVACY_AND_DIAGNOSTICS.md` | Capability policy, secret handling, projection rules, and stable diagnostics. |
| `IMPLEMENTATION_ORDER.md` | Eight coherent implementation cuts with entry and exit gates. |
| `TEST_MATRIX.md` | Complete behavior, codec, adapter, source, privacy, and dependency matrix. |
| `DELETION_INVENTORY.md` | Direct-removal inventory and preserved substrate. |
| `REPOSITORY_EVIDENCE.md` | Read-only inspection ledger pinned to current `main`. |
| `REQUIREMENTS_TRACEABILITY.md` | Every request requirement mapped to contract sections and tests. |
| `OPEN_QUESTIONS.md` | Exactly `none`. |
| `FINAL_STATUS.md` | Machine-readable readiness and verification boundary. |
| `MANIFEST.txt` | Member size and SHA-256 inventory. |

## Verification boundary

The repository was inspected through the authenticated GitHub connector at the
commit above. The request and Rust skill were read from the supplied files.
There was no local checkout, no production edit, and no Cargo, `just`, Tier 2,
or structural-audit execution. `REPOSITORY_EVIDENCE.md` records what was and
was not verified. `TEST_MATRIX.md` is the required implementation test plan,
not a claim that those tests have already run.

## Normative language

`MUST`, `MUST NOT`, `SHOULD`, and `MAY` are normative. Every identifier, limit,
wire coordinate, selection order, diagnostic code, and implementation cut in
this archive is closed. A later implementation may change a selected value only
through a new result-changing design request; it may not substitute an
unrecorded alternative during implementation.
