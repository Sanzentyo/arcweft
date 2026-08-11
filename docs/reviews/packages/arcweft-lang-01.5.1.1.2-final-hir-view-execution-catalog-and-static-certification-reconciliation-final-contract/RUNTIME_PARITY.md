# Runtime parity and lifecycle

## One evaluator

`BundleViewRuntime` remains the sole renderer-neutral evaluator. Its catalog maps a
subject to either an optional validated static fragment or ordinary instruction
range/value programs. The dispatch is internal:

```text
certificate present and validated -> execute same-schema fragment
certificate absent                -> execute dynamic instructions/programs
certificate present but invalid    -> reject candidate artifact/replacement
```

There is no `StaticViewRuntime`, renderer-specific static tree, or backend-specific
compiler.

## Canonical parity target

Both paths produce the same canonical retained result before backend projection:

- mount/instance path and exact program/revision-scoped node/instruction identity;
- element tree and paint order;
- text/RichText/display-frame value and source metadata;
- image resource identity and animation state;
- style/modifier/layout/scroll/navigation result;
- input controls, focus, selection, enabled state, values, and write-back;
- handler/event/action registration and typed inputs;
- part/export and semantic targets;
- source-map coordinates and diagnostics;
- Agent/MCP observation and redaction classification;
- save/replay semantic state and hot-replacement classification.

Native, Web, and headless consume this same result. Agent/MCP observe its shared
redacted projection. Generated artifacts bind the same program/certificate digests.

## Work certification may remove

- ordinary AWBC value-program invocations wholly inside the certified subject;
- dynamic instruction traversal for the certified range;
- repeated conversion/validation of folded constants;
- repeated immutable resource selection;
- repeated static modifier/layout normalization.

## Work that remains mandatory

- root/nested mount allocation and deterministic instance paths;
- parameter/default shape checks at call boundaries;
- retained state, input, selection, focus, and write-back slots;
- handler/action registration and dispatch;
- semantic target and exported-part publication;
- resource acquisition/release and animation logical time/frame selection;
- dynamic siblings/ancestors/descendants outside the certified subject;
- Agent/MCP observation and secret/masked redaction;
- source diagnostics;
- save/replay validation;
- hot-replacement generation and state reconciliation.

## Failure atomicity

Dynamic and static paths both write to `ViewFrameTransaction`. A failure discards
all mount mutations, input writes, resource leases, handler registrations, output,
and allocator changes. Hot replacement builds and validates a complete candidate
catalog, certificates, resources, and reconciled mount snapshot; active generation
and frame remain untouched until one commit. Save restore likewise builds a
candidate session and publishes once.
