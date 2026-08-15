# Validation commands and execution status

## Actually performed for this design return

- GitHub `main` history inspection: passed; first row is `35d42efdd89fef8fde73f62be2a3e38fd5e81e52`.
- Commit page and commit-pinned raw source inspection: passed.
- exact request byte count/hash: passed (`13997` bytes,
  `1b54121c38f7f957f9c168a02d25fef26ba21e7f50da9fc89e4b390ac9281c65`).
- previous INVALID byte count/hash: passed (`6064` bytes,
  `e17112bc1e6a6ce5611e1131448a8cec4efb647cfdabacfc042232d48dc15dc9`).
- Rust Skill and all applicable captured `AGENTS.md` files: read completely.
- package member/path/case/symlink preflight: performed by the package verifier.
- internal manifest hash verification: performed by the package verifier.
- no production patch/source extension check: performed by the package verifier.

## Attempted but unavailable

Local `git ls-remote` / `git fetch` transport could not resolve `github.com` in
the execution container. The baseline was therefore obtained through GitHub's
current `commits/main` page and commit-pinned raw HTTP evidence. This package
does not claim a local checkout or clean-tree result.

## Not run for this design-only return

No Cargo, Clippy, Rust tests, `just` test tier, VM/JIT/AOT execution, or
structure-audit command was run because no repository checkout or production
patch is included. These are executable implementation gates, not claimed
results.

## Required implementation commands (all package names exist at current main)

```text
cargo fmt --all -- --check
cargo check -p arcweft-core
cargo test -p arcweft-core
cargo check -p arcweft-lang-hir
cargo test -p arcweft-lang-hir
cargo check -p arcweft-runtime-plan
cargo test -p arcweft-runtime-plan
cargo check -p arcweft-compiler
cargo test -p arcweft-compiler
cargo check -p arcweft-bundle
cargo test -p arcweft-bundle
cargo check -p arcweft-runtime-driver
cargo test -p arcweft-runtime-driver
cargo check -p arcweft-lang-jit-cranelift
cargo test -p arcweft-lang-jit-cranelift
cargo check -p arcweft-runtime-codegen
cargo test -p arcweft-runtime-codegen
cargo check -p arcweft-runtime-accelerator
cargo test -p arcweft-runtime-accelerator
cargo check -p arcweft-save
cargo test -p arcweft-save
cargo check -p arcweft-verify
cargo test -p arcweft-verify
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
just structure-audit
just structure-audit-gate
just test-workspace
just test-tier2
```

There is deliberately no command for `arcweft-aot`, `arcweft-lang-aot-rust`,
or `arcweft-lang-vm`; none is a current workspace member.
