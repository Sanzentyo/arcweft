# Owner and dependency graph

In every diagram below, `A -> B` means **crate A depends on crate B**.

## Selected acyclic graph

```text
arcweft-manifest-model -> arcweft-id and neutral primitives

arcweft-view           -> lower source/resource/id layers
arcweft-dialogue       -> arcweft-view, arcweft-source, arcweft-resource-model
arcweft-launch         -> arcweft-manifest-model, arcweft-view,
                          arcweft-dialogue, arcweft-source
arcweft-project-loader -> arcweft-launch and lower project/source layers

arcweft-runtime-plan   -> arcweft-dialogue, arcweft-view, arcweft-source
arcweft-compiler       -> arcweft-launch, arcweft-dialogue, arcweft-view,
                          arcweft-resource-model, arcweft-runtime-plan,
                          compiler/bundle product layers
arcweft-runtime-driver -> runtime-plan/bundle/dialogue/view runtime layers
```

The compiler may import runtime-plan because it lowers into that product.
Runtime-plan therefore cannot import compiler-owned `CheckedDialogueProfile`.
The reusable `DialogueProfileRevision` lives in `arcweft-dialogue`, which both
sides can import without a cycle. `CheckedDialogueProfile`, source labels,
admission, and retention of the accepted product remain compiler-owned.

## Owner/capability matrix

| Crate | May own | Must not own |
|---|---|---|
| `arcweft-manifest-model` | neutral IDs, schema version, hashes, metadata-compatible primitives | `DialogueProfileSpec`, View/Style presentation types, decoder, source map, admission |
| `arcweft-view` | `ViewId`, `ViewStyleSheetId`, View roles/capability behavior | launch profile, manifest decoder, checked profile |
| `arcweft-dialogue` | presentation profile, strict inline policy, revision tuple, runtime dialogue value/state | schema-1 decoder, compiler admission, project loading |
| `arcweft-launch` | sole decoder, strict authored specs, `SourceBackedManifest`, generic source map, pure resolution | compiler product validation, runtime-driver catalog |
| project-loader | file/project topology construction and source revision freeze | View compilation, checked dialogue profile, runtime-driver dependency |
| `arcweft-compiler` | validated compiled View/Style product, checked profile, source-bound admission diagnostics | alternate decoder/source map, runtime publication state |
| `arcweft-runtime-plan` | checked display plan carrying lower revision/value facts | dependency on compiler, re-admission, manifest parse |
| runtime-driver/backends | execution and rendering of the checked plan | decoder, second catalog, ID-spelling validation |

## Forbidden edges

The following edges are contract failures:

```text
arcweft-manifest-model -> arcweft-view
arcweft-manifest-model -> arcweft-dialogue
arcweft-manifest-model -> arcweft-launch
arcweft-project-loader -> arcweft-runtime-driver
arcweft-project-loader -> compiler-owned View catalog merely for profile checking
arcweft-runtime-plan -> arcweft-compiler
arcweft-dialogue -> arcweft-launch
```

## Product ownership, not catalog duplication

`ValidatedViewProduct` is the single immutable compiler/bundle-validated product
used for admission and retained by `CheckedDialogueProfile`/`CompiledProject`.
Runtime catalog construction is downstream from that accepted product. It is
not an input to project-loader and is not duplicated into a lower “profile
catalog.”

## Structured dependency test

Acceptance must use `cargo metadata --format-version 1`, resolve package IDs,
and assert forbidden edges are absent. Grepping Cargo.toml is only a review aid
and is not the acceptance proof.
