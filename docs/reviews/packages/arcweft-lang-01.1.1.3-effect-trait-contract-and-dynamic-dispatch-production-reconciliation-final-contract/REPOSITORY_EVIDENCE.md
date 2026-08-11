# Repository evidence

## 1. Inspected repository identity

```text
REPOSITORY=Sanzentyo/arcweft
BRANCH=main
GIT_COMMIT=0b7e095f4193b9f7fbbc95cc350a626a8a63640a
COMMIT_TITLE=Make Stream reconciliation request independently throwable
PUSHED_COMMIT_DATE=2026-07-25T02:04:16Z
AGENTS_BLOB=e91f99213dde67953beda6aa078c370a8dc4541d
```

The latest pushed-main query was repeated immediately before package drafting
and returned the same commit.

### Jujutsu identity

The private repository was inspected through the GitHub connector. GitHub's
pushed commit/REST object exposes the Git SHA, tree, parents, message, files, and
diff, but not Jujutsu's extra `change-id` commit header. Repository search did
not contain a trailer mapping this commit to a Jujutsu change. Therefore:

```text
JUJUTSU_CHANGE_ID=NOT_EXPORTED_BY_PUSH_SURFACE
```

No older or guessed change ID is presented as matching. In a local Jujutsu
checkout, the exact mandatory lookup is:

```bash
jj log -r 'git_commit(0b7e095f4193b9f7fbbc95cc350a626a8a63640a)' \
  --no-graph -T 'change_id ++ "\n"'
```

The result must be recorded in the implementation intake note before the first
production commit. This is a truthful repository-evidence transport limitation,
not an unresolved semantic decision in this contract.

## 2. Supplied input verification

```text
REQUEST_SHA256=65b912d18765c24fcad7f195ef4a6914992fd28b220ec4fc11043e04e9ee7330
PARENT_ZIP_SHA256=ed469929680ddeb2c656577d2a049f0d8954b085fd20e2281291630974e01930
RUST_SKILL_LINES=56
```

The parent ZIP passed `unzip -t`. Its internal `MANIFEST.sha256` listed twelve
members; every listed hash matched. All parent documents and all 200 CSV test
rows were read. The complete supplied Rust skill was read through its final
line.

## 3. Inspected current files and observed boundaries

| Path | Blob SHA | Observed current boundary |
|---|---|---|
| `AGENTS.md` | `e91f99213dde67953beda6aa078c370a8dc4541d` | typed ownership/dependency direction; no source gates; direct replacement; workspace/Clippy/Tier 2 policy |
| `docs/01-language/syntax.md` | `160cab...` | current trait syntax; no dynamic-object production owner |
| `docs/01-language/traits-seq-ranges.md` | `7f8b5bdc9edda350c94e2b7df7310842c63c4e01` | static trait/witness substrate; dynamic trait objects deferred |
| `docs/implementation/2026-07-22-lang-01-1-1-direct-style-suspension-generator.md` | `cb0455...` | trait effect contract remained blocked in that implementation cut |
| `docs/implementation/2026-07-24-lang-01-1-1-suspension-diagnostics.md` | `3dcf85...` | typed Await diagnostics landed; trait effects still unresolved |
| `crates/arcweft-source/src/document.rs` | `e1b1a545d28f62704a7e7b517620b85b6ffe73b6` | `SourceDocumentIdentity`, `SourceRevision`, `SourceSetRevision`, and exact `SourceSpan` are revision-bound |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | `6ac5696f8f5c1296dd64f7fcdac7d048b3c7227f` | current `CallableDeclarationId`; owner inherent behavior; project world/revision and unified declaration IDs |
| `crates/arcweft-lang-hir/src/model.rs` | `ec952a...` | HIR retains trait/impl items and project source mapping |
| `crates/arcweft-lang-hir/src/symbol/table/publication.rs` | `ba871d...` | ordinary callable publication exists; methods are not yet project callable symbols |
| `crates/arcweft-lang-syntax/src/ast/flow.rs` | `22303fb950216ac9a00a5556c0ecd252790d81ea` | source-less `ContractClause` enum |
| `crates/arcweft-lang-syntax/src/parser/headers.rs` | `48ba1aead4aa4cb3a351a9bf33cb4e7ba8e1c4bb` | existing contract clause grammar and `effects`/`no_effect` parsing |
| `crates/arcweft-lang-syntax/src/parser/items.rs` | `a7cfecf036a2f3098266fc4b6efa5f78106fa6aa` | ordinary source retention; trait/impl method parser lacks contract/source ownership |
| `crates/arcweft-lang-sema/src/traits.rs` | `056e66e39d76f2c5739777177e841e72ca90d77a` | typed local trait/witness handles; requirement/impl records have no effect owner |
| `crates/arcweft-lang-sema/src/traits/builder.rs` | `0cf7fd5b69764e821aadae5bf1494d89483fc473` | trait/impl collection and signature conformance; no effect conformance |
| `crates/arcweft-lang-sema/src/traits/catalog.rs` | `df7961...` | static witness/inherent lookup; inherited cloning and requirement-as-impl projection |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `6a97f5...` | `TraitCallableId` uses trait path/method/local impl index |
| `crates/arcweft-lang-sema/src/callable/resolver.rs` | `81aef2...` | resolved trait methods currently receive hard-coded closed-empty effects |
| `crates/arcweft-lang-sema/src/callable/schema.rs` | `776142...` | project schema currently carries a copied declared row |
| `crates/arcweft-lang-sema/src/callable/facts.rs` | `12ab3b...` | exact call-target facts are the correct tooling projection layer |
| `crates/arcweft-lang-sema/src/effect_row.rs` | `5e7a34...` | canonical closed/open/unknown row and substitution owner already exists |
| `crates/arcweft-lang-sema/src/effect_contract.rs` | `7c32218cb603f7e00c6349497ddd87b07c14b6c9` | lowers existing clauses but has no exact clause-source binding |
| `crates/arcweft-lang-sema/src/effect_collector.rs` | `8eff3680ff87869914b8eff3e9fb6cdeb478b0ad` | integrated one-pass collector, but keyed by string/source-name and legacy callable ID |
| `crates/arcweft-lang-sema/src/effect_analysis.rs` | `ee6e92...` | existing fixed point and current trace selection |
| `crates/arcweft-lang-sema/src/effect_diagnostics.rs` | `008ae3...` | legacy `AWF-EFX-*`, including generic upper-bound category |
| `crates/arcweft-lang-sema/src/diagnostics/effect_trace.rs` | `c462bc...` | trace currently projects text notes rather than typed source steps |
| `crates/arcweft-lang-sema/src/diagnostics/error.rs` | `b50539...` | shared `TypeCheckError::diagnostic()` boundary used by downstream consumers |
| `crates/arcweft-lang-sema/src/checker/module.rs` | `ce46da...` | trait catalog before effect registration; one collector finish point |
| `crates/arcweft-lang-sema/src/checker/expr/member.rs` | `4c1985...` | project method values explicitly rejected pending receiver-binding contract |
| `crates/arcweft-lang-sema/src/project_index.rs` | `ad1e4445d51975fc38c2bfa99beb88ac26d60ef8` | project callable records currently only Function/View and carry existing declaration ID |
| `crates/arcweft-lang-sema/src/checker/iterator.rs` | `d505b9ab0b3e43e5e54b809ac6c172c0f186664d` | current iteration evidence retains trait witness IDs, requiring compiler-side `"into_iter"`/`"next"` lookup |
| `crates/arcweft-compiler/src/trait_methods.rs` | `4fba2195f172f0766cd3da1cfa27e58596b27e71` | builds runtime trait-method identity from local impl/trait/witness indices, trait/self/method strings, and a witness monomorph label |
| `crates/arcweft-runtime-plan/src/trait_methods.rs` | `26a9b95bcf839ece8c2adbb79b9472de47f62eab` | inventory owns `BTreeMap<(usize, String), RuntimeTraitMethodId>` and populates it from witness plus method name |
| `crates/arcweft-core/src/plan.rs` | `8e3f62555ed88903950d1ed68871c0f64855b7eb` | serialized `RuntimeTraitMethodIdentity` carries local indices and multiple display strings |
| `crates/arcweft-core/src/entry/identity.rs` | `4a4c982978cb3079f984b5f7bc0ca05fcd407bef` | existing general `RuntimeCallableId` is the reusable opaque downstream runtime callable projection owner |
| `crates/arcweft-lsp/src/diagnostics.rs` | `227369...` | LSP projects shared diagnostics and validates source revision |

