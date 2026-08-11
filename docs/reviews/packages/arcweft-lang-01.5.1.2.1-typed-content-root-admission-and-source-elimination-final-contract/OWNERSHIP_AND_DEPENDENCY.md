# Ownership and dependency contract

## 1. Final owner table

| Responsibility | Sole owner | I/O status | Required dependencies | Forbidden dependency or duplication |
| --- | --- | --- | --- | --- |
| Neutral manifest IDs, `ContentRootRef`, demand/visibility/profile policy | `arcweft-manifest-model` | Sans I/O | existing lower identity crates | source documents, filesystem, sema, LSP |
| Exact binary bytes and topology transcript | `arcweft-project::content` | Sans I/O | character IDs/paths, manifest model, resource registry digest, project fingerprint | host paths, filesystem reads, LSP versions |
| Final root family/target/facts | `arcweft-project::content` | Sans I/O | manifest model, source spans, resource identity, Character/Activity IDs | sema symbol tables, loader paths |
| Strict manifest decode and content source ranges | `arcweft-launch` | Sans I/O after bytes supplied | manifest model, source | second decoder or content-only source map |
| Character package validation | `arcweft-character::package` | Sans I/O | manifest model and PNG decoder | directory discovery or loader callbacks |
| Project symbol/family resolution and typed reference collection | `arcweft-lang-sema` | Sans I/O | HIR/syntax/source/project/resource/character catalogs | filesystem and manifest reparsing |
| Host path containment, disk/overlay acquisition, transaction coordinator | `arcweft-project-loader` | I/O adapter | launch, project, sema, character, manifest model | data-format ownership or source-string reconstruction |
| Final manifest-owned content index | `arcweft-lang-sema::ProjectSemanticIndex` | Sans I/O | `Arc<AcceptedProjectContent>` | source-HIR `ContentRoot` graph copy |
| Bundle projection | `arcweft-bundle` | Sans I/O over supplied accepted carrier | bundle model, CharacterPackage projection | scanning package directories or re-decoding manifests |
| Watch path adaptation | project-loader/host watcher adapter | host I/O | accepted watch inventory | recursive content inference |
| LSP generation/publication | `arcweft-lsp` | host/editor adapter | one `Arc<AcceptedProfileProject>` | separate topology/index publication or text-only candidate reconstruction |

## 2. Required graph

```text
arcweft-manifest-model ─┐
arcweft-source ─────────┼──> arcweft-project::content
arcweft-character ──────┤
arcweft-resource-model ─┘

syntax/HIR + registered semantic catalogs
                │
                v
       arcweft-lang-sema
                │
                v
      arcweft-project-loader  <── filesystem/editor overlay adapters
                │
                ├──> compiler / bundle
                └──> LSP accepted environment
```

`arcweft-project` remains below sema and loader. `arcweft-lang-sema` adds or
retains a direct dependency on `arcweft-project`; `arcweft-project-loader`
already sits high enough to depend on both and owns the cross-product
transaction. No dependency from project, manifest-model, character, or source
to sema/loader/LSP is introduced.

## 3. Sans-I/O invariants

- Core/data/model crates accept exact bytes, typed IDs, and source documents
  supplied by callers; they never open a path.
- Host `PathBuf`, URI, file version, mtime, and watch handles remain in loader,
  LSP, or host adapter types.
- `NormalizedProjectPath` is logical package identity and may exist in Sans-I/O
  products.
- `SourceSpan` is source provenance, not a filesystem locator.
- Bundle receives accepted bytes and logical paths; it does not acquire them.

## 4. Why the coordinator belongs in project-loader

The coordinator needs the accepted manifest/topology, filesystem containment,
text and binary overlays, complete Character packages, the sema world, and the
final project index. `arcweft-project-loader` can depend on all of those without
a cycle. Moving it into `arcweft-project` would pull in sema and I/O; moving it
into sema would pull in filesystem ownership; moving it into LSP would make the
editor an authority. Therefore `arcweft-project-loader::admission` is the sole
coordinator.
