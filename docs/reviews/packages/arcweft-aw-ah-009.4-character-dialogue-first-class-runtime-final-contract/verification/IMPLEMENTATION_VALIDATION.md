# Post-implementation validation procedure

This file specifies the commands to run against the implementing Arcweft
checkout. It is not evidence that implementation has already occurred.

## 1. Per-cut loop

For each cut in `IMPLEMENTATION_ORDER.md`:

1. run the exact changed-crate check/test/clippy commands listed for that cut;
2. run `cargo fmt --all -- --check`;
3. run the structure audit when the cut changes a public contract, dependency,
   codec, wire, or large owner;
4. record command, revision, exit status, and failures in the implementation
   note;
5. commit and push the coherent cut to `main` before starting an independent
   cut.

No validation command may search checked-in source text for a symbol spelling,
path, snippet, or absence. Deletion is proven by type/API compile-fail tests,
ordinary parser/type rejection, executable behavior, codec rejection, and Cargo
metadata dependency tests.

## 2. Final normal workspace gate

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
just test-fast
just test-rich-text
just test-cli-check
just test-workspace
just test-doc
git diff --check
```

## 3. Final slow/runtime gate

Because the contract changes runtime presentation, AWBC, save/restore, replay,
hot reload, Agent observation, and native/Web output:

```bash
just test-tier2
```

The milestone must additionally record:

- native/Web/headless parity for direct and dynamically selected
  CharacterDialogue values;
- Agent/MCP observe and dialogue advance behavior;
- AWBC ABI2/codec8 malformed and old-version rejection;
- display-catalog schema2 deterministic round-trip and old transcript rejection;
- save-schema2 exact restore and schema1 rejection;
- root replay v1 generic nominal-payload validation;
- compatible and stale hot reload;
- affected exact visual goldens and browser/wasm checks required by current
  repository policy.

## 4. Structural audit

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/aw-ah-009-4-final
```

The final implementation note must include the exact repository revision,
changed-file metrics, dependency fan-in/fan-out where relevant, error/warning
counts, and any resolved structural decomposition.

## 5. Completion rule

All 260 rows in `TEST_MATRIX.md` are normative. A test may be implemented by a
larger parameterized matrix, but its direct assertion must remain identifiable
in test output or the implementation note. Blocked environment-specific tests
are recorded as blocked with exact command/output; they are not reported as
passes. `OPEN_QUESTIONS=0` means no design decision remains, not that an
unexecuted implementation test is successful.