Ellipsized blob prefixes above identify connector-observed blobs whose complete
hash was not needed for a package integrity calculation; exact full hashes for
the load-bearing current owners are recorded in their rows. No claim is made
that a prefix is a full SHA.

## 4. Findings that drive the contract

1. Current method sema has static typed witnesses but no authoritative method
   effect row or source clause span.
2. Resolver method effects are currently manufactured as closed empty; this is
   not truthful for awaiting methods.
3. Existing `EffectRow` and substitutions are sufficient; a second row model is
   unnecessary.
4. Existing effect collection is already integrated into body checking, so a
   second inference pass would be architectural duplication.
5. Current method values are deliberately rejected and need the exact bound
   receiver/target contract supplied here.
6. Maintained language docs defer dynamic trait objects; no complete production
   owner exists, so E017 must be superseded rather than faked.
7. Shared diagnostic projection and revision-bound `SourceSpan` are sufficient
   for one CLI/LSP typed diagnostic path.
8. Current compiler/runtime trait-method identity is not suitable as a semantic
   join: it serializes local vector indices and display strings and maintains a
   `(usize, String)` witness-method lookup. Current iteration evidence retains
   only trait witness IDs, which forces later `"into_iter"`/`"next"` string
   selection. The existing general `RuntimeCallableId` can receive an opaque
   checked-ID projection, while conformance-keyed direct `RuntimeTraitMethodId`
   evidence removes runtime name resolution.
9. Detached HIR has exact module/source data but no project package identity;
   a source-revision-bound unified ordinal is truthful, whereas fabricating a
   package string would not be.

## 5. Validation performed for this output

Performed:

- complete request read;
- complete Rust skill read;
- parent ZIP structural test;
- parent internal manifest verification;
- complete parent member/test-matrix inspection;
- latest pushed-main query and repository file inspection through private
  GitHub connector;
- root `AGENTS.md` complete read;
- deterministic package member generation;
- member SHA-256/length manifest generation;
- ZIP structural test and post-ZIP member verification;
- external completed-ZIP SHA-256 generation.

Not performed, truthfully:

- no production repository clone was available in the artifact container;
- no Rust source, tests, manifests, fixtures, schemas, or stable docs were
  edited;
- no `cargo check`, Clippy, workspace, Tier 2, or product runtime test was run;
- no Jujutsu local metadata was available through the push connector.

Those are implementation validation gates, not design decisions. The exact
gates are specified in `IMPLEMENTATION_ORDER.md` and `TEST_MATRIX.md`.
