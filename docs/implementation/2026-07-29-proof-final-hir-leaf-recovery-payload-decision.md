# Proof final-HIR leaf recovery payload decision

## Recovery context

- Recovered into the Proof public-switch worktree: 2026-08-07
- Inspected Git revision:
  `f587e75750d9c5d9b6d8c84e0f098a4cfa80f68b`
- Working tree: dirty Proof public-switch integration
- Validation authority:
  [`2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md`](2026-08-06-proof-concurrency-v6-1-1-full-matrix-closure.md)

This document restores a schema decision, not its former implementation or
validation state.

## Representable known-family recovery

EntityReference, LifetimePath, Path, and ShortVariant must remain their
recognized semantic families when malformed. Their valid payload types cannot
represent a missing/invalid identity, registry scope, segment, or name without
fabrication, so each uses an explicit resolved/recovered value.

Expression and Pattern entity references share one payload:

```text
HirIdRefValue::Resolved(HirIdRef)
HirIdRefValue::Recovered {
    shape: Absolute | Relative | FamilyRelative | Missing,
    issue: Missing | Invalid,
}
```

The Pattern-specific duplicate wrapper is not retained. `HirIdRef` remains a
strict valid identity and gains no sentinel state.

Expression-specific leaves use equivalent typed forms:

```text
HirLifetimePathValue::Resolved | Recovered
HirPathValue::Resolved | Recovered
HirShortVariantName::Resolved | Recovered
```

Lifetime recovery retains only scope presence, segment count, optional-marker
state, and typed issue. Path recovery retains a required parser-classified root,
segment count, and typed issue. ShortVariant recovery retains its exact name
invariant error. Recovery structures preserve only semantic/source-role shape;
text and ranges remain in the revision-bound source index.

## State, limits, and deletion

A resolved payload is clean unless another typed child poisons its owner. A
recovered payload requires the matching recovery issue and can never be clean.
Source validation rejects impossible roles and non-contiguous ordinals from the
recorded recovery shape.

Name, path, segment, and count limits are fatal rollback, not recovery payloads.
Unit has no malformed state. Placeholder exists only for authored `_` and `^`;
missing or unclassified markers are generic Error.

The direct migration deletes direct-only leaf payloads and duplicate wrappers,
then repairs all constructors/source consumers. It cannot add forwarding
aliases, dummy names/paths/IDs, compatibility readers, reparsing, or a second
source map. Current resolved/recovered, coherence, limit, role, and deletion
evidence remains owned by the full matrix.
