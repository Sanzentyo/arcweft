# Verification report

## Result

- Contract status: **implementation-ready**
- `OPEN_QUESTIONS=0`
- Production implementation performed: **false**
- Package construction status: **verified deterministic ZIP**
- Integration base re-confirmed on 2026-07-21 (Asia/Tokyo):
  `Sanzentyo/arcweft` `main` at
  `23ed5d93824630d8ead9092d32f7fc70f0a8f314`
- Sole request SHA-256: `b8ec6b3efcbe31739b2d2b7bad119c2a9dcc394b95777941c4210fc485315f64`

“Implementation-ready” applies to the closed design contract. It does not claim that the
subsequent Rust implementation already exists or has passed Cargo validation.

## Instructions and normative basis read

1. The complete local Rust skill file was read through its final line before package work.
2. The project premise file was read.
3. The exact root `AGENTS.md` at the integration base was read through line 447; its blob
   identity is `ea4a46132ff8cd004f860c89c854e4cbfe807d86`.
4. `SOURCE_REQUEST.md` is the sole request specification. Its byte hash matches
   `REQUEST_COVERAGE.md` and `CONTRACT_MODEL.json`.

## Repository inspection actually performed

The private repository was inspected statically through the configured GitHub connector.
The branch head was re-queried and remained the stated integration base. The exact paths,
blob identities, implemented substrate, concrete defects, and proposed changes are listed
in `REPOSITORY_EVIDENCE.md`. The inspection included the shared callable schema/resolver,
effect inventory, Stream/Source core, RuntimePlan, RuntimeStep, FiberState, AWBC schema,
codec, verifier, bundle, save, and accepted manifest/profile owners.

No repository source file was modified. No compatibility shim, dual reader/writer,
source gate, endpoint DTO, CSS path, or Takumi path was introduced.

## Package checks actually executed

- 15 pre-manifest payload files decoded as UTF-8, contained no BOM
  or CRLF, and ended in LF.
- `CONTRACT_MODEL.json` and `TEST_MATRIX.json` parsed successfully.
- `CONTRACT_MODEL.json` reports `open_questions = 0`, the exact integration base, and the
  closed ABI/codec/bundle/save allocation.
- All 12 required decisions and all 9 required implementation-order steps have explicit
  coverage rows.
- `TEST_MATRIX.md`, `TEST_MATRIX.json`, and `TEST_MATRIX.csv` contain the same 530
  records in the same order, with unique stable IDs and contiguous per-prefix numbering.

| Test prefix | Count |
| --- | ---: |
| `ABI` | 48 |
| `CALL` | 27 |
| `DEL` | 21 |
| `DROP` | 36 |
| `EFF` | 13 |
| `EXH` | 34 |
| `JSON` | 36 |
| `OWN` | 35 |
| `PLAN` | 24 |
| `PROF` | 126 |
| `RPL` | 79 |
| `SAVE` | 51 |

- Six worked host/save JSON blocks parsed without duplicate keys and re-encoded byte-for-byte
  as compact canonical JSON.
- All Markdown code fences are balanced.
- Three worked BLAKE3-256 vectors were independently recomputed:

| Vector | Transcript bytes | BLAKE3-256 |
| ---: | ---: | --- |
| 1 | 394 | `06c9bbdb24b2d49e046351ed1abc232b56a36f2fcbb7001cbe5c88342f7725dd` |
| 2 | 155 | `947ea4e1562aaadb16a5541cb6f9d229a9ebbe451e9aa40dda0564e30f9a14eb` |
| 3 | 232 | `7ee9a40fbd4025897267158027764cca41dba79dce5b56d14fec7283764600b3` |

## Manifest and ZIP checks

`MANIFEST.json` records the byte count and SHA-256 of every payload artifact, including
this report. `MANIFEST.sha256` uses standard `sha256sum` syntax and covers every package
file except itself, including `MANIFEST.json`. Its self-exclusion is deliberate: a file
cannot contain its own ordinary SHA-256 without recursion.

The final ZIP is produced with:

- one top-level package directory;
- lexicographically sorted entries;
- fixed DOS timestamp `1980-01-01 00:00:00`;
- Unix file mode `0644` and directory mode `0755`;
- DEFLATE level 9;
- no host filesystem metadata.

The build procedure creates the archive twice and requires byte-for-byte equality. It then
extracts the final archive into a new temporary directory, rejects unsafe paths, compares
every extracted payload byte-for-byte with the package source, validates every
`MANIFEST.json` entry, and re-runs every `MANIFEST.sha256` digest. The external
`*.zip.sha256` file records the final archive digest without creating a recursive in-archive
ZIP hash.

## Verification intentionally not claimed

The artifact environment did not contain a local checkout of the private repository or a
Rust toolchain. Therefore the following implementation gates were **not run** here:

- `cargo check`, focused or workspace tests;
- `cargo clippy` or `cargo fmt --check`;
- native/web/Agent integration execution;
- `cargo +nightly -Zscript tools/structure-audit.rs --root .`.

Those are ordered implementation-review gates in `IMPLEMENTATION_ORDER.md` and
`TEST_MATRIX.md`; they are not package-generation prerequisites because the request
explicitly forbids production implementation in this task.

## Final boundary

The package is internally integrity-checked and closes every public/runtime/wire design
decision requested by Lang-01.3.1.2.1. Subsequent implementation must re-read the then-current
`AGENTS.md` and re-confirm numeric allocations immediately before the protected ABI 2 /
codec 8 / bundle 6 / save 2 merge unit. A collision changes only the newly allocated value
and all package vectors atomically; it never creates an alias or predecessor reader.
