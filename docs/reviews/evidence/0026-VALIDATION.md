# Validation record

## 1. Artifact scope

This is a design-only archive. Validation distinguishes:

1. source/repository inspection;
2. package mechanical integrity;
3. executable design reference checks; and
4. production implementation/build validation.

Only the first three were performed. No production code was changed or built.

## 2. Actual input checks

| Check | Result |
|---|---|
| source request readable in full | PASS |
| source request SHA-256 | `dc9d39578e4706b7b518bc2cfdd37fda33d6be38352007c957e2360704afcf76` |
| project premise readable | PASS |
| Rust Skill read through final line | PASS |
| Rust Skill SHA-256 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` |
| latest root/nested AGENTS read | PASS |
| inspected main recorded | `d8fbeaa5757fe5836fba17fca35fa104eeb72a1d` |
| accepted classifier recorded | `b76465c128322be2d5e66398bc6c30794ca0276f` |
| parent archive hashes copied from exact request | PASS |
| parent binary ZIPs re-extracted in this build | NOT PERFORMED; not mounted |

## 3. Executable design-model check

Command actually run:

```text
python3 validation/model_checks.py
```

Actual result:

```json
{"checks": 77, "scope": "design reference model only", "status": "PASS"}
```

The 77 checks cover:

- monotonic/exhausted cursors;
- cursor restore continuation, including retired-maximum exhaustion;
- compatible repeated Copy-source normalization and conflicting participants;
- anonymous/nominal record IDs;
- manual path and owner-slot ordering;
- preparation precedence;
- source-preserving Move preparation;
- commit mismatch before take;
- one-active domain behavior; and
- machine-readable codec golden lengths/bytes.

This script is not Arcweft production code and does not satisfy Cargo or
integration gates.

## 4. Package semantic inventory checks

The sealed package validator checks:

- required member presence;
- path safety and forbidden production-overlay suffixes;
- exact `OPEN_QUESTIONS.md == b"none\n"`;
- exact source-request hash;
- READY/zero-open/no-implementation status markers;
- all request-named Rust symbols;
- machine-readable symbol closure uniqueness;
- JSON validity and golden byte lengths;
- at least 400 unique normative test rows;
- all required test kinds/prefixes;
- manifest coverage/digests; and
- final ZIP CRC, lexical order, timestamp, mode, and byte parity.

Expected final counts, verified after sealing:

```text
archive members=26
normative test rows=438
symbol closure entries=108
decision register entries=72
valid codec goldens=23
invalid binary codec vectors=18
invalid JSON codec vectors=15
reference model checks=77
open questions=0
```

## 5. Python syntax check

Actually run before sealing:

```text
python3 -m py_compile validation/model_checks.py validation/validate_package.py
```

Result: `PASS`.

Generated `__pycache__` was removed and is not archived.

## 6. ZIP checks

The final ZIP is created deterministically with:

- lexical member order;
- timestamp `1980-01-01 00:00:00`;
- Unix mode `0644`;
- Deflate compression;
- no explicit directory entries; and
- no absolute/backslash/`..` paths.

After final sealing, `validation/validate_package.py` is run against both the
package directory and ZIP. Required result:

```text
CRC=PASS
PATH_SAFETY=PASS
MEMBER_BYTE_PARITY=PASS
MANIFEST=PASS
DETERMINISTIC_REBUILD=PASS
```

The actual outer archive SHA-256 is reported in the delivery response because a
ZIP cannot contain its own final cryptographic hash without self-reference.

## 7. Manifest model

`MANIFEST.txt` is sorted by relative path:

```text
<64-lowercase-sha256>  <relative/path>
```

The `MANIFEST.txt` self-entry is 64 zeroes. Every other digest covers exact
archived bytes. The package validator recalculates every row.

## 8. Deliberately not performed

```text
PRODUCTION_IMPLEMENTATION=NO
LOCAL_ARCWEFT_CHECKOUT=NO
PRODUCTION_PATCH=NO
CARGO_FMT_RUN=NO
CARGO_CHECK_RUN=NO
CARGO_CLIPPY_RUN=NO
CARGO_TEST_RUN=NO
JUST_GATES_RUN=NO
TIER2_RUN=NO
NATIVE_RUNTIME_RUN=NO
WEB_RUNTIME_RUN=NO
AGENT_RUNTIME_RUN=NO
RUST_TARGET_DECLARATIONS_COMPILED=NO
PARENT_ZIP_MEMBER_REHASH_IN_THIS_BUILD=NO
```

These are implementation-time requirements in `IMPLEMENTATION_ORDER.md` and
`FUL-*` rows. They are not presented as passes.

## 9. Verification confidence by artifact content

| Content | Verification level |
|---|---|
| request reproduction/hash | exact bytes/hash |
| repository policies/current source findings | read-only source evidence |
| design decision closure | internal traceability/symbol/model checks |
| Rust target shape | reviewed text/symbol closure; not production-compiled |
| codec goldens | generated and byte-length checked |
| test matrix | schema/uniqueness/coverage checked; not executed on production |
| ZIP/manifest | mechanically validated after sealing |
| production behavioral correctness | pending implementation/full gates |

## 10. Reproduction

After extraction:

```text
python3 validation/model_checks.py
python3 validation/validate_package.py .
```

To validate the delivered ZIP from its parent directory:

```text
python3 extracted/validation/validate_package.py extracted \
  --zip arcweft-lang-01.3.1.2.3.2-generic-ownership-identity-and-slot-reconciliation-correction-final-contract.zip
```
