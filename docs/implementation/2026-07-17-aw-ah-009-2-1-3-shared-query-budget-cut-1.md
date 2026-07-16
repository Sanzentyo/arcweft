# AW-AH-009.2.1.3 shared query budget — Cut 1

## Status and basis

The independently compiling request-budget substrate is implemented in
Jujutsu change `nonqyspr` on Git base `a8403dcb26d7`. The source package is
`arcweft-aw-ah-009.2.1.3-shared-query-budget-verification-reconciliation-final-contract.zip`
with SHA-256
`2dc2c2ee3c4425029639349273ace0b9eb9b323432be2426d67d133c2fdee52b`.

This cut implements only the sema-owned budget types and their state machine.
It does not claim the complete AW-AH-009.2.1.3 request path. In particular,
semantic collection/query charging, cache receipts, target-source adaptation,
and final accepted-request checks remain outside this independently pushable
cut.

## Implemented contract

- `arcweft-lang-sema::character_definition` owns the protocol-neutral
  `CharacterDefinitionRequestBudget`, work-kind enum, opaque checkpoint, and
  ordered receipt.
- The budget is request-local and does not implement `Clone`, `Copy`,
  `Default`, serialization, or an integer conversion. Its only public
  constructor reads the existing inclusive production query-work maximum of
  4,096; the reduced-limit constructor is same-crate test-only.
- All eight work kinds contribute one-for-one to one ordered transcript. The
  operation that proves one-over is appended before the canonical limit error
  becomes terminal.
- Addition, sequence-length conversion, checkpoint subtraction, and receipt
  replay are checked. A counter/receipt inconsistency becomes a stable terminal
  arithmetic-overflow result; no saturation, wrapping, bulk replay, or public
  limit override exists.
- Ordered receipts clone only the bounded post-checkpoint slice. Replay
  re-enters the ordinary per-unit `charge` transition and therefore preserves
  the exact first failing work kind.
- Direct unit tests cover every work kind at exact and one-over reduced limits,
  the 4,096/4,097 production boundary, empty and ordered receipts, terminal
  replay, impossible checkpoints, addition/conversion overflow, and independent
  concurrent requests.

## Atomic integration boundary

Cuts 2 through 4 directly replace the public semantic collection and query
signatures with a required mutable budget. The only production caller is the
LSP character-definition request path. Therefore those cuts cannot be pushed
alone without either breaking the workspace or adding a prohibited
compatibility overload/per-call budget.

The next atomic compiling cut must occur only after the accepted launch-profile
overlay and source-adapter contracts are present. It must:

1. create one production budget before accepted-source acquisition;
2. pass that same mutable borrow through semantic inventory and query work;
3. pair generation-local semantic cache entries with ordered receipts and
   replay before observing cached values;
4. pass the same borrow through the selected target-source adapter, range
   conversion, link materialization, and final accepted-request stamp checks;
5. keep `LocationLink` values and source-availability failures uncached; and
6. land the sema Cuts 2–4 and LSP Cuts 5–8 together, with no old-signature
   overload, second counter, global state, or limit override.

The sema Cuts 2–4 may be developed and focused-tested in an isolated descendant,
but they remain unpushable until that atomic handoff is available.

## Structural result

The canonical report is
[`structure-audits/aw-ah-009-2-1-3-request-budget-cut1/`](structure-audits/aw-ah-009-2-1-3-request-budget-cut1/).
It scanned 3,069 files, including 1,535 Rust files and 706,010 Rust physical
LOC, and reported zero error-level violations and 130 repository-wide warnings.

| Path | Bytes | Physical LOC | Class | Major responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-lang-sema/src/character_definition.rs` | 35,073 | 1,024 | production | public character-reference/query model and existing query implementation |
| `crates/arcweft-lang-sema/src/character_definition/request_budget.rs` | 6,006 | 193 | production | request-local budget, checkpoint, receipt, and replay state machine |
| `crates/arcweft-lang-sema/src/character_definition/tests/budget.rs` | 9,400 | 310 | unit test | exact/one-over, terminal, receipt, arithmetic, and concurrency evidence |

The sema crate's normal dependency fan-in/fan-out remains `8/10`; this cut adds
no crate dependency, Cargo feature, unsafe code, source gate, compatibility
module, serializer, global counter, protocol type, or I/O boundary. The root
character-definition module remains below the 1,200-LOC production review
threshold, while budget implementation and tests have separate responsibility
files.

## Validation

Rust commands use `CARGO_INCREMENTAL=0`:

```bash
cargo test -p arcweft-lang-sema character_definition::request_budget::tests::budget -- --nocapture
cargo check -p arcweft-lang-sema --all-targets --all-features
cargo clippy -p arcweft-lang-sema --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-2-1-3-request-budget-cut1
jj diff --git | git apply --numstat --whitespace=error-all
```

The focused budget suite passes 26 of 26 tests. The changed crate check and
Clippy pass with warnings denied; format and diff whitespace checks pass.
Workspace check reached and compiled `arcweft-lang-sema`, then was externally
blocked because this checkout does not contain the untracked
`web/assets/noto-sans-jp-vf.ttf` included by the `arcweft-glyphon` integration
test and `arcweft-render-wgpu` unit tests. Workspace Clippy and
`just test-workspace` are deferred to the atomic sema/LSP integration cut under
the same missing-asset prerequisite; this substrate alone changes no caller or
runtime behavior.
