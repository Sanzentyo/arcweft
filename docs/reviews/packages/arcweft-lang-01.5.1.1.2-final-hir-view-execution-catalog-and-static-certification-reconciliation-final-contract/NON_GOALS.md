# Constraints and non-goals

This contract does **not**:

- restore the deleted flattened-HIR/AST View lowerer or copy its schema;
- add source reconstruction, span/string expression recovery, a second parser,
  compiler-side or endpoint checked catalog, or stringly callable/resource lookup;
- add `ViewRuntimeValue`, a View-specific expression VM, parallel bytecode, or a
  generic resource `Presentable` trait;
- add a new AWFB section, ViewProgram V2 compatibility wrapper, dual reader/writer,
  deprecated field, alias, migration shim, source gate, or removed-syntax diagnostic;
- add `Image` to `ViewElementKind`, treat every resource as a retained entity, or
  collapse ResourceRef/AssetRef/retained identity categories;
- add APNG support, new image formats, provider I/O, filesystem loading, or network
  behavior to lower/data crates;
- add `mount`, Action emit/receive, a shared View parser, Dialogue
  `#call()[content]`, Ruby, try/pipe, Choice, broader lexical identity, persistent
  reference redesign, or new Style naming;
- revive CSS or Takumi;
- redesign CharacterDialogue, dialogue profile admission, typed RichText,
  ordinary-function/direct-suspension, Proof syntax/HIR identity, resource registry,
  prepared text, renderer geometry, or current persistent identities without a new
  concrete production contradiction;
- serialize snapshot-local HIR IDs, source ranges as semantic identity, decoded
  renderer frames, certificate selections, fragment caches, or static-path markers
  into save data; or
- claim production Cargo, Clippy, browser, native, Tier-2, or structural validation
  from this design-only packaging run.
