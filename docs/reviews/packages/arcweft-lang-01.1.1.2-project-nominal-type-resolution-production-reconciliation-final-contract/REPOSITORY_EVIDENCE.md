# REPOSITORY EVIDENCE

## 1. Audit identity

```text
repository: Sanzentyo/arcweft
branch: main
audited head: 23ed5d93824630d8ead9092d32f7fc70f0a8f314
head subject: Move View and image products into compiler ownership
request preparation baseline: c56c82240dacc0d254c7d32e17359d4be0f04b41
baseline comparison: head is 41 commits ahead, 0 behind
final head recheck: 23ed5d93824630d8ead9092d32f7fc70f0a8f314
audit date (Asia/Tokyo): 2026-07-21
access path: authenticated GitHub connector
production writes: none
```

The repository is private. Evidence was fetched through the configured GitHub
connector rather than inferred from public search results.

## 2. Governing inputs

| Input | Verification |
|---|---|
| Attached request Markdown | read in full; SHA-256 `c941ba223dc88f6958c59a6cf83295778b6958e4737c08fc8d5d3b44c88faf77` |
| Root `AGENTS.md` at audited head | read in full; Git blob `ea4a46132ff8cd004f860c89c854e4cbfe807d86` |
| Attached Rust skill | read in full through final line; SHA-256 `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` |
| Arcweft premise file | read in full; SHA-256 `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` |
| Named predecessor contract ZIP | bytes not supplied; dispatch SHA retained only as metadata: `0f02ac8c7b0ed405d036dfb75148998c7980070a0f2e6a440f8feb886d02121c` |

The request Markdown is the only task specification. Repository evidence is
used to reconcile that specification with current production ownership and
successful paths.

## 3. File-level evidence

