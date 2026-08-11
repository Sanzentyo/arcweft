# Non-goals and forbidden designs

The following are explicitly outside or forbidden by this final contract:

- removing `Ref<Entity>` or replacing it with a different surface syntax;
- adding `Ref` to `AcceptedNominalSemantics`;
- adding a second contextual registry beside `BuiltinTypeConstructor`;
- resolving an entity family through a display string, uppercase heuristic,
  project lookup fallback, accepted/open rule, or `EntityKind::Other` synthesis;
- restoring `From<&TypeRef> for TypeKind`;
- a consumer-local helper that recognizes the spelling `Ref`;
- `TypeKind::Named("Ref<...>")` or `TypeShape::Named("Ref<...>")`;
- compatibility aliases, dual readers, old/new schema branches, or a fallback
  success result;
- a permanent removed-spelling diagnostic;
- a dedicated syntax/HIR `Ref` node when the existing generic source map is
  sufficient;
- redesigning project declaration identity, source maps, accepted catalog
  limits, poison IDs, cache keys, or resolution index topology;
- making `EntityKind::Other` source-authorable;
- inventing a source definition for `Ref` or entity-family atoms in LSP;
- treating an invalid project nominal argument as a valid rename/reference edge;
- adding a persisted entity-reference wire shape without a separate versioned
  save/replay contract;
- production code changes in this ZIP.
