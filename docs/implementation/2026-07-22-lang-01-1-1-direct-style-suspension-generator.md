# Lang-01.1.1 direct-style suspension and generator classification

Date: 2026-07-22

## Outcome

Ordinary `fn` declarations now publish a typed semantic execution fact instead
of relying on the removed author-facing `task fn`, `dialogue fn`, or
`stream fn` roles:

- `CheckedCallableExecution` is keyed by canonical
  `CallableDeclarationId`;
- `CallableExecutionMode::DirectFrame` covers ordinary direct execution and
  `Stream<T, E>` passthrough functions with no own-scope `yield`;
- `CallableExecutionMode::StreamFactory` covers an ordinary function whose
  resolved return type is `Stream<T, E>` and whose own body suspends through
  `yield`;
- `StreamGeneratorFacts` records the resolved element/error contract and the
  retained suspension sites; and
- an own-scope `yield` contributes the direct `control.suspend` effect.

The suspension walk crosses ordinary expression/control-flow containers but
does not steal `yield` from another execution owner such as a closure,
`Seq`/nested stream body, thread, event handler, dialogue body, or source
owner.

Maintained design chapters, examples, and positive fixtures now use ordinary
`fn`. Historical design packages, implementation records, and negative
removed-syntax fixtures remain historical evidence and were not rewritten.

## Completion boundary

This cut establishes the semantic authority required by the later parser/HIR
and runtime switches. It does not yet:

- delete the provisional `FunctionKind` variants and their parser branches;
- connect the checked execution fact to runtime-plan/AWBC lowering; or
- publish Stream ABI/codec wire changes.

Those changes remain ordered behind the corrected project nominal resolver and
the pending Stream runtime-wire correction. No provisional
`CheckedReturnTarget`, compatibility alias, dual reader, or source gate was
introduced.

## Verification

- focused execution-mode tests: 5 passed;
- `cargo test -p arcweft-lang-sema --lib --quiet`: 861 passed;
- `cargo check -p arcweft-lang-sema --all-targets`: passed;
- `cargo clippy -p arcweft-lang-sema --lib --no-deps -- -D warnings`: passed;
- format check and diff check: passed.
