# Authored resource storage implementation — 2026-07-10

## Status

The first local-filesystem slice is implemented. Authored binary assets and
structured content now use visible typed roots; `.arcweft/` is reserved for
local mutable state and temporary bundle-workspace materialization.

The stable design contract is
[`docs/05-build-and-security/authored-resource-storage.md`](../05-build-and-security/authored-resource-storage.md).

## Implemented contract

- `arcw.toml` accepts optional `[resources]` `asset-dir` and `content-dir`
  fields, defaulting to project-root `assets/` and `content/`.
- Package manifests and launch-only profile manifests share the same parser and
  path validation for resource roots.
- Resource roots must be non-empty, normalized project-relative paths. Absolute
  paths, `.`, `..`, and portable case-insensitive overlap are rejected.
- Direct source commands use source-adjacent `assets/`, `content/`, and
  `.arcweft/`. Project/profile commands use the selected manifest directory.
- Bundle collection and watch mode use the same resolved authored roots. The
  old `.arcweft/asset` / `.arcweft/content` lookup and parent fallback were
  removed.
- Native file adapters mount the authored asset root read-only. `save`, `temp`,
  and `export` writes remain under the selected `.arcweft/` state root.
- Bundle workspaces still materialize virtual files under temporary
  `.arcweft/<space>/` paths. This is runtime-owned state rather than an authored
  source layout.

## Repository migration

- Shared sample images moved from `samples/.arcweft/asset/bg/` to
  `samples/assets/bg/`.
- The modern feedback sample's required background is tracked under its
  project-root `assets/` directory.
- Reactive View sidecars are tracked under project-root `content/`; its sample
  has an explicit manifest/profile route. The previously ignored files also
  required the current View ownership name `view` and canonical `panel` kind
  when they became part of normal bundle validation. The later executable View
  contract replaced the provisional single `root_view` field with a typed
  per-definition inventory; no compatibility alias was retained.
- Responsive placement and Web demo generated assets now have one canonical
  visible path. The Web demo uses a dedicated manifest-selected
  `bundle-assets/` root, so unrelated browser fonts and ignored local files in
  `web/assets/` cannot enter AWFB by directory traversal. The Web generator no
  longer writes a duplicate hidden tree.
- Zundamon source-derived PNGs moved to visible `assets/zundamon/` but remain
  intentionally ignored because the checked-in preparation tool derives them
  from separately obtained source artwork.
- `.gitignore` now ignores `.arcweft/` at every depth without ignoring authored
  `assets/` or `content/` roots globally.
- The structural scanner excludes `.arcweft/`, `.arcweft-local/`, and `.jj/`
  so audit reports cannot capture local asset provenance, mutable state, or VCS
  internals.

Historical dated implementation notes and structural-audit snapshots retain
their original paths as chronological evidence. Current design, tooling, and
regeneration documentation use the new contract.

## Explicit non-goals

- No engine-owned external content-addressed asset resolver or asset lockfile
  format is introduced in this slice.
- No network fetch, authentication, license acquisition, or shared cache
  publication is added to the runtime or Sans I/O crates.
- Git LFS is not made a repository baseline. A project may use it only after
  its Git/Jujutsu and CI hydration path is verified independently.
- Existing production-file hotspot decomposition is not bundled into this
  storage migration unless the structural audit finds a new error-level
  regression.

## Validation

