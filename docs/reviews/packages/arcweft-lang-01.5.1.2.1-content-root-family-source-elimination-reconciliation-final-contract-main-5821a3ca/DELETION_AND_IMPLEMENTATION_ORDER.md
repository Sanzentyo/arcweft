# Deletion inventory and implementation order

## 1. Non-negotiable sequencing

The content-root family must be frozen against the final post-Source typed
symbol inventory, not against the current transitional tree.

Required production order:

1. Land/reuse the Lang-01.5.1.2 binary topology, binary overlay,
   `CharacterPackage`, exact source accessor, and transaction substrate that is
   independent of the root-family enum.
2. Complete the Lang-01.1.1 ordinary function/generator and project-symbol
   substrate required by Lang-01.3.1.
3. Complete Lang-01.3.1 Source elimination, including runtime/AWBC/wire
   migration, and consume its final typed symbol/callable inventory.
4. Freeze the closed post-Source `ContentRootFamily`,
   `AuthoredContentRootFamily`, and `AcceptedContentRootTarget` shapes from this
   package.
5. Add project-wide typed root/reference collection and atomic admission.
6. Delete source `content` syntax/HIR/sema/tooling ownership in the same
   migration that publishes manifest-owned content facts.
7. Migrate compiler, bundle, watch, LSP, CLI, fixtures, and diagnostics to the
   accepted typed inventory.
8. Run focused tests, workspace tests, Tier 2, and the structural audit.

No intermediate accepted state may contain both old source-content ownership
and manifest content facts, or both Source/Stream runtime paths.

## 2. Source declaration/runtime deletion inventory

The Lang-01.3.1 migration deletes rather than aliases:

### Syntax/parser

- `arcweft_lang_syntax::ast::source::SourceItem`;
- `Item::Source`;
- source parser dispatch/CST kinds/accessors;
- source-specific parser diagnostics/lints/completion/formatting;
- `FunctionKind::Stream` or `stream fn` author surface where superseded by the
  selected ordinary `fn -> Stream<T, E>` generator model;
- Source syntax fixtures as accepted programs.

### HIR/sema

- `HirSource` and `HirTopLevelDecl::Source`;
- `TypeKind::Source`;
- `EntityKind::Source`;
- `EntityKind::from_type_name("Source")`;
- Source-specific checker, canonicalization, lifetime, trait, symbol, project
  index, and callable paths;
- Source-specific aliases/reexports and compatibility readers.

### Public/runtime

- public `Source<T, E>`;
- `SourcePlan`, `SourcePolicy`, `SourceRuntimeState`;
- Source event kinds/requests/effects and `RuntimePlan.source_plans`;
- separate `source_events` ingress or a second Source scheduler/queue owner;
- Source save/hot-swap/runtime observation rows.

### AWBC/data/wire/host

- Source tables, policies, handlers, opcodes, verifier/codec/VM/fiber state;
- Source snapshot/save rows;
- Source bundle/runtime-driver/runtime-host/player/CLI/LSP/tooling projections;
- any dual Stream/Source wire reader or compatibility decoder.

The final owner for asynchronous sequences is the typed Stream model. This
content-root contract neither duplicates nor modifies its runtime semantics.

## 3. Source `content` deletion inventory

The manifest owns content units and root occurrences. Delete rather than
redirect:

### Syntax/parser

- `EntityDeclKind::Content`;
- its `"content"` keyword arm;
- `EntityDeclBody::Content`;
- `ContentDeclBody`;
- content-declaration body parser and source fixtures;
- parser completion/formatting that treats `content` as a declaration family.

### HIR/sema/index

- the executable/typed source content declaration node;
- `EntityKind::Content` as a source declaration family;
- source-content root lists in HIR;
- source-content symbol/index publication;
- `index_content_root_relations` from source-owned content IDs;
- the old PublicId-to-PublicId `ProjectGraphRelationKind::ContentRoot` producer;
- any compiler/runtime node created only to execute a content declaration.

### Tooling/docs/fixtures

- LSP symbols/completion/definition for source content declarations;
- formatter/highlighter declarations for accepted source content;
- bundle/watch discovery through source content;
- examples and fixtures that compile source `content`;
- documentation that calls it an executable or compatibility surface.

The replacement is direct manifest-owned `ProjectContentUnitFact` and
`ProjectContentRootFact` publication. No compatibility `Content` entity is
created.

## 4. Retained source-provenance names

Source elimination does **not** delete compiler provenance terminology:

- crate/module names such as `arcweft-source`;
- `SourceDocument`, `SourceDocumentId`, `SourceDocumentIdentity`;
- `SourceSpan`, `SourceRange`, `SourceAnchor`;
- `SourceBackedManifest` and `SourceBackedCharacterManifest`;
- AWBC source maps and diagnostic source locations;
- ordinary variable/field words meaning provenance rather than the removed
  runtime entity/type.

These are not aliases for the removed declaration or runtime abstraction.

## 5. Family implementation cut

After Source elimination:

1. Add `AuthoredContentRootFamily` and `ContentRootFamily` to
   `arcweft-project::content`.
2. Add the `AuthoredEntity` target variant and no legacy alias.
3. Extend `EntityKind`'s own inherent impl with final root classification.
4. Extend the accepted resource declaration index's own impl with exact lookup.
5. Replace the loader's `strip_prefix("@character.")` classification with a
   typed Character acquisition request produced from accepted manifest
   references.
6. Build `ContentAdmissionCandidate` from exact manifest spans and Character
   acquisition results.
7. Resolve semantic-pending roots through the final accepted symbol/resource
   world.
8. Add optional Character reservations to typed reference collection.
9. Finalize `AcceptedContentInventory`.
10. Extend ProjectIndex graph endpoint/relation enums in place and delete the
    source-content relation producer.

No extension trait, loader-local enum clone, ad hoc family helper, or
name/return-type heuristic is permitted.

## 6. Consumer migration cut

The same accepted inventory is passed to:

- compiler reachability/partition planning;
- bundle entry construction;
- exact watch inventory construction;
- LSP content symbols/links/diagnostics;
- CLI/agent/project inspection.

Delete consumer-local rescans and fallback classifiers as each consumer is
migrated. A consumer cannot temporarily accept Source roots while the canonical
inventory rejects them.

## 7. Compatibility deletion checklist

The final tree has no:

- Source root variant;
- Source target variant;
- Source family alias;
- Source spelling compatibility reader;
- dual old/new family decoder;
- provisional Source tag in topology, bundle, cache, ProjectIndex, LSP, or wire;
- function-name or return-type migration heuristic;
- old source-content entity;
- legacy root path arrays;
- directory inference;
- last-known-good candidate accepted as the new result;
- source gate, CSS path, or Takumi path.

## 8. Proof of deletion

Required proof is behavioral and typed:

1. parser rejects removed source/source-content syntax through the normal
   grammar/recovery path;
2. compiler cannot produce an executable typed Source/content node;
3. final accepted symbol/entity inventories contain no Source/content
   declaration category;
4. runtime/AWBC/bundle cannot encode a Source plan/root;
5. LSP publishes no Source/content compatibility symbol;
6. tests construct representative removed syntax and assert the above behavior.

A repository substring scan may be recorded as manual review evidence, but it
is not an automated source gate and is not the definition of completion.
