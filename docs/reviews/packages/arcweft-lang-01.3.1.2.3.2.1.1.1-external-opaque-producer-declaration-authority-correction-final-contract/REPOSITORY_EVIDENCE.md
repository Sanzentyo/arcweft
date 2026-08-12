# Repository evidence

## Basis and inspection mode

- Repository: `Sanzentyo/arcweft`
- Requested/current commit: `78f50f5b5ac082745bab91b7373a6602918a436d`
- Parent: `7636b61a1c4c8e81127cb81a8fd27ef765d5ce2a`
- Commit message: `Request external opaque producer authority`
- Commit change scope: the new request and parent-return intake update
- Source request SHA-256: `a7ab7d47f50804bae5a5b9fff1e5e39b7c97922bdda191b444216724d56ba9a7`
- Parent retained ZIP SHA-256: `93af482a2914ca4a9e6b985aa7a09c040f569bd71141611dcaa4d579ac01640c`

The source and parent frozen mirror were inspected read-only through GitHub/raw
surfaces at the exact commit. A complete writable Cargo checkout was not
available in the artifact runtime, so production compilation and tests were not
run. Local files were used only as static mirrors and to build/validate this
archive. No repository production file was edited.

## Concrete current gaps observed

- `AdapterNominalDeclaration` currently contains path, arity, visibility, and
  source label only.
- `AdapterManifest::try_with_rust_manifest` validates the Rust manifest and
  clones each `ArcweftRustTypeDecl` into `AdapterRustType`; the derived type has
  no authored producer field.
- adapter codec schema is 1 and decodes body before a producer can exist.
- standard helper declarations construct producerless nominal rows.
- `ARCWEFT_RUST_ABI_SCHEMA_VERSION` is 1 and `ArcweftRustTypeDecl` has no
  producer.
- the derive currently has no mandatory producer helper input.
- adapter-sema generated source/digest/publication has no producer field.
- accepted environment inventory and current accepted opaque semantics are
  producerless at this external boundary.
- accepted nominal substitution reconstructs the type without producer
  evidence.

These facts are the narrow defect this design corrects. They do not provide
authority to redesign the parent runtime model.

## Static evidence limitations

Exact Rust snippets in this package are selected target APIs. They were not
compiled. Exact inventory closure is defined semantically and by mandatory G0
search commands; a production implementer must materialize all matches at the
implementation head before editing. This does not leave an open design choice:
every match belongs to the same mandatory migration/deletion rule.
