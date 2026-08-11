# Implementation handoff

## 1. Required implementation sequence

Proof 01.1.1 is **not** a prerequisite. The selected range/identity branch must
be implemented in these compiling cuts.

### Cut 1 — exact syntax/HIR range substrate and frozen API types

1. Add `arcweft-lang-syntax/src/call.rs` with
   `CallExpressionSyntax`, `ArgumentListSyntax`, `ArgumentSyntax`, forms,
   recovery, validation, and public accessors.
2. Replace the current `Expr::Call` shape directly and update every exhaustive
   match. Do not retain a legacy variant or compatibility constructor.
3. Make the expression parser retain exact callee, delimiter, argument, name,
   value, separator, and recovery ranges while tokens are already available.
4. Extend `SpeakerLineSurface` and `ContentCall` directly with the optional
   typed argument list.
5. Preserve these owned values through HIR lowering.
6. Add `HirModule::source_identity`, `HirModule::module_path`, and
   `ProjectSymbolTable::source_identity` as inherent methods on the owning
   types.
7. Add all public sema signature/result/error/limit types with private fields
   and documented invariants.

Compiling gate:

```bash
cargo test -p arcweft-lang-syntax call
cargo test -p arcweft-lang-hir call_range
cargo check -p arcweft-lang-sema
```

### Cut 2 — one shared semantic resolver and position query

1. Add `arcweft-lang-sema/src/call_resolution.rs` as a crate-private
   responsibility module.
2. Move the current call dispatch order from checker-local branches into
   `ResolvedCallTarget` production. Existing checker-specific validation remains
   in checker methods, but target identity and candidate selection occur once.
3. Add typed `PresentationCallableId` and `DialogueCallableId` with inherent
   `resolve` and `signature_schema` behavior. Remove duplicated presentation
   and dialogue parameter-shape matches from checker code.
4. Make presentation `show.look` and dialogue `look` call
   `check_expr_with_expected` with the exact structural `TypeKind` obtained from
   the schema.
5. Extend `RegisteredTypeCheckEnv` with typed callable candidate records and
   read-only project/environment/method lookup methods. Keep its base
   environment private.
6. Collect project source function and extern-capability signatures,
   documentation, declaration IDs, and source spans during the same registered
   world build.
7. Add `arcweft-lang-sema/src/signature.rs` and implement
   `query_signature`. It locates the exact argument list, invokes target-fact
   checker mode, binds arguments, ranks overloads, and emits structured results.
8. Add cancellation/deadline checks and all production work counters before
   each bounded operation.

Compiling gate:

```bash
cargo test -p arcweft-lang-sema signature
cargo test -p arcweft-lang-sema presentation
cargo test -p arcweft-lang-sema dialogue
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
```

### Cut 3 — normalize adapter metadata into the same result model

1. In `arcweft-adapter-context`, construct validated `AdapterPackageId` and
   `EnvironmentCallableId` values while applying accepted metadata.
2. Store typed parameters, return type, callable/parameter documentation,
   package provenance, and origin in `TypeCheckEnv` records.
3. Change environment maps to ordered candidate sets and reject duplicate IDs
   or same-rank authorities. Do not keep first/last map replacement behavior.
4. Ensure `CharacterRegistrar` publishes these base records and project records
   in one `RegisteredSemanticWorld` without exposing or rebuilding a parallel
   `TypeCheckEnv`.
5. Add adapter-only and same-name project/adapter sema tests before LSP work.

No new external dependency or Cargo-manifest change is required: current crate
manifests already provide adapter-context-to-sema and LSP-to-syntax/HIR/sema
edges.

Compiling gate:

```bash
cargo test -p arcweft-adapter-context --features sema
cargo test -p arcweft-lang-sema registered_callable
```

### Cut 4 — checked LSP mapping, deterministic presentation, and cache

1. Add `LineIndex::try_byte_offset_from_position` and direct boundary tests for
   negotiated UTF-8 and UTF-16 positions.
2. Replace `arcweft-lsp/src/features/signature.rs` with a handler that:
   - clones one `DocumentSnapshot`;
   - obtains one current `AcceptedProfileEnvironment`;
   - forms `SignatureRequestStamp`;
   - checks the typed cache;
   - invokes the sema query with `accepted.world()`;
   - performs the final stamp check;
   - builds deterministic LSP labels and UTF-16 parameter-label ranges;
   - inserts only cacheable outcomes.
3. Replace `ProfileSemanticCaches` placeholder entries with the typed bounded
   signature cache. Do not add a second field retaining the old string cache.
4. Change session signature handling from an `Option`-only path to typed request
   error mapping. Preserve LSP `null` for semantic non-applicability.
5. Add explicit document-close, workspace-removal, profile-replacement, and
   shutdown cache cleanup.
6. Keep client cancellation visible to the running query through the request
   control token; map timeout/resource failure to `ServerCancelled`.

Compiling gate:

