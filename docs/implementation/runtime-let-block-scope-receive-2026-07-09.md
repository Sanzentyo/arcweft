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

## 2026-07-10 Structural Follow-Up

The first cut still classified bare `Expr::Block` directly in
`lower_let_stmt`. This follow-up replaces that local branch with two explicit
flow-lowering boundary types:

- `FlowValueBlock` classifies value-producing flow blocks that lower through
  lexical runtime scope: bare blocks, named blocks, and computation blocks.
  `MemoBlock` is intentionally excluded because cache policy needs dedicated
  memo lowering rather than plain scope lowering.
- `LoweredLetBinding` carries the lowered ops plus optional function arity, so
  `lower_let_stmt` records binding metadata once after delegating to
  `lower_let_binding`.

The runtime-plan regression now covers both:

```arcw
let submitted = {
    let event = receive action(@action:.feedback.submit)
    event.value
}

let submitted = result {
    let event = receive action(@action:.feedback.submit)
    event.value
}
```

Additional validation:

```bash
cargo fmt
cargo test -p arcweft-runtime-plan receive_action_inside_ -- --nocapture
cargo test -p arcweft-runtime-plan --test runtime_plan -- --nocapture
cargo test -p arcweft-cli --test native_text_input_sample_sidecars -- --nocapture
cargo run -p arcweft-cli -- check samples/modern-feedback-view/src/main.arcw
cargo run -p arcweft-cli -- bundle samples/modern-feedback-view/src/main.arcw --output web/modern-feedback-view.awfb
cargo clippy -p arcweft-runtime-plan -p arcweft-cli --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root . --write target/structure-audit-codex-let-binding-structure
```

Structural audit result:

- Revision: `runlwmvknovovztxqutlwqukwzuurrln` / `9dd46e852621`
- Files scanned: 2566
- Rust files: 1189
- Rust physical LOC: 589125
- Violations: 1 error, 152 warnings

Changed Rust file metrics:

| Path | Crate | Kind | Bytes | Physical LOC | Embedded tests | Responsibilities |
| --- | --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-runtime-plan/src/flow.rs` | `arcweft-runtime-plan` | production | 90131 | 2468 | no | Flow runtime lowering orchestration and dispatch |
| `crates/arcweft-runtime-plan/src/flow/binding.rs` | `arcweft-runtime-plan` | production | 578 | 27 | no | Lowered let-binding result and function-arity metadata |
| `crates/arcweft-runtime-plan/src/flow/syntax_helpers.rs` | `arcweft-runtime-plan` | production | 3134 | 100 | no | Flow syntax helper extraction for dialogue calls, task names, runtime IDs, and traverse/parallel parsing |
| `crates/arcweft-runtime-plan/src/flow/value_block.rs` | `arcweft-runtime-plan` | production | 1105 | 39 | no | Value-producing flow block classification |
| `crates/arcweft-runtime-plan/tests/runtime_plan.rs` | `arcweft-runtime-plan` | integration test | 58963 | 1939 | no | Runtime-plan lowering regressions |

The remaining error-level structural violation is still existing:

- `crates/arcweft-cli/src/app/bundle_view.rs`: 2590 physical LOC.

## 2026-07-10 AWBC Entry Parameter Follow-Up

The structured let-binding cut exposed a second boundary bug in AWBC lowering.
`FlowOp::LetScope` was semantically correct, but AWBC entry-parameter
inference collected free locals in the block body and final block value with
different lexical scopes. For:

```arcw
let visitor_name = {
    let name_event = receive action(@action:.feedback.submit_name)
    name_event.value
}
```

the `name_event` local was incorrectly treated as an undeclared entry
parameter while inferring the entry flow signature. The generated AWBC product
therefore expected one startup argument, and web/native runtime startup failed
with:

```text
AWBC entry expects 1 arguments, received 0
```

The runtime then produced an empty presentation frame, which made the WebGPU
player show only the background image.

The fix is in AWBC free-local collection, not in the sample or renderer:
`LetScope` now collects its body ops and final value expression under one
temporary value-scope declaration set, then restores the outer declaration set
before declaring the parent binding pattern. This matches the lowering model
used by the executable flow: locals introduced by earlier block statements are
visible to the final block value but do not leak out of the block.

Additional regression coverage:

- `entry_parameter_inference_keeps_let_scope_locals_inside_block_value` checks
  that both the entry signature and target flow function stay zero-arity, then
  executes the lowered AWBC entry and returns the block value.

Additional validation:

```bash
cargo fmt
cargo test -p arcweft-runtime-plan entry_parameter_inference_keeps_let_scope_locals_inside_block_value -- --nocapture
cargo test -p arcweft-compiler lower_source_runtime_plan -- --nocapture
cargo build -p arcweft-cli
target/debug/arcw.exe check samples/modern-feedback-view/src/main.arcw
target/debug/arcw.exe bundle samples/modern-feedback-view/src/main.arcw --output web/modern-feedback-view.awfb
target/debug/arcw.exe run-bundle web/modern-feedback-view.awfb --steps 3 --mode game --max-ops 64 --json
target/debug/arcw.exe agent observe samples/modern-feedback-view/src/main.arcw --steps 8 --mode game --max-ops 64 --resource observation --json
cargo +nightly -Zscript tools/structure-audit.rs --root .
cargo clippy --workspace --all-targets --all-features
```

Observed results:

- `run-bundle` reaches `dialogue modern_feedback_view.concierge.001` with no
  `AWBC entry expects` diagnostic.
- `agent observe` reports status `ok`, final status
  `dialogue modern_feedback_view.concierge.001`, 2 objects, 2 views, and
  `action.advance_text.object.dialogue.7.0`.
- Browser verification on
  `http://127.0.0.1:4173/?bundle=./modern-feedback-view.awfb&cachebust=codex-entry-scope-fix-001`
  shows the background and concierge dialogue instead of a solid green frame.
