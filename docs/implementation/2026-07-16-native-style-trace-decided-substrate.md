# Native Style trace decided-substrate cleanup

- Date: 2026-07-16
- Sequence: `seq-06.11d.5.1` (implementation-ready subset only)
- Parent revision: `5a068a0005cb1acae164ebc382e76ef7789a2671`
- Jujutsu change: `wxkysuzk`
- Package SHA-256:
  `2467ae195ba63515a58d875d3947a1569d620b770a5438d815d8f7f827aa2a64`

## Intake result

The supplied package, its manifest, README, request, normative design,
implementation note, Rust sketches, resolver overlays, patch, schema,
examples, and validation records were inspected against the current checkout.
The package is reference evidence, not an overlay to copy into this checkout.
Its model, prose, schema, examples, and projection disagree on the contract
items catalogued by the existing
[seq-06.11d.5.1.1 reconciliation request](../reviews/requests/2026-07-14-seq-06.11d.5.1.1-native-style-trace-contract-reconciliation.md).

The current checkout also does not contain the final d.4.2 environment or
d.4.3 container contracts. Consequently, the portable DTO, revision and
identity bindings, event order, redaction, cache envelope, paging, cursor,
strict codec, schema, examples, source projection, and adaptive evidence are
not implementation-ready. Adding them now would select result-changing policy
or duplicate predecessor-owned types.

One package integration requirement is independent of those open decisions:
the unreleased resolver must not retain the mutable `push(mode, entry)`
compatibility seam. The sole resolver, the three trace modes, winner
reconstruction, and full-mode computed-cache bypass are already fixed native
substrate. This cut implements only that independently decidable cleanup.

## Implemented contract

`arcweft-view::style::trace` now separates the public immutable trace result
from a resolver-private recorder whose state is selected once:

```text
Off      -> no evidence vector while resolving -> empty trace result
Winners  -> no detail vector while resolving   -> winners from computed provenance
Full     -> detail vector                       -> actual resolver branch evidence
```

The recorder exposes domain-specific contribution, rule-rejection, and
patch-rejection operations. Resolver call sites no longer pass a mode beside
an already-constructed event and therefore cannot mutate collection policy or
construct rejected detail records for `Off` and `Winners`. Full mode retains
the existing typed event behavior and continues to bypass the computed cache.
All three modes produce the same `ComputedViewStyle` for an identical snapshot.

This is an internal ownership change. It adds no crate, dependency, public
trace DTO, schema, diagnostic code, CSS/Takumi carrier, compatibility alias,
second resolver, or tooling surface.

## Acceptance evidence

The public resolver behavior test covers one identical node snapshot through
all three modes:

- `Off` is a cache miss with an empty trace;
- `Winners` is a computed-cache hit with a winner reconstructed from retained
  provenance;
- `Full` bypasses that cache and records the actual contribution;
- all computed results are equal.

No source spelling or source-file placement is used as correctness evidence.

## Structural measurement

The canonical audit measured the current checkout, not diff additions:

| Path | Owning crate | Bytes | Physical LOC | Category | Embedded test LOC | Responsibility |
| --- | --- | ---: | ---: | --- | ---: | --- |
| `crates/arcweft-view/src/style/resolver.rs` | `arcweft-view` | 42,848 | 1,182 | production | 0 | sole native cascade orchestration, cache/provider coordination, typed trace call sites |
| `crates/arcweft-view/src/style/trace.rs` | `arcweft-view` | 4,296 | 158 | production | 0 | trace mode, result event algebra, resolver-private collection policy |
| `crates/arcweft-view/tests/computed_style.rs` | `arcweft-view` | 28,677 | 927 | integration test | 0 | computed resolver, cache, trace-mode, budget, axis and predicate behavior |

No Cargo manifest or dependency edge changed. The recorder stays inside
`arcweft-view`; fan-in and fan-out are therefore unchanged. The resolver is
below the 1,200-LOC production warning and the integration test is below its
2,500-LOC warning. The repository audit scanned 2,743 files, including 1,306
Rust files and 641,144 Rust physical LOC, and reported 0 errors and 127
pre-existing warnings. The largest non-generated production Rust file remains
`crates/arcweft-core/src/value.rs` at 84,017 bytes and 2,500 physical LOC; this
cut does not touch it.

## Validation

| Command | Result |
| --- | --- |
| `cargo test -p arcweft-view --test computed_style --quiet` | Pass: 12 tests |
| `cargo test -p arcweft-view --all-features --quiet` | Pass: 123 tests across library and integration targets |
| `cargo check -p arcweft-view --all-targets --all-features` | Pass |
| `cargo clippy -p arcweft-view --all-targets --all-features -- -D warnings` | Pass |
| `cargo fmt --all -- --check` | Pass after formatting |
| `cargo +nightly -Zscript tools/structure-audit.rs --root .` | Pass: 0 errors, 127 warnings |
| PowerShell SHA-256 verification of `MANIFEST.sha256` | Pass: all 55 payload entries; 56 archive members including the manifest |
| `cargo check --workspace --all-targets --all-features` | Environment-blocked: the isolated JJ workspace does not materialize ignored `web/assets/noto-sans-jp-vf.ttf`; `arcweft-glyphon` and `arcweft-render-wgpu` test targets fail at `include_bytes!` |
| `cargo check --workspace --all-features` | Environment-blocked by the same absent ignored font in the `arcweft-player-scene` library target |

Validation used `CARGO_INCREMENTAL=0` and the stable target directory
`D:\git\arcweft-targets\native-style-trace`.
The focused all-target/all-feature Clippy run proves the changed crate. A final
all-workspace check must be rerun after landing in the main checkout, where the
ignored font asset is present; neither blocked command reported an error in a
changed file.

## Deferred acceptance and non-goals

The following remain explicitly outside this cut and outside its completion
claim:

- final environment/container fact and revision evidence from d.4.2/d.4.3;
- portable query/result/event/source/error DTOs and strict canonical codec;
- query/node/evidence identity ownership and duplicate-application policy;
- canonical event and diagnostic ordering, winner paging, priority redaction,
  cache disclosure, cursor layout/authentication, reference validation, and
  schema generation;
- Agent, LSP, CLI, renderer, filesystem, network, or persistent trace work.

Those items remain governed by the linked standalone reconciliation request.
That request already gives each contradictory decision, predecessor gate,
module-decomposition requirement, behavioral test matrix, and expected output;
no narrower replacement request is needed. Implementation must wait for its
decision-complete final contract after d.4.2 and d.4.3 land.

## Design deviations

- The package's old patch, whole-file resolver overlays, schema, examples, and
  package validator were not integrated.
- The package's two proposed trace crates were not added because their wire
  contract is disputed and predecessor-bound.
- The current internal event algebra was not expanded speculatively. This cut
  removes only the compatibility seam while preserving observable resolver
  behavior.
