# LANG-01 scaffold declaration removal

## Outcome

The author-facing `hook`, `memo fn`, generic `memo(...) { ... }`, and
top-level `parser` scaffolds are removed from syntax, AST, HIR, semantic
analysis, runtime-plan traversal, verifier traversal, LSP actions, and stable
language documentation. They had no complete executable contract and are not
preserved through aliases, dual readers, removed-syntax nodes, or dedicated
migration diagnostics.

Event handling now remains with its owning surface (View, source, line plan,
flow/action, or host observer). Caches remain owned by the subsystem that knows
their key, lifetime, invalidation, and failure policy. Decoders are ordinary
typed functions using `match`, cursor APIs, or codecs.

Line-plan `memo(.name, ...)` metadata is not the removed generic expression. It
remains a scoped line-plan directive. Internal deterministic dispatch tables
and subsystem caches likewise remain implementation substrate rather than
author-facing universal declarations.

## Proof trust consolidation

The separate `trusted axiom` item is removed. External evidence is represented
by a normal proof with typed metadata:

```arcw
#[verify.trusted(reason = "validated by signed build metadata")]
proof @proof.external_fact {
    check signed_external_fact()
}
```

The reason is required, must be a nonempty string literal, and is retained in
semantic and verifier reports. `use @proof.id` and
`assume ..., proof = @proof.id` form proof dependencies. Trust propagates
transitively through those dependencies and into every proof discharge that
uses them. `VerificationPolicy::allow_trusted_proofs` controls whether such
evidence is permitted; the CLI permits it in development/test modes and
forbids it in release mode.

## Acceptance evidence

- No removed syntax/CST/AST/HIR variants remain.
- Removed source forms do not reach typechecked HIR or executable lowering.
- No compatibility aliases, dual readers, or spelling-specific diagnostics
  were added.
- Stable documentation no longer presents the removed declarations as
  canonical syntax.
- Verifier reports enumerate direct trust reasons and transitive trusted proof
  dependencies.

## Verification

Validated after rebasing Jujutsu change `pvmznrry` onto main revision
`29dee80a8459`. All Cargo commands used `CARGO_INCREMENTAL=0`:

```text
cargo fmt --all
cargo test -p arcweft-lang-syntax --lib
  173 passed
cargo test -p arcweft-lang-sema --lib
  566 passed
cargo test -p arcweft-verify
  39 passed
cargo test -p arcweft-cli --test check verify_json_reports_unknown_proof_dependency
  1 passed; 468 filtered out
cargo check --workspace --all-targets --all-features
  passed
cargo clippy --workspace --all-targets --all-features -- -D warnings
  passed
cargo +nightly -Zscript tools/structure-audit.rs --root .
  0 errors; 128 warnings
just test-workspace
  blocked by one unchanged arcweft-bundle baseline assertion
```

`just test-workspace` reached `arcweft-bundle` and failed only
`view_resource_codecs::emit_text_requires_a_one_to_one_owned_text_block_graph`:
the decoder returned `ViewExport(DuplicateStaticTarget)` while the existing
test expects `NonCanonicalTable("view_emit_text_block_duplicate_refs")`. The
focused test reproduces the same mismatch. This change has no
`crates/arcweft-bundle` diff, so that independent baseline failure is not folded
into the language-declaration removal.

The machine-readable structural audit is in
`docs/implementation/structure-audits/lang-01-scaffold-declaration-removal-2026-07-16/`.

## Structural review

No Cargo dependency or feature boundary changed. The audit records these
current production hotspots most directly involved in the removal:

| File | Bytes | Physical LOC | Main responsibility |
| --- | ---: | ---: | --- |
| `arcweft-lang-syntax/src/ast/items.rs` | 39,127 | 1,650 | remaining top-level typed AST items |
| `arcweft-lang-syntax/src/parser/items.rs` | 49,830 | 1,424 | remaining top-level item parsers |
| `arcweft-lang-syntax/src/parser/proof.rs` | 9,638 | 285 | proof/test/bench syntax and proof trust metadata |
| `arcweft-lang-sema/src/semantic.rs` | 79,077 | 2,123 | semantic obligations, proof inventory, and trust propagation |
| `arcweft-verify/src/lib.rs` | 67,273 | 1,959 | public verifier report/policy facade and obligation collection |

The measured dependency fan-in/fan-out is syntax 14/7, HIR 11/5, sema
11/12, and verifier 5/8. AST and parser item files became smaller by deleting
the provisional families. Proof trust remains in semantic/verifier ownership;
it does not introduce a reverse dependency or duplicate the trust model in a
runtime crate. The warning-level large-file findings remain recorded for a
future responsibility decomposition; there are no error-level structural
exceptions in this cut.

## Deliberate non-goals

- This cut does not remove internal runtime dispatch or cache machinery.
- This cut does not remove line-plan memo directives.
- This cut does not redesign ordinary `fn`, owner-local handlers, codecs, or
  cursor APIs.
- Other top-level reductions (`reducer`, `state`, resource unification, and
  callable-kind consolidation) are independent changes.
