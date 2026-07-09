# Runtime Let Block Scope Receive Cut

Date: 2026-07-09

## Summary

This cut fixes the runtime-plan gap where `check` accepted a value-producing
block but `bundle` rejected effectful statements inside it.

- A flow statement of the form `let value = { ... }` now lowers a bare block
  expression to `FlowOp::LetScope`.
- The block statements and final value expression lower under the same
  compile-time scope, so locals introduced before the value expression remain
  available while the value is lowered.
- `receive action(...)` inside that block lowers through the existing
  `view.action.await` suspend host call path.
- The runtime already expands `LetScope` as `EnterScope`, body ops,
  `ExitScopeBind`; this evaluates the value before cleanup and binds it in the
  parent scope. View handles retained inside the block therefore remain alive
  while awaiting the action, then leave scope naturally.
- Generated runtime line IDs now use a non-reserved `dialogue` segment instead
  of `line`, avoiding invalid canonical IDs such as
  `line_handles.line.0`.

## Sample State

`samples/modern-feedback-view/src/main.arcw` now uses the ordinary flow shape
that motivated the fix:

```arcw
let visitor_name = {
    let name_panel = view(@view:.ModernFeedbackNamePanel)
    let name_event = receive action(@action:.feedback.submit_name)
    name_event.value
}
```

The one-line name panel is mounted only while the name action is awaited. The
multi-line brief panel is kept in a separate lexical block so it remains
retained while its receive point is active and is cleaned up when that block
ends.

## Validation

Commands run:

```bash
cargo fmt
cargo test -p arcweft-runtime-plan receive_action_inside_let_block_lowers_to_scope_value -- --nocapture
cargo test -p arcweft-runtime-plan --test runtime_plan -- --nocapture
cargo test -p arcweft-cli --test native_text_input_sample_sidecars -- --nocapture
target/debug/arcw.exe check samples/modern-feedback-view/src/main.arcw
target/debug/arcw.exe bundle samples/modern-feedback-view/src/main.arcw --output web/modern-feedback-view.awfb
cargo clippy -p arcweft-runtime-plan -p arcweft-cli --all-targets --all-features
```

Browser verification on
`http://127.0.0.1:4173/?bundle=./modern-feedback-view.awfb&cachebust=codex-let-block-scope-002`
confirmed the WebGPU player creates a canvas for the rebuilt bundle and reports
no console warnings or errors.

Known validation notes:

- Focused clippy exits successfully but still reports existing warnings outside
  this cut: large syntax AST variants, sema `too_many_lines`, runtime-driver
  `Option<Option<_>>`, runtime-host elidable clipboard lifetimes, and native
  clipboard match/pass-by-value warnings.

## Structural Audit

Command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-codex-let-block-scope
```

Result:

- Revision: `wswpwvmrszwuynoryppnvqxmuvqxqnop` / `3e6c80861bcb`
- Files scanned: 2564
- Rust files: 1187
- Rust physical LOC: 588982
- Violations: 1 error, 152 warnings

Changed Rust file metrics:

| Path | Crate | Kind | Bytes | Physical LOC | Embedded tests | Responsibilities |
| --- | --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-runtime-plan/src/flow.rs` | `arcweft-runtime-plan` | production | 89123 | 2435 | no | Flow runtime lowering, scoped body lowering, dialogue/action/view lowering dispatch |
| `crates/arcweft-runtime-plan/src/flow/syntax_helpers.rs` | `arcweft-runtime-plan` | production | 3134 | 100 | no | Flow syntax helper extraction for dialogue calls, task names, runtime IDs, and traverse/parallel parsing |
| `crates/arcweft-runtime-plan/tests/runtime_plan.rs` | `arcweft-runtime-plan` | integration test | 57794 | 1895 | no | Runtime-plan lowering regressions |
| `crates/arcweft-cli/tests/native_text_input_sample_sidecars.rs` | `arcweft-cli` | integration test | 4969 | 94 | no | Sample source assertions |

The remaining error-level structural violation is existing:

- `crates/arcweft-cli/src/app/bundle_view.rs`: 2590 physical LOC.

During this cut `flow.rs` briefly crossed the 2500 LOC error threshold. The
helper extraction above reduced it to 2435 LOC without changing lowering
behavior.