```bash
cargo test -p arcweft-lsp signature
cargo test -p arcweft-lsp positions
cargo test -p arcweft-lsp profile_cache
cargo clippy -p arcweft-lsp --all-targets --all-features -- -D warnings
```

### Cut 5 — delete the competing resolver

After every caller uses the sema query:

1. delete `arcweft_verify_lsp::rust_adapter_signature_help`;
2. delete signature-help-only word extraction from
   `arcweft-lsp::features::signature`;
3. delete metadata result construction that selects a Rust function by name;
4. remove tests that assert word-based or first-match behavior;
5. retain only unrelated verifier/LSP adapters in `arcweft-verify-lsp`;
6. remove any now-unused dependency import, but do not add a compatibility
   wrapper or deprecated re-export.

Compiling gate:

```bash
cargo test -p arcweft-verify-lsp
cargo test -p arcweft-lsp signature
cargo check --workspace --all-targets --all-features
```

### Cut 6 — completion validation

Implement every direct test in `TEST_MATRIX.md`, then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-workspace
cargo test --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

If repository test-execution policy selects a narrower checked-in command for
routine validation, run that command during development, but the final
implementation cut must still record the focused signature suites, workspace
check/clippy, the normal workspace validation entry point, and the canonical
structural audit. Do not claim an unrun command.

## 2. File/module ownership cuts

| Crate | Required owner edits |
| --- | --- |
| `arcweft-lang-syntax` | new `call.rs`; direct `Expr::Call` shape; Pratt/call parser; dialogue parser and owned surfaces; recovery tests |
| `arcweft-lang-hir` | preserve owned call ranges; expose source/module identity on owning `HirModule`; populate per-module source identity in `ProjectSymbolTable` construction |
| `arcweft-lang-sema` | new `signature.rs`; new crate-private `call_resolution.rs`; typed callable candidate records; presentation/dialogue inherent schemas; target-fact checker mode; structured errors/limits |
| `arcweft-adapter-context` | build typed adapter IDs, typed signature records, documentation/provenance, and collision errors through existing sema feature |
| `arcweft-lsp` | checked position method; accepted stamp; typed cache; result formatter; session error mapping and invalidation |
| `arcweft-verify-lsp` | delete Rust signature resolver and its tests; no replacement resolver here |

No stable design chapter, schema, fixture, Cargo manifest, completion, hover, or
rename owner is part of this implementation.

## 3. Resolver migration rule

The checker migration must remain compiling after each family:

1. make the shared resolver return the same special-form category currently
   selected;
2. switch that checker branch to consume the category;
3. add its signature schema and direct tests;
4. only then delete the old branch-local selection code.

At no point may checker and signature query both successfully resolve the same
call through independent logic. A temporary compiling cut may keep an old
checker branch only when the shared resolver's result is asserted and the old
branch cannot be reached for that family; it must be deleted within the same
reviewable cut.

## 4. Character registration integration

`CharacterNominalType` remains unchanged as identity. Add no alias registry to
signature code. Dynamic `look` construction occurs only after typed owner
resolution through the accepted symbols/environment. The schema calls existing
`CharacterNominalType::look` and, for accepted part/variant types, returns the
already-stored nominal unchanged.

When behavior is missing from an Arcweft-owned enum or boundary type, add an
inherent method to that owner. In particular:

- type label behavior belongs on `TypeKind`/`CharacterNominalType`;
- presentation schema behavior belongs on `PresentationCallableId`;
- dialogue schema behavior belongs on `DialogueCallableId`;
- source identity lookup belongs on `ProjectSymbolTable`;
- checked position conversion belongs on `LineIndex`.

Do not add local extension traits or repetitive feature-level matches.

## 5. Failed rebuild sequence

Profile rebuild remains transactional:

```text
build candidate world completely
  -> validate symbols/environment/character inventory/callable collisions
  -> on success: replace_accepted and allocate fresh cache
  -> on failure: record diagnostics only; retain prior accepted Arc and cache
```

The signature handler reads only `state.current()`. A changed open document that
is not represented by the retained project-table module identity returns stale.
No attempted generation or digest is ever formed.

## 6. Structural audit expectations

The final implementation should leave:

- one public sema signature query;
- one internal call resolver;
- one typed accepted cache;
- zero word-only signature resolvers;
- zero signature-specific node IDs or syntax databases;
- zero label parsers or global source searches;
- no dependency reversal from syntax/HIR/sema to LSP or adapter metadata.

The structural audit must be used for crate ownership, dependency direction,
file size, and typed boundaries only. It must not scan implementation source for
symbol names or snippets as pass/fail evidence.

## 7. Completion record required from production implementation

The later production handoff must report:

- exact changed files by crate;
- focused tests and their results;
- workspace check/clippy/test and structural-audit results actually run;
- whether no new dependencies/manifests were needed;
- fallback deletion confirmation based on compiled callers and direct behavior,
  not a source grep gate;
- remaining work, which must be empty for this contract's acceptance matrix.
