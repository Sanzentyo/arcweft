# Implementation order and commit discipline

1. Freeze baseline and read root/scoped `AGENTS.md`; record exact commit and clean status.
2. Add/fix role behavior in the original enum inherent `impl`, plus golden role-domain/ordinal tests.
3. Add bounded canonical typed catalog encoders and per-role digest derivation.
4. Add complete/unique role-root derivation and golden vectors.
5. Refactor plan/AWBC admission to build one private candidate aggregate and generation.
6. Add opaque non-Serde admitted root/generation handles and scoped construction authority.
7. Change the original nominal runtime value constructor to require authority + admitted layout.
8. Add external producer and CharacterDialogue typed façades.
9. Migrate all runtime/session/save/replay/hot-swap consumers.
10. Add compile-fail, unit, property, integration, codec, diagnostic, and failure-atomicity matrix coverage.
11. Delete old constructors, Serde impls, direct raw execution, side tables/traits/helpers, shims, and dual readers.
12. Run format, compile, Clippy, targeted suites, workspace suites, Tier 2, structural grep, and clean-tree evidence.

Do not leave the tree in a dual-authority intermediate state for final review. Intermediate implementation commits may be mechanically staged, but the submitted implementation must have all deletions and exact callers closed.