- Workspace clippy completes successfully, with existing warnings outside this
  cut still reported.
- Structural audit summary: 2566 files scanned, 1189 Rust files, 589183 Rust
  physical LOC, 1 existing error, 152 warnings.

Changed Rust file metrics:

| Path | Crate | Kind | Bytes | Physical LOC | Embedded tests | Responsibilities |
| --- | --- | --- | ---: | ---: | --- | --- |
| `crates/arcweft-runtime-plan/src/awbc_lower/flow.rs` | `arcweft-runtime-plan` | production | 79548 | 2055 | no | AWBC flow lowering and entry free-local collection |
| `crates/arcweft-runtime-plan/src/awbc_lower/tests.rs` | `arcweft-runtime-plan` | unit test | 14182 | 397 | no | AWBC lowering and VM regression tests |

## 2026-07-10 AWBC LetScope Cleanup Follow-Up

The previous follow-up fixed entry arity but also exposed a lifetime mismatch:
the structured Sans I/O flow engine expands `FlowOp::LetScope` into a scoped
sequence, while AWBC lowering emitted only the block body, final value, and
parent binding. Presentation handle cleanups registered by a view inside a
value-producing block were therefore stored on the surrounding/root cleanup
stack instead of the block cleanup stack. In the modern feedback sample this
meant `name_panel` remained visible after:

```arcw
let visitor_name = {
    let name_panel = view(@view:.ModernFeedbackNamePanel)
    let name_event = receive action(@action:.feedback.submit_name)
    name_event.value
}
```

even though the binding's lexical block had ended.

AWBC lowering now treats `LetScope` as a real runtime scope:

1. emit `EnterScope`;
2. lower block statements;
3. evaluate the block value;
4. move the value into a root-scope temporary;
5. emit `ExitScope`, which drains scoped cleanups;
6. bind the saved value to the parent pattern.

This preserves the intended order: the block value is computed before cleanup,
then presentation handles registered inside the block are disposed before the
parent binding continues.

Additional regression coverage:

- `let_scope_exit_emits_registered_cleanup_before_parent_binding` executes the
  lowered AWBC entry and verifies that leaving the let-scope emits the
  registered cleanup effect while still returning the block value.

Additional validation:

```bash
cargo fmt
cargo test -p arcweft-runtime-plan let_scope_exit_emits_registered_cleanup_before_parent_binding -- --nocapture
cargo test -p arcweft-runtime-plan entry_parameter_inference_keeps_let_scope_locals_inside_block_value -- --nocapture
cargo test -p arcweft-runtime-plan --test runtime_plan receive_action_inside_ -- --nocapture
cargo build -p arcweft-cli
target/debug/arcw.exe check samples/modern-feedback-view/src/main.arcw
target/debug/arcw.exe bundle samples/modern-feedback-view/src/main.arcw --output web/modern-feedback-view.awfb
target/debug/arcw.exe run-bundle web/modern-feedback-view.awfb --steps 3 --mode game --max-ops 64 --json
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/structure-audit.rs --root .
```

Browser verification on
`http://127.0.0.1:4173/?bundle=./modern-feedback-view.awfb&cachebust=codex-let-scope-cleanup-002`
confirmed that the name panel disappears after submitting the name action.