The audited checkout used Jujutsu change id `ztmmzrpx` with parent
`142acf58`. The canonical audit command was:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/authored-resource-storage-2026-07-10
```

The final dry-run scanned 2,520 files, including 1,180 Rust files and 588,316
Rust physical LOC. It reported one pre-existing error and 152 warnings. The sole error is
`crates/arcweft-cli/src/app/bundle_view.rs` at 2,590 LOC; that file is not
changed by this slice. Reports are under
[`structure-audits/authored-resource-storage-2026-07-10/`](structure-audits/authored-resource-storage-2026-07-10/).

Changed boundary hotspots measured from the current checkout:

| Path | Owning crate | Bytes | Physical LOC | Classification | Embedded tests | Major responsibility |
| --- | --- | ---: | ---: | --- | --- | --- |
| `crates/arcweft-project/src/manifest.rs` | `arcweft-project` | 13,235 | 477 | production | yes | typed project/build/resource manifest data and validation |
| `crates/arcweft-project-loader/src/project.rs` | `arcweft-project-loader` | 13,087 | 393 | production | yes | filesystem-backed manifest and source loading |
| `crates/arcweft-runtime-host/src/native_task.rs` | `arcweft-runtime-host` | 30,576 | 890 | production | yes | native host adapters, scheduling, and virtual-file mounts |
| `crates/arcweft-runtime-host/src/bundle_runner.rs` | `arcweft-runtime-host` | 40,103 | 1,166 | production | yes | bundle validation, materialization, and execution |
| `crates/arcweft-cli/src/app/project.rs` | `arcweft-cli` | 28,115 | 832 | production | yes | source/profile selection and project-facing compiler setup |
| `crates/arcweft-cli/src/app/bundle.rs` | `arcweft-cli` | 81,634 | 2,275 | production | no | bundle assembly plus image/View resource collection |
| `crates/arcweft-cli/src/app/runtime/run.rs` | `arcweft-cli` | 49,616 | 1,350 | production | yes | run/watch orchestration |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | `arcweft-cli` | 47,913 | 1,353 | production | yes | player-backed Agent observation orchestration |

`bundle.rs`, `run.rs`, and `player_observation.rs` remain warning-level existing
hotspots. `bundle.rs` stayed at 2,275 physical LOC, `run.rs` decreased from
1,352 to 1,350, and `player_observation.rs` grew by six lines only for typed
root plumbing. No changed file crossed an error threshold or grew by 300 LOC,
so decomposition is not coupled to this storage migration.

Relevant workspace dependency fan-in/fan-out from the same audit:

| Crate | Fan-in | Fan-out |
| --- | ---: | ---: |
| `arcweft-project` | 3 | 5 |
| `arcweft-project-loader` | 2 | 14 |
| `arcweft-runtime-host` | 5 | 18 |
| `arcweft-cli` | 0 | 64 |

Successful validation included:

- `cargo fmt --all --check`
- `git diff --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features` (existing warnings
  remain outside this slice; focused follow-up clippy confirmed no new warning)
- `cargo test -p arcweft-project`
- `cargo test -p arcweft-project-loader --lib` (82 passed)
- `cargo test -p arcweft-runtime-host --lib` (45 passed)
- `cargo test -p arcweft-cli --lib --bins --quiet` (177 passed)
- all 36 `app::bundle::tests`, including explicit custom asset and project-state
  roots with no legacy source-local fallback
- all three authored-resource watch tests
- focused direct native-file execution, image-animation AWFB validation,
  responsive placement, reactive sidecar, native text-input, and Web image
  parity tests
- `cargo test -p arcweft-cli --test arcw_fixtures_check_run --quiet` (5 passed)
- Web and modern-feedback checked-in AWFB regeneration through manifest/profile
  routes
- Web generator plus a temporary Web AWFB build with exactly four virtual files
  and four image assets
- reactive manifest/profile AWFB inspection with `ViewProgram`, `ViewStyle`, and
  `ViewText` sections
- direct native-capture behavior evidence in
  `agent_observe_native::native_checked_in_visual_golden_artifacts_are_well_formed`,
  `agent_observe_native::agent_observe_native_renderer_reports_vertical_lr_ruby_text_combine_geometry`,
  and
  `agent_observe_native::agent_observe_native_renderer_reports_vertical_goal_clear_smoke_geometry`
- ignore-policy checks: `.arcweft/` and generated Zundamon PNGs are ignored;
  required modern/reactive/Web inputs are not

## Known validation gaps outside this slice

`just test-workspace` does not currently have a green parent baseline. The
workspace leg failed in unchanged crates at two `arcweft-core` status
expectations; exact reruns reproduced both failures. Re-running without
`arcweft-core` reached an existing `arcweft-lang-sema` rest-parameter diagnostic
expectation failure. The later standard legs were run separately: CLI lib/bins
and the Arcweft fixture harness passed, while these existing gates still fail:

- `regression_harness`: historical docs already contain host absolute paths and
  the literal policy terms `deprecated` / `compatibility shim`;
- persistent-cache CLI goldens: checked-in fixtures still use removed
  `start(@flow.opening)` syntax;
- the focused product-AWBC native-file `run-bundle` test: product host-call
  reconstruction leaves the native file request pending. Direct bytecode-VM
  native file I/O passes, and this slice's bundle assertions verify the asset
  and save files are encoded at the correct virtual paths.

The Zundamon preparation script also prints its usage and the correct new
default output for `--help` but exits with status 1 under its existing argument
parser. None of these failures is caused by or changed to green by the authored
resource storage contract.
