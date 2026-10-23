# Validation record

```text
PACKAGE_VALIDATION_STATUS=PASS
CONTRACT_STATUS=FINAL
OPEN_QUESTIONS=0
FALLBACK=NO
PRODUCTION_CODE_CHANGED=NO
REPOSITORY_AWARE_BASELINE=5821a3ca479b5b89ca6ede997b9cf4f42f6280a6
```

## 1. Repository-aware validation actually performed

The private repository was read through the GitHub connector. No GitHub mutation,
branch, commit, pull request, repository checkout write, or production file edit
was performed.

The final baseline is `main` commit
`5821a3ca479b5b89ca6ede997b9cf4f42f6280a6`. During construction, `main` advanced
by two commits from the initial observation. The commit comparison was inspected,
the root `AGENTS.md` was reread in full at blob
`e91f99213dde67953beda6aa078c370a8dc4541d`, and changed relevant HIR/RuntimePlan
owners were reread. Core value/AWBC/fiber/swap/save/step and callable coordinate/
limit owners were unchanged by those two commits.

The uploaded request and final-main request were verified as the same Git blob:

```text
6d24910f7961c56faaffddea5cfa6775b48578a1
```

Repository evidence was checked for:

- shared `(group, parameter)` coordinates and accepted call facts;
- nested callable schemas and production limits;
- current flat closure partial behavior and its inability to retain groups;
- current ABI-1/codec-7 table/tag boundaries;
- FiberState generation/cursor ownership;
- save traversal and hot-reload generation pins;
- host Sans-I/O and decimal-string integer policy; and
- compiler/core/sema dependency directions.

The selected contract was reconciled against those owners and the latest AGENTS
rules: behavior is placed on owning types/enums, the compiler is the sole sema to
core projection boundary, no compatibility layer is added, and acceptance relies
on typed/codec/behavior/dependency evidence rather than a repository source gate.

## 2. Package validation actually performed

The standard-library validator verifies structured contract metadata, input
hashes, Git-blob identity, UTF-8 text hygiene, Cargo metadata for the standalone
model, responsibility-module size bounds, the complete paired Markdown/JSON test
matrix, strict host JSON behavior, and the SHA-256 manifest when present.

```bash
PYTHONDONTWRITEBYTECODE=1 python3 validation/verify_contract.py
```

Result: PASS. The matrix contains 168 unique required cases:

```text
positive/parity=44
negative=45
AWBC tamper=30
host JSON=16
save/restore=14
hot reload=10
architecture/deletion=9
```

The host fixture validator was also run directly:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 host/validate_host_fixtures.py
```

Result: PASS. Four canonical valid fixtures are byte-identical across common,
native, Web, and Agent files. Duplicate-field and unknown-field fixtures are
rejected.

The package uses no symlinks, no Python cache files, no external Python package,
and no model dependency. Its model is split into a 30-line facade and
responsibility modules below the package audit threshold.

## 3. Integrity validation

- `manifest.json` records every payload file's byte length and SHA-256.
- `MANIFEST.sha256` covers every package file except itself.
- The final ZIP is built in sorted path order with fixed timestamp
  `2026-07-22T00:00:00`, normalized regular-file modes, UTF-8 names, and deflate
  level 9.
- The final outer ZIP is tested with `unzip -t`.
- The ZIP SHA-256 is supplied as a sibling `.sha256` sidecar because an archive
  cannot contain a stable hash of itself.

## 4. Rust execution boundary not claimed

No `rustc`, `cargo`, `rustfmt`, or `cargo-clippy` executable was installed in the
artifact runtime, and outbound DNS for Rust installation was unavailable. The
following commands therefore were not executed here:

```bash
cargo fmt --manifest-path model/Cargo.toml -- --check
cargo test --manifest-path model/Cargo.toml
cargo clippy --manifest-path model/Cargo.toml --all-targets -- -D warnings
```

This limitation is recorded rather than converted into a false pass. The model is
included as reviewable, dependency-free executable evidence for an environment
with Rust installed. The final contract does not depend on the model compiling to
resolve any design choice: all tags, owners, ordering rules, rejection behavior,
versions, and tests are frozen independently in the normative documents and
structured `contract.json`.

The Arcweft workspace was not built or tested because production code was
intentionally not changed and no writable repository checkout was used. This
package claims repository-aware contract validation, not implementation
validation of changes that do not yet exist.

## 5. Completion statement

Every required decision in the request has one normative answer. There is no
fallback branch, provisional opcode, optional flat carrier, migration reader, or
unresolved implementation policy in this package.
