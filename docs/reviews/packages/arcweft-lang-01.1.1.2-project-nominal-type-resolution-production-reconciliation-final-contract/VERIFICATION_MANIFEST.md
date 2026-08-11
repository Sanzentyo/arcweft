# VERIFICATION MANIFEST

## 1. Artifact-generation verification

| Check | Result |
|---|---|
| Attached request read to final line | PASS |
| Root `AGENTS.md` at `23ed5d93824630d8ead9092d32f7fc70f0a8f314` read to final line | PASS |
| Attached Rust skill read to final line | PASS |
| Latest `main` rechecked immediately before generation | PASS — `23ed5d93824630d8ead9092d32f7fc70f0a8f314` |
| Request baseline compared with audited head | PASS — head 41 commits ahead, 0 behind |
| Production repository edited | NO |
| Implementation performed | NO |
| Required archive entries present | verified by generator |
| `OPEN_QUESTIONS.md` exact bytes | verified as `none` |
| Test IDs unique | verified by generator |
| Test-matrix rows | 242 |
| Decisions 1–14 traceable | verified by artifact lint |
| Required TM/RD IDs present | `TM-072`, `RD-084`, `TM-074`, `TM-080-*`, `TM-083*` |
| Manifest hashes/sizes verified before ZIP | verified by generator |
| ZIP entry order/timestamps deterministic | verified by generator |
| ZIP extraction byte equality | verified by generator |
| Production Cargo/test/Clippy/Tier 2 execution | NOT RUN — no implementation or production checkout mutation was requested |

“Not run” above is a scope fact, not a waiver. The commands below are mandatory
for the implementation cut.

## 2. Input hashes

```text
sha256 c941ba223dc88f6958c59a6cf83295778b6958e4737c08fc8d5d3b44c88faf77  2026-07-20-lang-01.1.1.2-project-nominal-type-resolution-production-reconciliation.md
sha256 1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665  Rust Skill.txt
sha256 cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1  前提(Sanzentyo-arcweft).txt
git-blob ea4a46132ff8cd004f860c89c854e4cbfe807d86  AGENTS.md@23ed5d93824630d8ead9092d32f7fc70f0a8f314
dispatch-only sha256 0f02ac8c7b0ed405d036dfb75148998c7980070a0f2e6a440f8feb886d02121c  2026-07-20-lang-01.1.1.1-implementation-ready-final-contract.zip
```

The predecessor ZIP bytes were not supplied and therefore its dispatch hash
was not independently recomputed.

## 3. Mandatory focused implementation verification

### Syntax and HIR

```text
cargo fmt --all -- --check
cargo check -p arcweft-lang-syntax --all-targets
cargo check -p arcweft-lang-hir --all-targets
cargo test -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-lang-hir --all-targets
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo clippy -p arcweft-lang-hir --all-targets -- -D warnings
```

### Sema and entry

```text
cargo check -p arcweft-lang-sema --all-targets
cargo test -p arcweft-lang-sema nominal --all-targets
cargo test -p arcweft-lang-sema entry --all-targets
cargo test -p arcweft-lang-sema --all-targets
cargo clippy -p arcweft-lang-sema --all-targets -- -D warnings
```

### Compiler and LSP

```text
cargo check -p arcweft-compiler --all-targets
cargo check -p arcweft-lsp --all-targets
cargo test -p arcweft-compiler --all-targets
cargo test -p arcweft-lsp --all-targets
cargo clippy -p arcweft-compiler --all-targets -- -D warnings
cargo clippy -p arcweft-lsp --all-targets -- -D warnings
```

## 4. Mandatory repository-wide implementation verification

```text
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo metadata --format-version 1 --no-deps
git diff --check
```

Run all applicable Tier 2 commands prescribed by the then-current root
`AGENTS.md`. If `main` moves before implementation, re-read the entire
`AGENTS.md`, compare the audited head to the new head, and re-audit every
changed owner path before applying this contract.

## 5. Structural audit requirements

The structural audit must prove through typed APIs, compiler errors, Cargo
metadata, and behavior tests:

1. HIR has no dependency on sema.
2. `arcweft-core`, runtime wire, CSS, Takumi, and rendering have no new nominal
   dependency.
3. The project table is the only project/import authority.
4. Entry and LSP have no successful project resolver.
5. No authored `TypeRef` reaches context-free `TypeKind` conversion.
6. No `ArcResult`/`Unknown` spelling branch exists.
7. No old/new field pair, compatibility alias, dual reader, extension-trait
   shim, or legacy semantic-index reader remains.
8. Identity/navigation never parses display/diagnostic text.
9. Open-name acceptance is represented by typed catalog rules.
10. Every `TEST_MATRIX.csv` row reports pass.

A source grep may assist a human audit, but it is not an acceptance gate and
must not substitute for typed behavior or dependency proof.

## 6. ZIP verification algorithm

The generator performs:

1. normalize every Markdown file to UTF-8 LF;
2. require the exact mandatory file-name set;
3. require `OPEN_QUESTIONS.md == b"none"`;
4. parse `TEST_MATRIX.csv` with the standard CSV reader;
5. require all columns and unique/nonempty `test_id`s;
6. require all mandatory TM/RD rows;
7. calculate SHA-256 and size for every non-manifest file;
8. write `MANIFEST.txt`;
9. re-hash each file against the manifest;
10. create a ZIP with sorted paths and fixed `1980-01-01 00:00:00` timestamps;
11. reopen the ZIP, validate path order and required names, and compare every
    extracted byte with its source file.

The final response reports the independently calculated ZIP SHA-256.
