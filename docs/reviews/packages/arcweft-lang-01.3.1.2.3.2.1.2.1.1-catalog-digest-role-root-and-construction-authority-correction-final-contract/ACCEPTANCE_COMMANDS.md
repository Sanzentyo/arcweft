# Compile-clean acceptance evidence

Run from a clean checkout of `175a74da637ca5f455abdefda49c6b62897b00e2` after implementing the contract. Replace `<affected-package>` and exact test target names only with owners observed in the workspace; do not omit categories.

```bash
git status --short
git rev-parse HEAD
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Run focused suites for catalog canonicalization, role root, generation pair admission, nominal runtime value construction, CharacterDialogue, external producers, AWBC/VM/fiber, session, restore/replay/save, and hot swap. Run the repository-prescribed Tier 2/structural commands from the applicable `AGENTS.md` and CI configuration.

## Required compile-fail evidence

External test crates must fail to:

- construct `RuntimeCatalogDigestRoleRoot` or `AdmittedRuntimeGeneration` from fields/raw bytes;
- serialize/deserialize/default either admitted wrapper;
- construct/serialize/deserialize `RuntimeConstructionAuthority`;
- invoke nominal construction with raw nominal/layout/digest parameters;
- issue a producer capability without an admitted generation.

## Required deletion/static audit

Use `rg`/`git grep` with the exact old constructor/type names found at implementation time. Record every match and require zero unexplained production matches. At minimum audit patterns equivalent to:

```bash
rg -n 'new_unchecked|from_digest|from_root_bytes|try_from_accepted_layout|RuntimeNominalRecordValue' crates src tests
rg -n 'Deserialize.*Admitted|impl[[:space:]]+Default.*Admitted|From<\[u8;[[:space:]]*32\]>' crates src tests
rg -n 'RuntimePlan|AwbcProgram|catalog_digest|role_root|generation_identity' crates src tests
rg -n 'trait .*Catalog.*Role|static .*ROLE|match .*CatalogDigestRole' crates src tests
```

Matches in the original enum/nominal owner and explicit negative fixtures are expected only when their disposition is recorded. A helper/trait/side table that duplicates enum behavior is a failure even if tests pass.

## Final evidence

```bash
git diff --check
git status --short
```

The implementation evidence note must record command, exit status, toolchain, exact commit, and skipped tests with a reason. This design ZIP itself does not claim those production commands were run.
