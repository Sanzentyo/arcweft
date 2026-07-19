# AW-AH-009.3.1 Cut 1: call-surface syntax types

## Status

Implemented the independently mergeable Cut 1 substrate against parent Git
commit `760a89786092` and Jujutsu change `yzqxnpsl`. The source package is
`arcweft-aw-ah-009.3.1-call-surface-syntax-production-reconciliation-final-contract.zip`,
with SHA-256
`6ede771a895af981a583fdfd50a080f2eca57bf7a2925216cf725f7dbb418588`.

This cut intentionally does not replace the public `Expr::Call` variant or
connect the Pratt parser. Those changes form the unmerged direct-replacement
series in Cuts 2 through 4. A narrow `cfg_attr(not(test), expect(dead_code))`
on the private module documents this compile frontier and will become
unfulfilled, forcing its removal, when Cut 2 connects the parser-owned
constructors.

## Implemented contract

`arcweft-lang-syntax::expr` now publicly owns immutable, private-field types
for:

- `CallExpr` and exhaustive
  `CallSurfaceSyntax::{Parenthesized, CallbackBlock}`;
- `ParenthesizedCallSyntax` and `CallbackBlockCallSyntax`;
- exact `ArgumentListSyntax`, authored/recovered terminators, typed owner
  recovery boundaries, argument forms, and argument recovery state; and
- exact callback braces, explicit or implicit parameter headers, parameter
  type ascriptions, body range, and closing brace.

Read-only accessors expose semantic children and exact half-open UTF-8 byte
ranges. No public constructor or mutable accessor was added. Parser-facing
initializers, constructors, and `CallSyntaxInvariantError` remain
`pub(crate)`.

Construction validates exact punctuation bytes without searching the source,
checked base arithmetic, UTF-8 boundaries, delimiter and child ordering,
separator/trailing-comma shape, named/spread ranges, recovered diagnostic
containment, recovery insertion ownership, callback parameter/body shape, and
semantic argument count/form correspondence. A recovered missing `)` stores
only its exact insertion and owner boundary; no close-parenthesis range is
fabricated.

The callback surface deliberately exposes no `ArgumentListSyntax`.
Consequently, the future AW-AH-009.3 resolver can classify an outer callback
application as inapplicable while still selecting independently parsed nested
parenthesized calls.

## Superseded dialogue boundary

The package's old dialogue-specific `SpeakerLineSurface` and
`ContentCallSurface` clauses are superseded by the CharacterDialogue sequence.
This cut does not add either type, does not preserve the string
`ContentCall`, and does not add dialogue-only argument parsing. The shared
ordinary `ArgumentListSyntax` is ready for the final typed dialogue-content
application selected by AW-AH-009.4.2.

## Direct tests

The focused unit suite has 18 tests covering:

- empty, positional UTF-8, named, spread, and trailing-comma lists;
- exact cursor containment and comma/trailing-comma slot transitions;
- missing-close recovery at expression end and before an authored owner token;
- nonempty recovered argument diagnostics;
- explicit typed and implicit-zero callback headers;
- invalid UTF-8 boundaries and punctuation lengths;
- delimiter/child ordering, count/form mismatch, separator count, trailing
  comma, recovery boundary, callee range, callback argument/header/body, and
  checked-offset overflow failures.

The explicit zero-argument delimiter-order regression proves that an opening
parenthesis cannot follow its close or recovery insertion.

## Validation

All Cargo commands used `CARGO_INCREMENTAL=0`.

```bash
cargo fmt --all -- --check
cargo check -p arcweft-lang-syntax --all-targets
cargo test -p arcweft-lang-syntax call_syntax
cargo clippy -p arcweft-lang-syntax --all-targets -- -D warnings
cargo test -p arcweft-lang-syntax --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --lib --tests --exclude arcweft-cli --quiet
cargo test -p arcweft-cli --lib --bins --quiet
# plus the seven selected CLI integration binaries in `just test-workspace`
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/aw-ah-009-3-1-cut1-call-surface-types
git diff --check
```

Format check, focused all-target check, focused Clippy, and diff check passed.
The focused test command passed 18 tests. The complete syntax command passed,
including 256 library tests and all syntax integration/doc tests. Workspace
Clippy passed with all targets and all features. The normal non-CLI workspace
suite passed; the CLI library/binary suite passed 207 tests; and the seven
selected CLI integration binaries passed 22 tests.

Focused Clippy passed after replacing one needless by-value private initializer
with a `Copy` initializer. An earlier test compile exposed an incorrect
test-only `Name` conversion, and the first Clippy attempt exposed that by-value
initializer; both were fixed before the successful reruns.

Two unredirected non-CLI workspace invocations crossed the command watcher
while Windows was compiling and emitting the large test stream. The first
child Cargo job was allowed to finish its initial build. The second watcher
closure broke the test output pipe and caused an `arcweft-runtime-host` test
harness `SendError`; this was an execution-transport failure, not an assertion
failure. The exact command was then rerun with output redirected to a temporary
log and completed successfully.

The structural audit scanned 3,289 files, including 1,687 Rust files,
776,637 physical Rust LOC, and 92 package manifests. It reported zero errors
and 132 pre-existing warnings. The report is under
[`structure-audits/aw-ah-009-3-1-cut1-call-surface-types/`](structure-audits/aw-ah-009-3-1-cut1-call-surface-types/).

Current changed Rust-file measurements are:

| Path | Bytes | Physical LOC | Class | Responsibility |
| --- | ---: | ---: | --- | --- |
| `crates/arcweft-lang-syntax/src/expr.rs` | 33,454 | 1,255 | production root module | focused module declaration and deliberate public exports |
| `crates/arcweft-lang-syntax/src/expr/call_syntax.rs` | 31,811 | 943 | production responsibility module | immutable call surfaces, accessors, checked construction, invariants |
| `crates/arcweft-lang-syntax/src/expr/call_syntax_tests.rs` | 22,559 | 699 | unit test module | exact positive and negative invariant coverage |

`expr.rs` was already above the 1,200-LOC warning threshold; this cut adds only
the focused module declaration/export boundary rather than embedding the new
implementation there. `call_syntax.rs` remains below the production warning
threshold and is cohesive around one public ownership contract. No dependency
edge, Cargo feature, manifest, serialized format, source gate, compatibility
alias, dual AST, or removed-syntax diagnostic was added.

Tier 2 was not run because Cut 1 only introduces currently unused,
parser-private construction and immutable syntax carriers. It does not change
the public parser result, HIR, sema, runtime, rendering, Agent/MCP, capture, or
LSP execution path. Cuts 2 through 4 must run the risk-appropriate downstream
and Tier 2 gates when those paths are connected.

## Remaining Cuts 2 through 4

- replace the old struct-like `Expr::Call` directly with `Expr::Call(CallExpr)`;
- have the Pratt and callback parsers construct these checked surfaces from
  lexer tokens, including missing-`)` recovery;
- remove public source-less call constructors and migrate source tests to
  authored parsing;
- remove call-specific normalization/static-generic/source-reconstruction
  fallbacks;
- migrate exhaustive syntax/HIR/sema/runtime/verify/tooling consumers to
  read-only `CallExpr` accessors; and
- remove the temporary private-module dead-code expectation when parser
  construction makes it unfulfilled.

The competing word-only LSP signature resolver is not removed in this syntax
substrate cut. Its deletion belongs to the later AW-AH-009.3 sema/LSP migration
after the exact call carrier, accepted HIR request lifecycle, and shared
callable resolver are connected; deleting it here would remove current user
functionality without a replacement.
