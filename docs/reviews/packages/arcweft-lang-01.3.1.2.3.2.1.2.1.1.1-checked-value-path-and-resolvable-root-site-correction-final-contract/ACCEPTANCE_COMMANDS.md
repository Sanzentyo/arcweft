# Implementation acceptance commands

These are the required implementation-time gates, not commands claimed by this design-only archive. Run them from a clean checkout at the implementation commit, after each phase's narrow checks and again after Phase 15.

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test -p arcweft-interaction-model
cargo test -p arcweft-core value::ownership::path
cargo test -p arcweft-core pattern
cargo test -p arcweft-core plan
cargo test -p arcweft-core awbc
cargo test -p arcweft-lang-sema character_dialogue
cargo test -p arcweft-runtime-plan
cargo test -p arcweft-dialogue character_dialogue
cargo test -p arcweft-runtime-driver
cargo test -p arcweft-bundle
cargo test -p arcweft-save
cargo test --workspace --all-features
```

Required focused suites must include:

- human/non-human value-path golden bytes and all tags 0–10;
- `RuntimeIndexPath` empty/root/depth wire rejection;
- complete RuntimeExpr/RuntimePattern node tables and every plan typed site;
- every AWBC instruction/terminator/audio/table slot, bounds, duplicate, alias, and cycle case;
- plan-only, declaration-only, and paired-tamper admission fixtures;
- direct plan/AWBC equality transcript goldens without hashing;
- all physical RuntimeValue shapes, Bytes-as-Sequence, nested Choice/nominal paths, shared budget/depth;
- catalog generation provenance, custom-field View relationships, and absence of unsupported Character-to-View errors;
- atomic sema role issuance and compile-fail tests for private authority constructors/fields;
- raw plan/AWBC execution and unchecked nominal construction compile-fail fixtures.

Run the repository-owned structure/dependency audit and each Tier 2 command named by the latest root/scoped `AGENTS.md`; do not guess a script path that is not present at the implementation head.
