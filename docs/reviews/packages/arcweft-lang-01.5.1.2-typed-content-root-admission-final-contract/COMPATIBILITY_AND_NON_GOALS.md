# Compatibility statement and non-goals

## Compatibility statement

The manifest/resource/topology formats involved are unreleased internal contracts. Move directly to this one final model. There is no old accepted reader, no migration reader, no alias, no deprecated field, and no schema-version branch added for the discarded shape.

- Schema-1 `arcw.toml` remains schema 1; this correction changes admission products, not manifest transport.
- Existing text `ProfileTopologyOverlaySeed` remains text-only. Binary support is a distinct typed input, not a widened string carrier.
- `SourceSetRevision` remains valid for source-document indexes. It is not used as a topology/cache authority after binary admission.
- Existing `CharacterPackage`, Character manifest/source maps, strict project manifest decoder, generated metadata product, project containment, resource registry, reachability, content partition, bundle Character adapter, and LSP CAS publication are extended/consumed rather than replaced.

## Explicit non-goals

- no second project manifest decoder or canonical manifest encoder;
- no remote package fetch/provider execution;
- no generic content-addressed blob store or persistent cache design;
- no asset/package directory enumeration;
- no automatic inference of layers not named by the Character manifest;
- no redesign of Character nominal identity, View/dialogue admission, resource extension wire, generated-artifact binding, bundle container, or content partition algorithm;
- no binary text document, base64 TOML field, or `Arc<str>` byte coercion;
- no source `content` compatibility surface or removed-spelling diagnostic;
- no last-known-good candidate acceptance;
- no source gate;
- no CSS or Takumi route.