| Path at `23ed5d93824630d8ead9092d32f7fc70f0a8f314` | Blob SHA where captured | Finding used by contract |
|---|---|---|
| `AGENTS.md` | `ea4a46132ff8cd004f860c89c854e4cbfe807d86` | Layering, typed API, Sans I/O, ownership, validation, no ad-hoc compatibility mechanisms |
| `Cargo.toml` | connector-inspected | Workspace/crate inventory and dependency architecture |
| `crates/arcweft-lang-syntax/src/types.rs` | `34168bcb722cdb6b112ec2185fa7027af11d4476` | `TypeRef` paths/generic bases are strings; context lacks identity/source map |
| `crates/arcweft-lang-syntax/src/ast/items.rs` | `1b8da69f8a565cc97f1e4aeb2177232a3bf146fa` | Alias generic/source loss; enum payload strings; authored owner ranges |
| `crates/arcweft-lang-syntax/src/parser/type_declaration_grammar.rs` | connector-inspected | Lossless grammar already recognizes declaration generics and typed roles |
| `crates/arcweft-lang-syntax/tests/parser_function_signatures_and_types.rs` | `56c2178fbf79a4be824fb2d0e96684a14aa83f0b` | Generic signatures and open nominal grammar fixtures; source names currently preserved as paths |
| `crates/arcweft-lang-hir/src/model.rs` | connector-inspected | Module HIR carries top-level struct/enum/alias syntax records |
| `crates/arcweft-lang-hir/src/project.rs` | connector-inspected | Module-preserving `HirProject`, source identities, transitional flattening |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | connector-inspected | Current world/revision/callable/external identity and missing nominal variants |
| `crates/arcweft-lang-hir/src/symbol/table.rs` | `aea7b97f8052a1c7967d2cdcb7b028204b13e779` | Unified bounded table, fixed-point imports, visibility, BTree determinism, silent unknown-import omission |
| `crates/arcweft-lang-hir/src/symbol/error.rs` | `b9aaa2c85d3295cd6c509e0b7e6f8f4fd888929b` | Existing structured link error/code/report owner |
| `crates/arcweft-lang-hir/Cargo.toml` | connector-inspected | HIR depends below sema and can own project records without a cycle |
| `crates/arcweft-lang-sema/src/types.rs` | `faa33a4ddc25ea3c1a1a434ec30692c37575a6d0` | `Named` fallback, string generic IDs, and production `ArcResult` spelling branch |
| `crates/arcweft-lang-sema/src/env/base.rs` | connector-inspected | Standard/domain names, nominal records, enum inventories, Rust package facts |
| `crates/arcweft-lang-sema/src/registration/model.rs` | `27f85befd39ffbc3920424145b0ddaddc774cf6a` | Exact registered world/revision and external character/environment owner mapping |
| `crates/arcweft-lang-sema/src/checker/helpers.rs` | connector-inspected | Normal checker context-free `type_ref_kind*` success paths |
| `crates/arcweft-lang-sema/src/checker/signature.rs` | connector-inspected | Signature conversion uses context-free type projection |
| `crates/arcweft-lang-sema/src/checker/module.rs` | connector-inspected | String alias maps/erasure and string-keyed nominal structural maps |
| `crates/arcweft-lang-sema/src/entry/checker/nominal.rs` | connector-inspected | Entry-only declaration/import/alias resolver, cycles, payload reparsing |
| `crates/arcweft-lang-sema/src/entry/checker/contract.rs` | connector-inspected | Duplicate canonical type conversion and `ArcResult` constructor |
| `crates/arcweft-lang-sema/src/project_index.rs` | connector-inspected | Context-free type conversion; project struct/enum/alias indexing omitted |
| `crates/arcweft-lang-sema/Cargo.toml` | connector-inspected | Correct HIR/syntax/source/domain dependency direction |
| `crates/arcweft-source/src/document.rs` | connector-inspected | Revision-bound UTF-8 `SourceSpan` creation and validation |
| `crates/arcweft-source/src/diagnostic.rs` | connector-inspected | Structured diagnostics with primary/secondary labels |
| `crates/arcweft-lsp/src/profiles/accepted_project.rs` | connector-inspected | Immutable accepted HIR/source/symbol/environment transaction and exact stale checks |
| `docs/implementation/2026-07-20-lang-01.1.1-await-source-slice.md` | `1c400fe372fb178d51642761a6b6d677bf803198` | Typed Await source substrate and explicit pending nominal prerequisite |
| `docs/implementation/2026-07-21-lang-01-1-1-1-selected-contract-gap-audit.md` | connector-inspected | Current selected Try contract gap evidence |
| `docs/reviews/requests/2026-07-20-lang-01.1.1.1-prefix-postfix-try-source-and-propagation-contract-correction.md` | `11fb5bfd29e18c87520fc5de7a7d20b0355ec486` | Upstream Try/boundary decisions that this contract preserves |

## 4. Production behavior conclusions

1. One existing project symbol transaction is suitable for nominal
   declarations; an adjacent project catalog would duplicate module/import
   authority.
2. HIR can own IDs and source records without depending on sema.
3. Sema must own recursive conversion because project selection alone cannot
   classify generics, built-ins, `Self`, projections, environment evidence, or
   semantic `TypeKind`.
4. Normal checking and entry checking currently have disagreeing successful
   paths; both must migrate and delete the old paths.
5. The accepted environment already proves exact world/revision and external
   ownership; it should be extended, not replaced.
6. Exact nested type source evidence requires a typed syntax/HIR correction;
   downstream rescanning is prohibited.
7. The `ArcResult` and arbitrary `Named` branches are demonstrably current
   production behavior and are explicitly removed by the final contract.
8. Unknown imports are currently omitted by the linker and need the specified
   unknown/cycle classification.

## 5. What was not claimed

- No production file was changed.
- No Cargo build/test/Clippy/Tier 2 command was run for a design-only package.
- No unprovided predecessor ZIP content was inspected.
- No runtime, wire, CSS, Takumi, rendering, or host behavior was inferred.
- No source-text search was used as an acceptance gate.
