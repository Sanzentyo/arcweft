# Seq04.8.4 normal build CLI goldens and cache evidence

Date: 2026-07-02

## Summary

This implementation adds user-visible CLI golden coverage for the normal
`arcw build` route. It does not redesign the seq04.1 through seq04.8.3 persistent
cache substrate. The new coverage exercises the same command shape users run,
normalizes volatile output, and verifies bytecode/link cache evidence through:

- `arcw build --json` cache records;
- generated `.snapshot.json` query statuses;
- `arcw cache explain --json --logical` persistent-query evidence;
- rebuilt versus reused AWFB and extracted `ProgramBytecode` AWBC bytes;
- corrupt persistent query record recovery from the normal CLI route;
- remaining typed conservative full-build multi-module bytecode/link evidence.

The package patch was malformed, so the overlay files and intended narrow hunks
were applied manually against the current checkout. The production behavior is
the same package slice: normal `arcw build` now has checked-in normalized CLI
goldens and a regeneration command wired into the fixture refresh inventory.

While running the workspace fast path, this cut also closed stale validation
gaps exposed by the current mainline:

- `Stream<T, E>` now participates in the standard `IntoIterator` evidence
  substrate, and generic type-pattern matching handles typed containers instead
  of relying on named generic strings.
- Runtime-plan tests that exercise `for` lowering now pass type-checker
  iteration evidence into `RuntimePlanLowerOptions`.
- Unsuffixed numeric bracket sequences remain dense `i32`, matching the current
  integer inference rule.
- Product AWBC facade environment parity compares root bindings by name, because
  named entry arguments are order-independent.
- The regression harness recognizes the audited Windows TSF `unsafe_com.rs`
  boundary and still requires nearby `SAFETY:` comments.

## Added files

- `fixtures/persistent-cache-build/seq04-8-4/normal-single/`
- `fixtures/persistent-cache-build/seq04-8-4/normal-conservative-multi/`
- `fixtures/persistent-cache-build/seq04-8-4/goldens/*.json`
- `crates/arcweft-cli/tests/seq04_8_4_persistent_cache_build_cli_goldens.rs`

## Fixture selection

`normal-single` is the minimal actual-reuse fixture: one package, one root module,
one compile unit, and a product AWFB whose `ProgramBytecode` section is a stable
bytecode unit for the full-build producer. It is the ordinary build version of
the seq04.8.2/seq04.8.3 actual identity proof.

`normal-conservative-multi` keeps the same command shape but adds a second module.
That shape remains conservative because the current linked product AWBC is not
narrowed into per-module or per-SCC reusable AWBC unit bytes.

## Golden policy

The checked-in JSON files are repository-owned normalized goldens rather than raw
CLI output. Raw fields that carry host paths, cache roots, usernames, compiler
hashes, target triples, object digests, artifact keys, object lengths, or local
timestamps are reduced to stable markers or omitted. Stable semantic fields stay
checked in: query names, logical items, statuses, producer families,
classifications, conservative reasons, recovery actions, and byte-stability
booleans.

## Validation commands

```bash
cargo test -p arcweft-cli --test seq04_8_4_persistent_cache_build_cli_goldens -- --nocapture
just persistent-cache-build-seq04-8-4-goldens
just test-workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
git diff --check
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-seq04-8-4
```

All commands above passed on 2026-07-03. The structure audit completed with
existing repository hotspot findings; this cut did not add a new error-level
Rust file.

## Regeneration

```powershell
just persistent-cache-build-seq04-8-4-goldens-regenerate
```

The regeneration command sets `ARCWEFT_REGENERATE_SEQ04_8_4_GOLDENS=1` for the
focused integration test and overwrites only the normalized JSON goldens.

## Remaining work

No seq04.8.4-specific follow-up is open. The conservative multi-module fixture
continues to document the broader seq04 limitation: linked product AWBC is still
not narrowed into independently reusable per-module or per-SCC unit bytes.
