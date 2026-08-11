# Applied Rust-skill and repository rules

The supplied Rust Skill was read in full before authoring the schemas.

Applied rules:

1. The package is design-only and contains no Rust patch.
2. Every new public boundary is owned by the responsible crate:
   attached syntax in `arcweft-lang-syntax`, HIR/source identity in
   `arcweft-lang-hir`, semantic effect/callable identities in
   `arcweft-lang-sema`.
3. Required behavior is added to the original enum or owner. The design extends
   `SyntaxKind`, `SyntaxRole`, `HirSourceQuery`, and the original source-role
   enums in place. It introduces no local extension trait or ad hoc matching
   helper in a consumer.
4. Fields are private. Construction is crate-owned and checked. Consumers use
   read-only accessors.
5. Structural enums are exhaustive. No wildcard arm, string tag, sentinel ID,
   or untyped map is part of the final model.
6. Fallible construction has structured `thiserror`-style error payloads and a
   deterministic validation order.
7. Crate direction remains
   `syntax -> hir -> sema -> runtime-plan/verify -> tooling`.
8. No `unsafe`, serialization, raw numeric-ID conversion, or public source-less
   constructor is authorized.
9. Migration removes old variants and fixes call sites; it does not preserve
   compatibility.
10. Tests are behavioral, typed, compile-fail, codec/transcript, or structured
    dependency evidence. No source gate is authorized.

11. The latest root, `crates/`, `docs/`, `docs/reviews/`, and
    `docs/implementation/` scoped instructions were read. Evidence uses full
    Git SHAs only; no Jujutsu identity is recorded.
12. Planned implementation validation follows the current test-execution and
    structural-audit policies. This design-only archive does not report planned
    Rust commands as completed evidence.
