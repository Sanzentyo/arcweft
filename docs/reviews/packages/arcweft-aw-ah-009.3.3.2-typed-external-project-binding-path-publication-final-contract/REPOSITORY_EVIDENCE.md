# Repository evidence

## 1. Inspection identity

- Repository: `Sanzentyo/arcweft`
- Ref requested: latest `main`
- Exact inspected commit: `9a63ac5512cd75947ba70195681e43ab968f9f12`
- Latest commit subject: `Implement native physical box geometry reconciliation`
- Latest-main check immediately before archive construction: same commit
- Relevant predecessor commits observed:
  - `77fdced6800be0baa0285b2edfbfdf22e983429a` — dotted free callable resolution;
  - `e820c91ed966239ab7486cb4f8885540cbc2abb5` — shared registered free callable catalog.

The repository was inspected through the configured GitHub connector at immutable blobs/commit. No production checkout or repository mutation was performed.

## 2. Governing inputs

| Input | Read extent | Digest/evidence |
|---|---|---|
| attached AW-AH-009.3.3.2 request | complete 134 lines; sole request specification | SHA-256 `598aa6d354214d4ea486b52aa2ecaf1e31d016f6fbd53668d7ea8ec19bb7a1bb` |
| supplied Rust skill | complete 56 lines | SHA-256 `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` |
| supplied Arcweft premise | complete file | SHA-256 `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` |
| root `AGENTS.md` at inspected commit | complete file, all sections | blob `c41ff4d2b3baadda3e9f975c7de3e5a6678f8758` |
| AW-AH-009.3.3 final-contract material | relevant final contract, reconciliation, inventory, traceability, and evidence members | supplied attachment index/extracted text |

The repository contains a copy of the correction request, but the attached request digest above is the governing specification used by this package.

## 3. Inspected production files

| File | Blob SHA | Current fact established |
|---|---|---|
| `AGENTS.md` | `c41ff4d2b3baadda3e9f975c7de3e5a6678f8758` | typed APIs, direct unpublished replacement, no compatibility/source gates, canonical validation commands |
| `crates/arcweft-lang-syntax/src/ast/symbol_path.rs` | `e2bb44a4d7bce36df676fc0baed2b708f6c49926` | existing `ProjectSymbolPath`/segment model, `-` support, typed roots/segments, opaque `SymbolPath` conversion |
| `crates/arcweft-lang-syntax/src/ast/module_path.rs` | `bd0d3fe0619278523d5ccea15a840d98a269056b` | module segments reject `-`; project segments are intentionally broader |
| `crates/arcweft-lang-syntax/src/ast/common.rs` | `2cc29c3a15395fc5b6b981f16173c2a2e032a7d8` | typed import paths/aliases and ordered `Visibility` |
| `crates/arcweft-lang-hir/src/symbol/identity.rs` | `bdaa65f0ff98312719babf1ab2b37adc60a55bda` | current `ProjectDirectBinding { name: String }`; canonical seed and bindings are already separate fields |
| `crates/arcweft-lang-hir/src/symbol/table.rs` | `532840b442718f3dd9c452c9b1db4551577de93f` | current string-keyed scopes, string iterator, import/re-export/fixed-point/coalescing behavior |
| `crates/arcweft-lang-hir/src/symbol/tests.rs` | inspected at commit | current linker fixtures and direct-binding caller |
| `crates/arcweft-character/src/id.rs` | `fefbd574e637a631ea0785e14649596e106a516c` | `CharacterId::compact_segments()` already supplies validated component evidence |
| `crates/arcweft-project-loader/src/environment.rs` | `0f030a9ef123c1828a093a124eb21c06c57582ce` | character facts currently construct direct bindings from `as_str`/`compact_str` strings |
| `crates/arcweft-adapter-context/src/callable.rs` | `2e1d42798c2659398225ef5524b30254244db108` | existing adapter callable path is typed and must not be repurposed/redesigned |
| `crates/arcweft-adapter-context/src/manifest.rs` | `888adbfead7c15963b9f9c6d8ae1287b5492a2e2` | `AdapterSymbol { name: String }`, string `with_symbol`, and string direct fact publication |
| `crates/arcweft-adapter-context/src/codec.rs` | `aab255f4f8dd9d6be0db9de75e2084f072a0c052` | schema-v1 `symbols[].name` and current typed callable parser boundary |
| `crates/arcweft-adapter-context/Cargo.toml` | `ec752ae3728838c33c622d6973fb534b0c8e6df4` | syntax/HIR/sema dependencies are optional under `sema`; base manifest is language-free |
| `crates/arcweft-lang-sema/src/callable/identity.rs` | `301907413c3e82cc13c7b44abb0ea6be5a8bf59a` | implemented `CallableName`, segmented `CallablePath`, `ProjectCallablePath`, `ProjectNameBinding` |
| `crates/arcweft-lang-sema/src/callable/error.rs` | `9f76c09fd4cd05e8ab96512e435930ac2d184ca1` | existing path-limit and catalog error vocabulary is sufficient |
| `crates/arcweft-lang-sema/src/callable/builder.rs` | `c012642fc67b9afa637c55761d3b9de45091c1c6` | concrete invalid-name skip; existing deterministic catalog finish/collision behavior |
| `crates/arcweft-lang-sema/src/callable/catalog.rs` | inspected at commit | immutable project binding map and read-only catalog access are implemented |
| `crates/arcweft-lang-sema/src/callable/resolver.rs` | inspected at commit | project non-callable bindings terminate environment fallback; no resolver redesign needed |
| `crates/arcweft-lang-sema/src/registration/registrar.rs` | `e8f2fe5b3ad7bad0ba274b6071b0d7d6a1a935e7` | catalog built before candidate world; target-to-`TypeKind` mapping; string-based character collision audit remains |
| `crates/arcweft-lang-sema/src/registration/model.rs` | inspected at commit | accepted-world/environment ownership and external owner facts |
| `crates/arcweft-compiler/src/project.rs` | `a329ff8552014edb314a351d187ba6a576bff5fa` | compiler rollback fixture is a current direct-binding caller |
| `crates/arcweft-lsp/src/profiles/accepted_project.rs` and profile state | inspected at commit | existing accepted project holder is the correct pointer-atomicity test owner |

