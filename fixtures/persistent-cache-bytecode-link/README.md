# Fixture: persistent-cache-bytecode-link

This fixture describes the expected behavior for seq04.8 conservative bytecode
and link-plan gates.

## Scenario

1. Build the fixture project once with incremental cache enabled.
2. The build writes typed `.awbo` gate records for:
   - `QueryKind::BytecodeUnit` / `CompilerObjectKind::BytecodeUnit`;
   - `QueryKind::LinkPlan` / `CompilerObjectKind::LinkPlan`.
3. Build the same source again.
4. The cache adapter reads the bytecode/link gate records, validates their exact
   compiler identity, source digest, options, dependency body digest root, and
   deterministic gate facts.
5. The records are reported as `HitThenRebuilt`, not as reusable `Hit`.
6. Product bytes are identical because bytecode generation/linking still rebuilds
   from source.

## Expected explain properties

See `expected-cache-explain.json`.
