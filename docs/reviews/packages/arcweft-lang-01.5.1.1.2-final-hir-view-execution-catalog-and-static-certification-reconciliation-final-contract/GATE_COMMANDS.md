# Required implementation verification commands

All commands run from the repository root against the final implementation commit.
Capture exact stdout, stderr, exit status, Git commit, toolchain, target, features,
and environment. Resolve integration-test target names from current Cargo metadata;
do not invent a test target or replace a missing target with a source grep.

## Focused owner tests

```bash
CARGO_INCREMENTAL=0 cargo test -p arcweft-lang-sema final_analysis::view --all-features --jobs 4
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --test view_product --all-features --jobs 4
CARGO_INCREMENTAL=0 cargo test -p arcweft-compiler --test dialogue_profile_admission --all-features --jobs 4
CARGO_INCREMENTAL=0 cargo test -p arcweft-view --all-targets --all-features --jobs 4
CARGO_INCREMENTAL=0 cargo test -p arcweft-bundle view --all-features --jobs 4
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-driver view --all-features --jobs 4
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-driver session_save --all-features --jobs 4
CARGO_INCREMENTAL=0 cargo test -p arcweft-runtime-driver swap --all-features --jobs 4
```

The implementation must also run the owning UI/compile-fail targets for
`arcweft-lang-sema`, `arcweft-compiler`, `arcweft-view`, `arcweft-bundle`, and
`arcweft-runtime-driver` as discovered by current metadata/test policy. API tests
must exercise absent/private/typed APIs; they do not grep source text.

## Workspace gates

```bash
cargo fmt --all -- --check
CARGO_INCREMENTAL=0 cargo check --workspace --all-targets --all-features
CARGO_INCREMENTAL=0 cargo clippy --workspace --all-targets --all-features -- -D warnings
CARGO_INCREMENTAL=0 cargo test --workspace --all-targets --all-features --jobs 4
```

## Metadata and dependency evidence

```bash
mkdir -p target/lang-01-5-1-1-2-evidence
cargo metadata --format-version 1 --all-features   > target/lang-01-5-1-1-2-evidence/cargo-metadata.json
cargo tree -p arcweft-lang-sema -e normal   > target/lang-01-5-1-1-2-evidence/sema-tree.txt
cargo tree -p arcweft-compiler -e normal   > target/lang-01-5-1-1-2-evidence/compiler-tree.txt
cargo tree -p arcweft-view -e normal   > target/lang-01-5-1-1-2-evidence/view-tree.txt
cargo tree -p arcweft-bundle -e normal   > target/lang-01-5-1-1-2-evidence/bundle-tree.txt
cargo tree -p arcweft-runtime-driver -e normal   > target/lang-01-5-1-1-2-evidence/runtime-driver-tree.txt
```

The checked-in typed metadata validator for this cut must prove:

- syntax does not depend on HIR/sema/compiler/runtime/bundle;
- HIR does not depend on compiler/runtime/bundle;
- sema does not depend on compiler/bundle/runtime-driver;
- `arcweft-core` does not depend on syntax/HIR/sema/compiler/bundle;
- `arcweft-resource-model` uses core runtime value types without a reverse core
  dependency;
- native/Web/headless/Agent/MCP have no authoring/checked-catalog authority; and
- no second View value VM, endpoint catalog, CSS, or Takumi dependency is present.

## Tier-2 rows

Run every repository-maintained Tier-2 target corresponding to `T2-*` in
`TEST_MATRIX.csv`: compiler, bundle codec, runtime driver, save/replay, hot swap,
headless, native, Web/browser, Agent, MCP, image/animation, resources, and generated
artifact binding. Record the exact resolved commands from current test policy in
the implementation note. No affected row may be marked skipped solely because the
fast workspace suite passed.

## Diff and repository state

```bash
git diff --check -- .
git diff --cached --check -- .
test -z "$(git diff --name-only --diff-filter=U)"
git status --short
```

## Canonical structural audit

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Check in the cut-specific report under `docs/implementation/structure-audits/`.
It must show zero structural errors and exact bytes/physical LOC/code LOC/test LOC,
generated status, responsibility, fan-in, and fan-out for changed/largest files.
After checking it in, rerun format, diff check, strict Clippy, the complete workspace
test, and all affected Tier-2 rows.
