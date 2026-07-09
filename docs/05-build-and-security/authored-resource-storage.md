# Authored resource and local state storage

Arcweft separates authored project inputs from tool-owned mutable state. The
default project layout is:

```text
project/
  arcw.toml
  src/
  assets/
  content/
  .arcweft/
```

`assets/` contains authored binary inputs such as images. `content/` contains
authored structured inputs such as View program, style, and text sidecars.
Both are visible project inputs and are versioned by default. `.arcweft/` is a
local state root and must be ignored by version control.

This distinction is about ownership, not file format:

| Class | Default location | Version-control policy | Examples |
| --- | --- | --- | --- |
| Authored asset | `assets/` | tracked | PNG, JPEG, GIF, WebP |
| Authored structured content | `content/` | tracked | View JSON sidecars |
| Reproducible generated input | visible project-specific directory | tracked when a clean checkout must run; otherwise regenerated | generated sample images |
| Mutable local/runtime state | `.arcweft/` | ignored | `save/`, `temp/`, `export/`, caches |
| Build output | `target/` or configured target root | ignored | AWFB and build metadata |

Do not place a required authored input under `.arcweft/`. A clean checkout must
not depend on an ignored file unless the project explicitly documents and tests
a preparation step.

## Manifest contract

`arcw.toml` may override the two authored roots:

```toml
[resources]
asset-dir = "assets"
content-dir = "content"
```

The paths are resolved relative to the directory containing `arcw.toml`. They
must be non-empty, normalized project-relative paths: absolute paths, `.`,
`..`, and overlapping asset/content roots are rejected. The defaults apply to
both package manifests and launch-only manifests.

Direct single-source commands without a manifest use `assets/`, `content/`,
and `.arcweft/` next to the selected `.arcw` file. Manifest/profile commands
use the manifest directory even when the selected source is under `src/`.
There is no fallback search under a source-local or project-local
`.arcweft/asset` or `.arcweft/content`; removed layouts fail visibly instead of
silently loading stale copies.

An asset path is relative to the asset root. For example,
`assets/bg/room.png` is packaged as virtual asset path `bg/room.png` and derives
the stable id `asset.bg.room`. Structured files under `content/` are compiler
and bundler inputs rather than mutable runtime files.

`asset-dir` is an inclusion boundary, not a general-purpose static-file folder.
The bundler walks its regular files and does not consult Git ignore rules. A
local ignored file under that root can therefore enter a bundle. Projects that
also contain browser/server static files, fonts, source artwork, or private
local material must point `asset-dir` at a dedicated subtree rather than their
broad static-resource directory. Referenced-only tree shaking may be added as a
separate packaging policy, but it is not the current safety boundary.

The native file boundary mounts the authored asset root read-only. Runtime
writes for the `save`, `temp`, and `export` virtual spaces go to matching
subdirectories under `.arcweft/`. A bundle runner may materialize virtual
files under a temporary workspace's `.arcweft/<space>/`; that is an internal
runtime layout, not an authoring convention.

## Version-control strategies

The repository-level default is ordinary Git/Jujutsu tracking for `assets/`
and `content/`. This gives reviewable changes and makes checkout, CI, bisect,
and deterministic bundle construction work without an extra synchronization
step. Textual content should normally remain in this mode.

Large or restricted binaries may use a project-specific alternative while
keeping the same Arcweft filesystem contract:

1. Ignore the selected asset directory or file patterns in that project.
2. Keep a reviewable inventory containing stable logical paths, byte lengths,
   cryptographic digests, origin/license metadata, and the retrieval policy.
3. Provide one deterministic preparation command that populates `asset-dir`.
4. Verify digests before publishing the files into that directory.
5. Make CI either run the preparation step or fail early with a precise missing
   asset diagnostic.

This supports an external content-addressed store, licensed-art download, or a
local generation pipeline without treating `.arcweft/` as an asset warehouse.
The current Arcweft CLI does not fetch external authored resources or define an
asset lockfile format; retrieval, authentication, and cache publication remain
project/build-adapter responsibilities. An engine-owned content-addressed
resolver and lock contract require a separate design before implementation.

Git LFS is not the baseline policy. It may be adopted by an individual project
only after every Git and Jujutsu workflow used by developers and CI has been
verified to hydrate, diff, and publish the expected objects. Pointer files
without reliable object hydration do not satisfy the clean-checkout contract.

## Repository and sample policy

- Required sample inputs are tracked in visible `assets/` or `content/` roots.
- A sample that intentionally depends on external or licensed material keeps
  those files ignored and documents a preparation command and provenance.
- Generated fixtures have one canonical path; duplicated generated copies are
  not retained merely to support an old directory convention.
- `.gitignore` ignores `.arcweft/` at every depth but does not globally ignore
  `assets/` or `content/`.
- Packaging, watch mode, tests, and generators resolve the same typed roots
  instead of reconstructing directory names independently.
- Git tracking and bundle inclusion are independent: `.gitignore` never acts as
  a packaging exclusion list.

## Migration from the hidden authored layout

Move `.arcweft/asset/**` to `assets/**` and `.arcweft/content/**` to
`content/**`, preserving paths below each root. Update generators, fixtures,
notices, and tests at the same time. Add `[resources]` only when a project needs
non-default names. Remove the old directories rather than retaining a fallback
or duplicate copy, then validate from a clean checkout or through the project's
documented preparation route.