## 4. Concrete defect evidence

Current `RegisteredCallableCatalogBuilder::add_project_bindings` does this for each scope row:

```rust
let Ok(name) = CallableName::try_new(spelling) else {
    continue;
};
```

The adjacent production comment explicitly states that qualified external leaves are omitted until a producer owns segmented evidence. Therefore:

- `akane` is indexable as one segment;
- `character.akane` fails `CallableName` and is skipped;
- the project symbol table still accepts/resolves the qualified external binding;
- the project callable catalog is incomplete;
- a same-spelled environment callable can bypass a project non-callable shadow.

This is a concrete flaw in the string-only publication seam. No concrete flaw was found in `ProjectNameBinding`, catalog record structures, the resolver's project-first rule, or the candidate-world construction ordering.

## 5. Existing substrate that supports the selected correction

### 5.1 Syntax path evidence

`ProjectSymbolPath` already stores root plus ordered `ProjectSymbolSegment` values. The segment grammar permits `-`, while module segments do not. Its source parser is already the correct source-text boundary. Its conversion to `SymbolPath` already preserves current external-root behavior.

### 5.2 Character evidence

`CharacterId::compact_segments()` is an inherent typed-owner API derived under `CharacterId::try_new` validation. The loader and registrar can use it without splitting `as_str()`.

### 5.3 Callable evidence

`CallableName` rejects `.`, `:`, `/`, `\\`, controls, and grouping delimiters. `CallablePath` is segmented and limit-checked. `ProjectCallablePath` already combines package, module, and `CallablePath`. `ProjectNameBinding::NonCallable` already stores the exact project path and `TypeKind`.

### 5.4 Transaction evidence

The registrar links/validates facts, builds all project/environment catalog content, calls `finish`, and only then constructs the candidate registered environment/world. A catalog error returns before the candidate world is returned. Pointer preservation belongs to the existing accepted-world owner and requires direct regression tests, not transaction redesign.

## 6. Current producer inventory evidence

Exact code search for `ProjectDirectBinding::try_new` returned six files:

1. `crates/arcweft-compiler/src/project.rs`;
2. `crates/arcweft-adapter-context/src/manifest.rs`;
3. `crates/arcweft-project-loader/src/environment.rs`;
4. `crates/arcweft-lang-hir/src/symbol/tests.rs`;
5. `crates/arcweft-lang-sema/src/test_support/character_project.rs`;
6. `crates/arcweft-lang-sema/tests/character_manifest_types.rs`.

The adapter `with_symbol` surface also has standard-manifest, codec, LSP/verifier, sema, tooling, and compiler fixture consumers; deleting the old signature makes the Rust compiler enumerate every exact caller during the coherent cut.

## 7. Evidence-based design conclusions

1. Reusing `ProjectSymbolPath` is smaller and more faithful than creating a HIR duplicate.
2. Storing it directly in `ProjectDirectBinding` removes the lossy boundary at its owner.
3. Adding it to `ScopeBinding` preserves evidence through current linker behavior without replacing the resolver.
4. A typed iterator is sufficient for complete catalog publication.
5. Character producers already have typed component evidence.
6. Adapter producers need one new language-free typed symbol path before sema; using the existing callable path would conflate domains.
7. Existing catalog and resolver types become correct once given complete segmented input.
8. Existing accepted-world ordering is already fail-closed and should only receive direct atomicity tests.

## 8. Verification honesty

Completed in this artifact runtime:

- complete governing request, Rust skill, Arcweft premise, and root `AGENTS.md` read;
- latest-main pointer checked and relevant production/test files inspected through GitHub at an immutable commit;
- predecessor contract material inspected for already implemented callable/catalog/transaction decisions;
- exact producer code search performed;
- final contract/test/order/deletion package constructed;
- archive integrity and deterministic packaging verified.

Not performed, because no production implementation was requested or produced:

- no Rust source edit;
- no local repository clone/build;
- no `cargo fmt`, focused Rust tests, workspace check, clippy, workspace tests, metadata graph assertion against a patched tree, or structural audit;
- no claim that future implementation commands have passed.

Those commands and acceptance criteria are fixed in `VALIDATION_PLAN.md` for the implementation assignee.
