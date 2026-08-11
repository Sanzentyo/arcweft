# Validation plan

## 1. Validation policy

Validation runs only after the full direct replacement is implemented. Partial stages are not accepted as green because they can temporarily omit producers or retain dual APIs.

For every command, record:

- exact Git commit/change ID;
- command line;
- working directory;
- exit status;
- test counts where reported;
- any failure classified as introduced, pre-existing, environmental, or command/filter mismatch.

A command not executed is not reported as passed. A filtered test command that matches zero tests must be rerun with the owning package's complete tests.

## 2. Formatting gate

```bash
cargo fmt --all -- --check
```

Required result: exit 0 with no diff.

## 3. Focused owner and seam tests

Run in this order so failures localize to the owning layer:

```bash
cargo test -p arcweft-lang-syntax symbol_path
cargo test -p arcweft-lang-hir symbol
cargo test -p arcweft-character id
cargo test -p arcweft-adapter-context
cargo test -p arcweft-adapter-context --features sema
cargo test -p arcweft-project-loader environment
cargo test -p arcweft-lang-sema callable
cargo test -p arcweft-lang-sema registration
cargo test -p arcweft-lang-sema --test character_manifest_types
cargo test -p arcweft-compiler registration
cargo test -p arcweft-lsp profiles
```

Required coverage is the complete `TEST_MATRIX.md`, even when local test filters use different names.

Focused acceptance facts:

1. direct/adapter path constructors reject malformed values;
2. character qualified/compact/alias paths remain exact and same-target;
3. imports/globs/re-exports/aliases retain path segments;
4. typed iterator order is insertion-independent;
5. catalog includes qualified character/adapter non-callables;
6. environment fallback is blocked by qualified and alias project bindings;
7. collision and path/work-limit errors remain typed;
8. accepted pointer/generation survive malformed/colliding candidates.

## 4. Cargo metadata dependency evidence

Run:

```bash
cargo metadata --format-version 1 --no-deps > target/aw-ah-009.3.3.2-metadata.json
```

Consume the JSON package/dependency graph, not repository source text. Record these assertions:

- `arcweft-lang-hir` does not depend on `arcweft-lang-sema`;
- `arcweft-lang-hir` does not depend on `arcweft-adapter-context`;
- `arcweft-lang-sema` does not depend on `arcweft-adapter-context`;
- `arcweft-adapter-context` default features do not activate syntax/HIR/sema dependencies;
- `arcweft-adapter-context` feature `sema` activates the existing optional syntax/HIR/sema/source dependencies;
- no dependency cycle is introduced;
- public typed APIs compile from their documented owner crates.

A one-shot CI/test utility may parse Cargo metadata, but it must not inspect Rust source text and must not become a source gate.

## 5. Public API compile evidence

Compile representative external-use tests or doctests that:

1. build `ProjectSymbolSegment`/`ProjectSymbolPath`;
2. construct `ProjectDirectBinding` and read `path()`;
3. consume typed `ProjectSymbolTable::scope_bindings()` rows;
4. build `AdapterSymbolPath`/`AdapterSymbol` under adapter-context default features;
5. publish adapter facts under the `sema` feature.

Do not add negative compile fixtures solely to grep for old method names. The direct deletion plus all-target compilation is the old-API absence proof.

## 6. Workspace compile gate

```bash
cargo check --workspace --all-targets
```

Required result: exit 0.

This gate must enumerate and force migration of every old constructor/accessor consumer, including fixtures not found by the initial code search.

## 7. Clippy gate

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Required result: exit 0.

Review specifically for:

- needless allocations/render/reparse cycles;
- manual string path manipulation;
- redundant closures/helpers where inherent owner APIs exist;
- visibility broader than the contract;
- result handling that could silently omit a binding;
- unstable ordering through hash-map iteration.

Do not suppress a new warning unless the lint is genuinely inapplicable and the reason is local, specific, and documented.

## 8. Workspace tests

```bash
cargo test --workspace --all-targets --all-features
```

Required result: exit 0.

This is required after focused tests because adapter standard manifests, LSP/verifier fixtures, tooling tests, compiler fixtures, and unrelated import/callable regressions may exercise the directly replaced APIs.

## 9. Canonical structural audit

Run the root-instruction command exactly:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Required result: no structural errors. Warnings must be recorded and classified; a new mixed-responsibility or dependency-direction warning introduced by the correction must be fixed before acceptance.

The new adapter symbol model should live in its own cohesive `symbol.rs`, avoiding unnecessary growth of `manifest.rs`. HIR behavior remains in existing symbol identity/table owners.

## 10. Determinism validation

In addition to unit equality assertions:

1. run reversed producer/fact insertion variants in the same test process;
2. compare typed HIR iterator sequences;
3. compare complete `ProjectCallableCatalog` values, not `HashMap` iteration order;
4. compare collision path/first/second evidence;
5. compare accepted-world pointer/generation after rejection;
6. run relevant deterministic tests more than once when diagnosing nondeterminism.

No golden generated spelling is a substitute for typed path equality.

## 11. Atomicity validation

The state-owner test must capture before the rejected update:

- accepted `Arc<RegisteredSemanticWorld>`;
- accepted generation/revision;
- symbols `Arc`;
- registered environment `Arc`;
- callable catalog `Arc`;
- character definition index `Arc` where exposed.

After malformed/colliding input, assert pointer equality or exact unchanged identity for every exposed accepted object. Then submit a valid update and assert normal publication.

## 12. Manual code-review gates

Review the final diff against `DELETION_CHECKLIST.md` and answer all as true:

- only one direct-binding constructor exists;
- only one scope-binding iterator exists;
- adapter typed manifest stores no symbol string;
- codec has one source-field decoder and no alternate schema shape;
- no sema/catalog path split or invalid-name skip exists;
- no character `as_str` split/strip is used for binding identity;
- no adapter types enter sema;
- no HIR dependency on sema/adapter-context exists;
- no resolver/catalog/transaction redesign appears;
- no compatibility, dual reader, source gate, CSS, or Takumi route appears.

This review reads the actual diff; it is not implemented as a persistent source-text gate.

## 13. Final acceptance record

The implementation handoff must include a concise validation record with:

```text
IMPLEMENTED_COMMIT=<sha>
FORMAT=PASS
FOCUSED_TESTS=PASS
CARGO_METADATA_DEPENDENCIES=PASS
WORKSPACE_CHECK=PASS
CLIPPY_D_WARNINGS=PASS
WORKSPACE_TESTS=PASS
STRUCTURE_AUDIT=PASS
OPEN_QUESTIONS=0
```

Any non-pass value blocks completion. Environmental inability must be reported honestly and resolved before marking the implementation complete; it is not converted into a pass.

## 14. Current artifact validation versus future implementation validation

This final-contract archive has passed only packaging/integrity validation. It intentionally contains no Rust patch. Therefore all Rust/Cargo entries above are `NOT_RUN_FOR_FUTURE_IMPLEMENTATION` in this artifact and are not represented as current failures or passes.
