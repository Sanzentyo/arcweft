# Non-goals and compatibility statement

## Non-goals

This contract does not redesign:

- the strict Taplo-style schema-1 decoder or `SourceBackedManifest`;
- manifest canonical JSON/hash identities outside the existing topology
  transcript;
- project containment and normalized-path rules;
- Character nominal registration, alias resolution, source index, manifest
  schema, or package rendering behavior;
- typed `res` declaration schema/registry beyond using its accepted identity;
- Activity runtime artifact loading/binding (Lang-01.5.1.3);
- Stream scheduling, replay, backpressure, or save semantics (Lang-01.3.1);
- View, Style, dialogue presentation, CSS, or Takumi;
- bundle format version unless implementation evidence shows the existing
  Character package section cannot represent exact accepted files;
- general project graph relations unrelated to content roots;
- filesystem watcher implementation details beyond its typed input inventory.

## Compatibility

No evidence identifies a released schema, persisted user artifact, or external
consumer that requires the provisional Source root family, source `content`
declaration, text-only topology publication, or old content graph relation.
They are unreleased internal contracts and are replaced directly.

There SHALL be no:

- serde alias;
- dual reader/writer;
- deprecated enum variant;
- migration-only parser branch;
- old-spelling diagnostic;
- last-known-good candidate acceptance;
- directory inference;
- source-text resolver;
- hidden renamed Source inventory.

Existing accepted schema-1 manifest spelling and exact Character package layout
remain compatible because this contract changes admission ownership, not their
wire spelling.
