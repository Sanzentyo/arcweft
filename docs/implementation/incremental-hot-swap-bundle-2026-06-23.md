# Incremental hot-swap bundle implementation note (2026-06-23)

Source package:
`D:/sanze/Downloads/arcweft-incremental-hot-swap-bundle-2026-06-23.zip`

Baseline named by the package: `8117fecf`.

## Implemented in this cut

- Removed language-level import execution policy from current syntax:
  - `UseMode` and `UseDependencyMode` were removed from
    `arcweft-lang-syntax`.
  - `lazy use` and `eager use` now produce parser diagnostics instead of
    accepted AST imports.
  - `UseItem` now owns a typed `UseTree` with a parsed module prefix and exact
    module-prefix flag. Project loading no longer reparses raw use-tree strings
    to discover module dependencies.
  - Project graph dependencies are normal name-introduction edges only.
  - Agent REPL demand does not change this: REPL `:warm`/`:load` behavior,
    compiler interface/body demand, and tool schema readiness are not `use`
    syntax responsibilities.
- Added Sans I/O incremental model types to `arcweft-project`:
  - `artifact::{ArtifactKind, ArtifactKey, ArtifactKeyInput}`
  - `fingerprint::{BuildDigest, NamedDigest, ProjectFingerprint}`
  - `incremental::{CompileDemand, QueryKind, BuildSnapshot, ...}`
- Added filesystem cache adapter types to `arcweft-project-loader`:
  - immutable object storage under `objects/blake3/...`
  - immutable artifact-key records under `records/<query>/...`
  - create-new package lock guard under `locks/`
- Added compiler-side module-object/link/partition APIs:
  - `object::ModuleObject`
  - `link::{LinkRequest, LinkedProgram, link_project}`
  - `reachability::{LinkGraph, AvailabilityDomain, ...}`
  - `content_partition::partition_content`
  - `incremental::snapshot_compiled_project`
- Added v1 `content` source support without accepting `asset set`:
  - `content name { roots = [@flow.name] }` is parsed as a typed entity
    declaration body.
  - `asset set` is diagnosed at top-level parser entry and is not represented
    as an AST/HIR/sema declaration kind.
  - `@asset_set.*` references and `AssetSetRef<T>` annotations are rejected by
    semantic checks instead of being accepted as generic entity/type syntax.
  - `hot checkpoint` is diagnosed at top-level parser entry and is not
    represented as an AST/HIR/sema declaration kind.
  - Hand-written `asset` and `content` declaration headers prefer compact ids
    such as `asset bg_room { ... }` and `content chapter_two { ... }`; the
    declaration keyword supplies the default family, so fully qualified
    declaration ids such as `asset @asset.bg_room { ... }` are
    generated/fully elaborated surfaces and lint to the compact form.
  - Family-relative references such as `@asset:.bg.room` are the recommended
    authored spelling for asset refs, not only a typed-argument shorthand. The
    `asset` anchor is still present in the authored reference; the omitted
    default is the repeated family segment in the id path. Fully qualified
    `@asset.bg.room` remains valid for generated surfaces, manifest/tooling
    output, and external interfaces that need the stored public id verbatim.
    Runtime-plan expression labels preserve the normalized `asset.bg.room` id
    for these references instead of dropping the family. This does not admit
    bare `@.suffix` as a general reference; `@.suffix` remains an ID-context
    form.
  - CLI, Agent, and runtime-driver image-reference extraction normalize
    authored `@asset:.suffix` call/declaration arguments to the same public id
    as fully qualified `@asset.suffix`, so examples can use the recommended
    family-relative spelling without losing static image validation, Agent
    observation, or native display resolution.
  - `content.prefetch`, `content.ensure`, and `content.release` are typed
    runtime availability builtins.
  - Content root references are indexed as `content_root` project graph edges.
  - Dynamic-reference retention is represented internally as linker
    `FiniteRefSet` values that expand to `DynamicSet` reachability edges; no
    source-level `asset set` or `AssetSetRef<T>` syntax is accepted.
  - `arcw.toml` launch profiles now parse typed build/source/debug policy,
    hot-reload policy, and profile-level content residency/placement/
    compression policy such as
    `[profiles.release.content."content.chapter_two"]`.
  - Release project verification now rejects dynamic `goto` targets so release
    tree shaking does not silently depend on unbounded runtime flow lookup.
- Added the first Sans I/O AWFB v1 product container module in
  `arcweft-bundle`:
  - fixed magic/version/header and canonical manifest/index ranges;
  - typed bundle kind, section kind, residency, placement, compression, and
    section id/descriptors;
  - owner APIs for content residency, placement, and compression policy
    values, including default, parse, display, and stable string forms;
  - BLAKE3 stored/content digest verification for embedded raw sections;
  - stored and decoded size budgets for embedded payloads and external section
    descriptors;
  - zstd embedded-section compression/decompression with decoded output limits;
  - optional signature-block bounds/overlap validation and `BundleView`
    exposure as opaque bytes;
  - Program required-section validation and ContentPack executable-section
    rejection;
  - read-budget, bounds, overlap, duplicate section id, unknown required, and
    unknown optional-section policies.
  - External section descriptors now round-trip size/digest metadata without
    embedded payload bytes, so content roots can represent external payload
    identity before a host fetches bytes.
  - `BundleView::decoded_section_with_external_payloads` now resolves an
    external section from caller-supplied bytes without rewriting or
    re-encoding the AWFB container. It verifies the supplied payload size and
    content digest against the section descriptor, reports missing or duplicate
    payloads for the requested section, and ignores unrelated payload entries
    so adapters can pass a shared fetched-payload set.
- Wired AWFB v1 into bundle product encode/decode:
  - `BundleFormat::Awfb` is the `.awfb` product format and is selected by
    `.awfb` extension and AWFB magic/version probing.
  - `arcw bundle` now defaults to AWFB output while explicit JSON/TOML/YAML/
    MessagePack/CBOR/Avro remain inspection/interoperability formats.
  - `BundleFormat` owns parsing/display behavior for CLI format selection.
    The previous CLI-local bundle input/output format enums were removed;
    product `run-bundle` now accepts `.awfb` input only and decodes it through
    the fixed AWFB product path rather than a user-selected legacy format.
    `arcweft-bundle` now exposes separate product and inspection path decoders:
    product readers require explicit `.awfb` and AWFB bytes, while inspection
    readers require explicit legacy/export extensions such as `.awfb.json`
    instead of probing unknown paths.
  - Bundle encode/decode validation now rejects duplicate adapter manifest ids,
    virtual file paths, image asset ids, and image object ids instead of
    silently sorting and deduplicating payloads. CLI adapter-manifest inference
    explicitly merges identical inferred manifests and reports an error if the
    same id has conflicting bodies.
  - `arcw run-bundle` auto-decodes AWFB product containers before materializing
    the temporary workspace and executing bytecode.
  - `arcw run-bundle --patch update.awfb` validates an AWFB patch artifact
    against the base bundle, materializes embedded add/replace/remove section
    operations into patched AWFB bytes, and executes the patched bundle. Patch
    materialization keeps the base manifest bytes in this cut; JSON reports
    include the patch path.
  - Product AWFB `ProgramBytecode` sections now use an `AWBC` binary envelope
    whose v1 payload carries a decoded-and-verified compact bytecode validation
    table plus the structured `BytecodeProgram` needed by the current VM.
    Runtime types, entrypoints, adapter requirements, content catalog, display
    catalog, and normalized source still use typed section-specific JSON
    payloads.
  - Product AWFB decode now has explicit external-section entry points
    (`from_awfb_slice_with_external_sections` and
    `from_product_path_slice_with_external_sections`) so release/cache adapters
    can supply already fetched section payloads while preserving the original
    AWFB content root.
- Added the first bytecode artifact verification gate:
  - `arcweft-core::bytecode` now carries `BYTECODE_ABI_VERSION` and stores an
    ABI version plus a runtime layout signature on `BytecodeProgram` with
    serde/default compatibility for v1 structured JSON bytecode.
  - `BytecodeProgram::verify` enforces ABI, runtime layout signature, bounded
    counts, required entrypoint flow, duplicate entry/flow ids, entry target
    existence, `Dialogue` line-task group bounds, static `Goto` targets, and
    static choice-option targets.
  - `arcweft-core::compact_bytecode` now defines a Sans I/O compact bytecode
    table model and verifier for raw opcodes, code-slot indices, jump targets,
    constant indices, content-unit indices, runtime type indices, duplicate
    code slots, ABI, and validation budgets.
  - AWFB product decode compares the `RuntimeTypes` section runtime layout with
    the `ProgramBytecode` section runtime layout before rebuilding an
    `ArcweftBundle`.
  - `arcweft-runtime-driver::BundleSession` and
    `arcweft-runtime-host` bundle runner verify bytecode before constructing
    runtime sessions/executors.
- Added the first patch/hot-swap model slice:
  - `arcweft-bundle::patch` can diff AWFB section descriptors into stable
    add/replace/remove operations keyed by section id and content digest.
  - Patch plans carry exact base and target content roots and validate the
    active base before apply.
  - Patch bundles now require a `PatchPlan` section at the container
    validation layer.
  - `BundlePatchArtifact` now encodes/decodes AWFB `BundleKind::Patch`
    artifacts with a manifest and embedded `PatchPlan` section, including
    runtime ABI range, conservative section-kind compatibility class, and
    content-root consistency checks.
  - Embedded add/replace operations now carry changed section payload bytes as
    `AssetBlob` carrier sections in patch AWFB files. The PatchPlan retains the
    logical section kind, and decode verifies carrier shape, logical descriptor
    fields, and content digests against the PatchPlan.
  - `arcweft-runtime-driver::swap` owns live-apply compatibility classes and a
    Sans I/O generation swap state machine for prepare, quiesce, commit, and
    retire phases.
  - `ProgramGeneration::from_bundle` now verifies structured bytecode and
    derives content, adapter, code-slot, and runtime-table fingerprints from a
    decoded game bundle for compatibility classification.
  - `arcweft-runtime-driver::BundleSession` now owns an active generation and
    exposes `hot_swap_bundle` for decoded game bundles. Content-only swaps
    update display/image-object content without rebuilding bytecode execution;
    code-compatible swaps rebuild the VM executor at a quiescent host boundary;
    structured code-generational swaps are reported as host-restart-required
    until old executable fibers can continue against their original code slot
    tables. `BundleSession` now pins the generation owned by the active VM
    fiber and by outstanding host task dispatches; embedding code can also pin
    the active generation explicitly, and old generations are retained until
    all of those handles are dropped.
  - `BundleSession` can also inspect decoded or encoded AWFB patch artifacts:
    patch bytes are decoded, the patch artifact is validated, the active base
    AWFB content root is checked against sessions created from AWFB bytes, and
    no-op/target-required readiness plus the manifest compatibility class are
    reported without rebuilding a target.
  - Embedded AWFB patch artifacts can now materialize target AWFB bytes by
    merging add/replace/remove section operations with the base container.
    `BundleSession::hot_swap_patch_bytes` decodes and validates patch bytes,
    reports manifest `code_generational`/`restart_required` compatibility
    before materializing a target, and otherwise materializes/applies the target
    through the same decoded-bundle compatibility path.
  - External section descriptor changes can now be applied without inlining
    payload bytes. The target AWFB preserves external size/digest metadata; byte
    fetching remains a release-manifest/host-adapter concern.
  - `arcw patch --base old.awfb --next new.awfb --output update.awfb` now
    generates offline AWFB patch artifacts and reports base/target roots,
    operation counts, changed-section payload count, and compatibility.
  - `arcw sign-bundle --input unsigned.awfb --output signed.awfb --signer-id
    release-key-main --signing-key-file key.hex` now appends a release
    signature envelope as an AWFB trailing signature block. The CLI adapter
    reads Ed25519 signing material, signs the typed release message, validates
    the result with the same Sans I/O release policy verifier, and reports the
    public key, content root, signing digest, and deterministic key epoch.
  - `arcw inspect game.awfb [--manifest] [--json]` now verifies AWFB container
    structure through `BundleView`, then reports bundle kind, content root,
    manifest byte length, skipped optional sections, and section descriptors.
  - `arcw cache stats|verify|explain|prune|fetch [--root
    target/arcweft/cache/v1] [--json]` now reports filesystem cache inventory,
    verifies object digests, record schema, record path keys, record-object
    digest/length references, explains entries matching one artifact-key or
    object digest, explains explicitly labeled logical items through
    `--logical`, dry-runs conservative pruning unless `--apply` is supplied, and
    prunes unreferenced objects plus empty cache shard/record/lock directories
    under known cleanup roots. It also fetches `.awfr` release bundles through
    deterministic `file:`, `http://`, TLS-validated `https://`, and
    `arcweft-cache:` mirrors.
  - `arcw build` now writes a profile-aware project AWFB next to the existing
    project metadata, lowered runtime-plan, and build snapshot artifacts. It
    also stores those build outputs as immutable cache objects plus typed
    `.awci` artifact records under the selected target directory's `cache/v1`,
    using a package lock while publishing records.
    `--patch-base` emits a patch AWFB against an explicit base bundle, and
    `arcw build --watch` continuously refreshes the build AWFB, compares build
    snapshots, reports module/query invalidation counts, and writes patch
    artifacts under the build target's `patches/` directory when watched inputs
    change.
  - `arcweft-player-native::NativePatchEndpoint` is now an in-process native
    player patch endpoint for AWFB-backed sessions. It owns the active base
    AWFB bytes, validates patch readiness against the active session root,
    applies content-only/code-compatible patches live through `BundleSession`,
    and automatically restarts its owned `BundleSession` from the materialized
    target AWFB for code-generational or restart-required patch artifacts. The
    endpoint also consumes the `.transport.json` sidecar emitted by
    `arcw run --watch`, validating the schema version, runner, roots,
    compatibility label, operation count, and apply/restart action against the
    referenced patch bundle before applying it. Patch bundle paths are resolved
    in the same forms the watch CLI writes: absolute paths, current-working-
    directory relative paths such as `target/arcweft/run/patches/...`, and
    sidecar-directory relative paths.
- Added the first external release manifest model:
  - `arcweft-bundle::release` defines `.awfr` JSON manifests for external AWFB
    files keyed by content root and whole-file digest.
  - Release bundle refs validate byte length, whole-file digest, bundle kind,
    non-empty mirrors, duplicate content roots, and supported mirror schemes.
  - Release manifests now produce deterministic Sans I/O fetch plans for a
    content root, ordered by mirror priority, with byte length and digest
    verification for adapter-fetched candidates.
  - Release manifests now carry typed fetch policy for retry count, candidate
    byte budget, cancellation timeout metadata, and network client policy. The
    network policy carries HTTPS-only transport, proxy profile id, auth profile
    id, client profile id, and user-agent without embedding secrets. The
    local/cache fetch adapter applies retry count to `file:` mirrors, skips
    oversized file candidates before reading them, applies
    retry/timeout/body-budget policy to plain `http://` and TLS-validated
    `https://` mirrors, records each attempt, enforces HTTPS-only policy, and
    refuses network mirrors that require unresolved proxy/auth/client profiles
    instead of silently bypassing the manifest policy.
  - Release manifests now carry typed signature policy. Fetch plans can require
    an AWFB signature block and optional minimum signature size, and the cache
    fetch adapter rejects otherwise valid candidates that do not satisfy that
    container-level policy. The same policy can now require a release signature
    envelope whose `signer_id` appears in the manifest's trusted signer list;
    the envelope is also bound to the fetched bundle content root, bundle kind,
    and canonical AWFB signing digest. When the policy lists trusted Ed25519
    public keys, the release verifier also checks the envelope signature payload
    against the typed signing message and a matching trusted key. Signature
    policy also carries an `allowed_algorithms` registry, defaults it to
    `ed25519-v1`, rejects empty/duplicate/unknown entries during manifest
    validation, and rejects envelopes whose algorithm is not supported and
    allowed by policy.
  - `arcw sign-bundle` owns the operational signing adapter boundary: it reads
    local Ed25519 signing material, builds the release signature envelope,
    appends it as the trailing AWFB signature block, and re-verifies the signed
    bytes against trusted-public-key policy before writing output. The bundle
    crate still owns only the Sans I/O container mutation and release envelope
    verification.
  - AWFB bytes can be parsed into a release bundle ref without performing
    filesystem, network, cache, clock, or signing work.
  - The project-loader release cache adapter now has bytes-returning and
    product-returning fetch APIs. `fetch_release_bundle_bytes_to_cache` returns
    verified AWFB bytes together with the cache report, and
    `fetch_release_product_bundle` feeds those bytes into
    `ArcweftBundle::from_awfb_slice_with_external_sections`, so callers can
    provide already fetched external section payloads while preserving the
    release/cache verification boundary.

## Deliberate constraints

- The current semantic/typecheck/runtime-plan pass is still linked-HIR scoped.
  The new snapshot bridge records conservative unit evidence rather than
  claiming module-aware semantic reuse.
- Cache records are immutable by artifact key in this cut. A separate
  logical-key replacement layer can be added later if the CLI needs a mutable
  "latest record" index.
- Product `.awfb` output now uses the AWFB v1 fixed-header container.
  `ProgramBytecode` uses an explicit binary envelope with a compact bytecode
  validation table plus the structured `BytecodeProgram` still required by the
  current VM. Runtime types, entrypoints, adapter requirements, content
  catalog, display catalog, and normalized source still use typed JSON
  payloads. The final executable compact opcode design that would remove the
  structured `BytecodeProgram` payload is explicitly out of scope for this goal
  and is requested in
  `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`.
  Compact binary codecs for the remaining product resource sections are also
  out of scope until the design request in
  `docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`
  is answered.
- The native player product dependency tree no longer pulls
  `arcweft-compiler` or `arcweft-runtime-plan` through `arcweft-render-native`
  normal dependencies. Native rendering now accepts runtime-ready
  `RuntimePureHelper` values for Arcweft text shader/effect/motion registration
  instead of depending on compiler-side pure-helper candidates. Adapter
  manifests now use language-free adapter type/signature/capability records for
  runtime host-call policy; sema environment injection is available only behind
  the `arcweft-adapter-context/sema` feature used by language tooling.
- Hot-swap generations derived from current structured bytecode use
  conservative code signatures because the compact bytecode/type-layout tables
  do not exist yet. Any changed existing structured flow or runtime table is
  classified as code-generational. State layout fingerprints are intentionally
  empty until runtime state layout tables are available. The portable
  `BundleSession` can apply content-only and code-compatible swaps, but it
  reports code-generational structured bytecode changes as restart-required
  because the current single VM executor still does not continue an old fiber
  and start new entries against different executable generations in parallel.
  `BundleSession` now pins generations for the active VM fiber, outstanding
  host task dispatches, and explicit embedding handles. Patch materialization
  carries embedded changed payload bytes and metadata-only external descriptor
  changes in this cut. Product-grade manifest mutation, signature
  preservation/regeneration, external payload fetching, and release-manifest
  publication for materialized targets are out of scope until the design
  request in
  `docs/reviews/requests/2026-06-24-patch-target-manifest-signature-design.md`
  is answered. `arcw run-bundle --patch` can run materialized patch targets for
  CLI inspection. `arcw run --watch` now has an
  initial polling implementation that builds a product AWFB, records declared
  source/profile metadata inputs, rebuilds on changed input metadata, and
  writes AWFB patch artifacts with compatibility reports. The project build
  watch path now writes build snapshot artifacts and reports snapshot-derived
  module/query invalidation counts after rebuilds. The watch path now also
  writes local dev patch transport envelopes that tell the player side whether
  to apply a patch or restart from the target bundle. The native player
  library now has an in-process patch endpoint that executes both the
  live-apply and restart paths and validates/applies those sidecar envelopes;
  `arcw run --watch --runner native` now keeps that endpoint alive in the watch
  loop and applies emitted patch bundles to it without invoking a fresh player
  binary. The native player binary can also apply one transport envelope at
  startup through `--patch-transport`.

## Remaining bundle requirements

- Content DSL and manifest surface:
  `content` is the only v1 source-level declaration added by this surface and
  is modeled as an entity declaration with typed roots. `asset set`,
  `@asset_set.*`, `AssetSetRef<T>`, and `hot checkpoint` are not v1 source
  syntax. Finite dynamic references remain linker-internal `FiniteRefSet`
  values expanded into reachability edges; manifest-authored fallback roots
  remain the explicit escape hatch for reflection-like external integrations.
- AWFB v1 container:
  The product `.awfb` path is now fixed-header AWFB, CLI/runtime auto-decodes
  it, stored and decoded size budgets are enforced before exposing sections,
  zstd embedded payloads decode with an explicit output limit, optional
  signature-block ranges are bounded and exposed as opaque bytes, content
  residency/placement/compression policy values parse through typed owner APIs,
  legacy JSON is isolated to explicit inspection/export extensions, and patch
  bundles carry manifest, conservative compatibility class, `PatchPlan`
  metadata, and embedded changed-section payloads through `AssetBlob` carrier
  sections. Compact binary codecs for graph/entity/resource sections are not
  treated as remaining implementation work for this goal because section
  schemas, decoder budgets, cross-section validation, inspection behavior, and
  patch compatibility rules are not yet specified. The design request is
  recorded in
  `docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`.
- Bytecode artifact verifier:
  compact opcode/index validation now exists as a Sans I/O core model, and
  product `AWBC` sections now carry a decoded-and-verified compact validation
  table alongside the structured `BytecodeProgram`. The current structured
  bytecode artifact now has ABI, runtime layout signature, required
  entrypoint, budget, reference, AWFB `RuntimeTypes`/`ProgramBytecode` layout
  consistency, runtime/player construction gates, and non-JSON
  `ProgramBytecode` AWFB section decoding. Replacing that structured payload
  with a final executable compact constant/function/type/resource table is not
  treated as remaining implementation work for this goal because the opcode
  set, table schemas, host-call ABI, expression/value encoding, verifier, VM
  migration path, and compatibility fingerprints are not yet specified. The
  design request is recorded in
  `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`;
  until that response arrives, this goal keeps only the compact validation
  sidecar plus the structured executable bytecode payload.
- Patch and hot-swap runtime:
  patch section operations, base/target content roots, compatibility
  classifier, generation pinning, and quiesce/commit/retire state machine now
  exist as Sans I/O models. Patch AWFB artifacts now carry embedded `PatchPlan`
  payloads, `AssetBlob` carrier sections for changed embedded payload bytes,
  and a conservative manifest compatibility class. `ProgramGeneration`
  can now be derived from verified decoded game bundles with conservative
  structured-bytecode fingerprints. `BundleSession` now applies decoded
  content-only swaps and code-compatible VM rebuilds at a quiescent boundary,
  validates AWFB patch artifacts/bytes against the active AWFB root,
  materializes embedded-section patch bundles, applies the resulting target
  bundle, and pins generations for the active VM fiber, outstanding host task
  dispatches, and explicit host/catalog handles.
  Manifest `code_generational`/`restart_required` patch artifacts are now
  reported before target materialization. `arcweft-player-native` now exposes a
  `NativePatchEndpoint` that keeps the active AWFB base bytes beside an owned
  `BundleSession`, applies live-compatible patch bundles in process, and
  automatically restarts that session from the materialized target AWFB for
  code-generational/restart-required patches. It also reads and validates
  `arcw run --watch` transport sidecars, `arcw run --watch --runner native`
  keeps an in-process endpoint alive and applies emitted patch bundles to it,
  and the native player binary can apply one sidecar before running a bundle
  through `--patch-transport`. Remaining work is true code-generational
  execution where old fibers/tasks continue
  against old executable tables while new entries start on the new generation.
  The true code-generational execution design gap is tracked as
  `docs/reviews/requests/2026-06-24-code-generational-hot-swap-design.md`.
  That design request is explicitly out of scope for this incremental
  hot-swap bundle goal until its review response is supplied.
- CLI watch/dev transport:
  `arcw run --watch` now performs source/profile input polling, debounced by
  the configured poll interval, writes an initial product AWFB for native/web
  runners, and emits AWFB patch artifacts from base/next bundle roots on
  changed inputs. The same watched input inventory is reused by
  `arcw build --watch`, which writes profile-aware project AWFB, metadata,
  plan, build snapshot, and patch AWFB artifacts without launching a player.
  The watch input map now
  includes project source modules for profile-discovered manifests plus
  recursive `.arcweft/asset` and `.arcweft/content` files rooted next to the
  selected source, so added, removed, and modified payload files are observed by
  the polling loop. `arcw run --watch` additionally writes a `.transport.json`
  sidecar envelope recording the runner, target bundle, patch bundle, content
  roots, compatibility, operation count, and local dev action (`apply_patch` or
  `restart_player`). The native player library endpoint can execute that
  apply/restart decision in process, `arcw run --watch --runner native` now
  keeps such an endpoint alive during the polling loop, and the native player
  binary can consume one sidecar through `--patch-transport`. Build snapshots
  now expose query-level invalidation decisions, `arcw build` persists the
  emitted metadata/plan/snapshot/AWFB through the filesystem cache adapter, and
  repeated identical project builds reuse the cached verified Program AWFB
  artifact instead of re-encoding the bundle. `arcw build --watch` reports
  snapshot invalidation decisions after rebuilds and keeps an in-memory
  compile-unit cache for the running watch process, so unchanged parse/lint/HIR
  lowering units are reported as cache hits during watch rebuilds. Cross-
  invocation persistent compiler query reuse is not treated as remaining
  implementation work for this goal because the stable serialized unit format,
  query execution policy, linked-HIR semantic boundary, corrupt-record recovery
  behavior, and snapshot evidence model are not yet specified. The design
  request is recorded in
  `docs/reviews/requests/2026-06-24-persistent-compiler-query-cache-design.md`.
  Wiring patch updates into an already running windowed native event loop is
  also out of scope until its ownership/event-loop design is answered in
  `docs/reviews/requests/2026-06-24-windowed-native-live-patch-design.md`.
- Signed patch payload handling:
  patch AWFB bundles can now be decoded or applied through a
  `ReleaseSignaturePolicy`; policy-aware patch entry points reject unsigned or
  incorrectly signed patch bundles before materializing a target bundle. The
  signature envelope binds to the patch bundle content root, AWFB kind, and
  canonical signing digest, using the same Ed25519 v1 trusted-public-key
  verifier as external release bundles.
- Resolved product player dependency boundary:
  `arcweft-player-native` normal dependencies no longer include
  `arcweft-compiler`, `arcweft-runtime-plan`, `arcweft-lang-sema`,
  `arcweft-lang-hir`, or `arcweft-lang-syntax`. Runtime host-call ids, effect
  labels, and ABI signatures now live in a language-free adapter manifest model;
  sema-only environment injection remains an upper-layer adapter feature for
  CLI/LSP flows. `arcweft-player-web` normal dependencies also exclude the
  parser/HIR/sema/compiler/verifier stack. The native product player requires
  `.awfb` input by default and rejects `.awfb` paths containing legacy JSON
  bytes instead of falling back to the inspection JSON decoder. The native
  product player no longer exposes a `.arcw` source compilation route;
  source-to-product compilation is owned by `arcw build`, `arcw bundle`, and
  `arcw run` before the player boundary.
- External release manifest:
  `.awfr` mirror/digest mapping and deterministic fetch plans now exist as
  Sans I/O models. The filesystem cache now has inventory/verification
  commands and release-manifest-driven population through deterministic
  `file:`, plain `http://`, TLS-validated `https://`, and `arcweft-cache:`
  mirrors. Fetch plans now carry retry, candidate byte budget, and cancellation
  timeout policy metadata; the local/cache adapter applies retry and byte-budget
  policy to local and network mirrors and uses the cancellation timeout for HTTP
  socket operations and the HTTPS TLS adapter. Fetch plans also carry
  HTTPS-only, proxy profile, auth profile, client profile, and user-agent
  network policy; the default cache adapter enforces HTTPS-only policy and
  rejects network mirrors that require unresolved proxy/auth/client profiles.
  Cache
  records can now persist logical item labels, and `arcw cache explain
  --logical <item>` finds release bundle records such as
  `content-root:<digest>`. Cache prune now removes empty object-shard, record,
  and lock directories after safe file pruning. Fetch plans also carry
  signature-block policy, and cache fetch rejects unsigned candidates when the
  manifest requires an AWFB signature block. Signature policy now also supports
  JSON release signature envelopes, trusted signer id checks, and content
  root/kind/signing-digest envelope binding. Ed25519 v1 public-key policies now
  verify the signature payload in the Sans I/O release verifier. Release
  signature envelopes now carry a deterministic `key_epoch`; trusted public keys
  carry epoch validity windows and a `revoked` flag, so rotation and revocation
  are enforced without filesystem, network, or wall-clock access. `arcw
  sign-bundle` now provides the local release signing workflow that appends the
  signature block and reuses the same verifier before writing signed bytes.

## Design requests excluded from this goal

The following remaining areas are intentionally not implemented in this goal
because the current bundle/reference material does not specify the ownership
model, binary schema, verifier, migration path, or runtime behavior tightly
enough for safe implementation:

- `docs/reviews/requests/2026-06-24-code-generational-hot-swap-design.md`
- `docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`
- `docs/reviews/requests/2026-06-24-persistent-compiler-query-cache-design.md`
- `docs/reviews/requests/2026-06-24-windowed-native-live-patch-design.md`
- `docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`
- `docs/reviews/requests/2026-06-24-patch-target-manifest-signature-design.md`

## Validation so far

Focused checks run during this cut:

```bash
cargo test -p arcweft-lang-syntax --test parser_p0 removed_import_execution_modes_are_parse_diagnostics --quiet
cargo test -p arcweft-lang-syntax --test parser_p0 use_tree_exposes_typed_module_prefixes --quiet
cargo test -p arcweft-lang-syntax --test parser_p0 asset_set_is_not_v1_source_syntax --quiet
cargo test -p arcweft-lang-syntax --test parser_p0 hot_checkpoint_is_not_v1_source_syntax --quiet
cargo test -p arcweft-lang-syntax --test parser_p0 content_declaration_parses_as_typed_entity_body --quiet
cargo test -p arcweft-lang-syntax explicit_entity_decl_id_prefers_compact_authoring_form --quiet
cargo test -p arcweft-lang-sema tests::declarations::content_declaration_is_a_typed_entity --quiet
cargo test -p arcweft-lang-sema tests::typecheck::typechecks_content_availability_builtins --quiet
cargo test -p arcweft-lang-sema typecheck_reports_wrong_choice_target_kind --quiet
cargo test -p arcweft-lang-sema rejects_removed_asset_set_ref_surface --quiet
cargo test -p arcweft-lang-sema typechecks_family_relative_asset_references_in_asset_expected_calls --quiet
cargo test -p arcweft-lang-sema spec_should_pass_check_fixtures_pass_parser_hir_sema_after_refactor --quiet
cargo test -p arcweft-lang-sema project_index_records_content_root_relations --quiet
cargo test -p arcweft-lang-sema await_with_keeps_wait_view_branches --quiet
cargo test -p arcweft-lang-sema typechecks_await_wait_view_branches --quiet
cargo test -p arcweft-lang-sema normalizes_parent_module_root_alias --quiet
cargo test -p arcweft-project --quiet
cargo test -p arcweft-project build_snapshot_reports_module_and_query_invalidations --quiet
cargo test -p arcweft-launch --quiet
cargo test -p arcweft-project-loader --quiet
cargo test -p arcweft-project-loader cache::inspect --quiet
cargo test -p arcweft-project-loader prune_cache --quiet
cargo test -p arcweft-project-loader explain_cache_finds_record_by_logical_item --quiet
cargo test -p arcweft-project-loader cache::release --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_bytes_returns_verified_cached_bytes --quiet
cargo test -p arcweft-project-loader fetch_release_product_bundle_decodes_cached_awfb_product --quiet
cargo test -p arcweft-runtime-plan pure_function_candidate --quiet
cargo test -p arcweft-runtime-plan runtime_plan_normalizes_family_relative_asset_call_args --quiet
cargo test -p arcweft-compiler --lib --quiet
cargo test -p arcweft-compiler finite_dynamic_sets_retain_all_members_in_current_domain --quiet
cargo test -p arcweft-bundle awfb_v1 --quiet
cargo test -p arcweft-bundle awfb_v1_rejects_embedded_section_exceeding_decoded_budget --quiet
cargo test -p arcweft-bundle awfb_v1_rejects_external_descriptor_exceeding_decoded_budget --quiet
cargo test -p arcweft-bundle awfb_v1_exposes_bounded_signature_block --quiet
cargo test -p arcweft-bundle awfb_v1_rejects_signature_overlap_with_header_ranges --quiet
cargo test -p arcweft-bundle awfb_manifest_policy_enums_parse_display_and_default --quiet
cargo test -p arcweft-bundle awfb_v1_decodes_zstd_embedded_sections_with_output_limit --quiet
cargo test -p arcweft-bundle awfb_v1_rejects_zstd_section_exceeding_decoded_budget --quiet
cargo test -p arcweft-bundle awfb_v1_external_section_descriptors_round_trip_without_payload --quiet
cargo test -p arcweft-bundle external_section --quiet
cargo test -p arcweft-bundle awfb_product --quiet
cargo test -p arcweft-bundle awfb_product_encodes_program_bytecode_as_binary_envelope --quiet
cargo test -p arcweft-bundle awfb_product_embeds_verified_compact_bytecode_table --quiet
cargo test -p arcweft-bundle --no-default-features awfb_product --quiet
cargo test -p arcweft-bundle awfb_v1_signing_digest_excludes_trailing_signature_block --quiet
cargo test -p arcweft-bundle awfb_rejects_runtime_types_layout_mismatch --quiet
cargo test -p arcweft-bundle bundle_format_codecs_round_trip_supported_formats --quiet
cargo test -p arcweft-bundle bundle_format_can_be_inferred_from_common_extensions --quiet
cargo test -p arcweft-bundle awfb_path_does_not_fall_back_to_json_decoder --quiet
cargo test -p arcweft-bundle legacy --quiet
cargo test -p arcweft-bundle inspection_ --quiet
cargo test -p arcweft-bundle product_path_requires_awfb_extension --quiet
cargo test -p arcweft-bundle duplicate --quiet
cargo test -p arcweft-bundle awfb_v1_header_index_and_embedded_sections_round_trip --quiet
cargo test -p arcweft-bundle awfb_v1_patch --quiet
cargo test -p arcweft-bundle patch --quiet
cargo test -p arcweft-bundle patch_awfb_uses_asset_blob_carriers_for_changed_payloads --quiet
cargo test -p arcweft-bundle patch_manifest_records --quiet
cargo test -p arcweft-bundle patch_materializes_external_section_descriptor_changes_without_payloads --quiet
cargo test -p arcweft-bundle patch_materializes_target_awfb_preserving_zstd_section_compression --quiet
cargo test -p arcweft-bundle signed_patch_bundle_decodes_and_applies_with_signature_policy --quiet
cargo test -p arcweft-bundle release --quiet
cargo test -p arcweft-bundle release_manifest_builds_sorted_fetch_plan_and_verifies_result --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_carries_retry_cancel_and_budget_policy --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_candidate_over_byte_budget --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_missing_content_root --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_enforces_required_awfb_signature --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_accepts_trusted_signature_envelope --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_untrusted_signature_envelope --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_invalid_signature_envelope --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_signature_envelope_for_other_content_root --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_signature_envelope_with_wrong_signing_digest --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_verifies_ed25519_signature_payload --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_accepts_rotated_ed25519_key_epoch --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_revoked_ed25519_key --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_ed25519_key_epoch_outside_validity --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_bad_ed25519_signature_payload --quiet
cargo test -p arcweft-bundle release_manifest_rejects_invalid_signature_policy --quiet
cargo test -p arcweft-bundle release_manifest_rejects_unknown_signature_algorithm_policy --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_rejects_unsupported_signature_envelope_algorithm --quiet
cargo test -p arcweft-bundle release_manifest_rejects_invalid_trusted_key_epoch_window --quiet
cargo test -p arcweft-bundle release_manifest_fetch_plan_carries_network_client_policy --quiet
cargo test -p arcweft-bundle release_manifest_rejects_invalid_network_policy_profiles --quiet
cargo test -p arcweft-bundle release_manifest_rejects_invalid_network_policy_user_agent --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_retries_failed_file_mirror_then_uses_next_mirror --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_skips_file_candidate_over_byte_budget --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_reads_http_mirror_with_timeout_and_budget --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_requires_https_policy_skips_http_then_uses_file_mirror --quiet
cargo test -p arcweft-project-loader network_policy_rejects_profiles_unavailable_to_default_cache_client --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_attempts_https_mirror_then_uses_file_mirror --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_rejects_http_body_over_byte_budget --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_rejects_unsigned_file_when_signature_required --quiet
cargo test -p arcweft-project-loader fetch_release_bundle_rejects_untrusted_signature_envelope --quiet
cargo test -p arcweft-cli sign_awfb_bytes_appends_verifiable_release_signature --quiet
cargo test -p arcweft-cli static_image_asset_refs --quiet
cargo test -p arcweft-bundle --quiet
cargo test -p arcweft-core rejects_runtime_layout_signature_mismatch --quiet
cargo test -p arcweft-core rejects_missing_entrypoint --quiet
cargo test -p arcweft-core bytecode --quiet
cargo test -p arcweft-core compact_bytecode --quiet
cargo test -p arcweft-core --quiet
cargo test -p arcweft-runtime-driver session_rejects_unverified_bytecode_before_construction --quiet
cargo test -p arcweft-runtime-driver session_rejects_missing_bytecode_entrypoint_before_construction --quiet
cargo test -p arcweft-runtime-driver swap --quiet
cargo test -p arcweft-runtime-driver --quiet
cargo test -p arcweft-runtime-driver --test session --quiet
cargo test -p arcweft-runtime-driver patch_readiness --quiet
cargo test -p arcweft-runtime-driver inline_image_call_accepts_runtime_length_labels --quiet
cargo test -p arcweft-runtime-driver hot_swap_patch_bytes_reports_restart_required_before_materializing_target --quiet
cargo test -p arcweft-runtime-driver generation_pin_retains_old_bundle_generation_until_handle_drops --quiet
cargo test -p arcweft-runtime-driver active_fiber_pin_retains_old_generation_until_fiber_finishes --quiet
cargo test -p arcweft-runtime-driver pending_task_pin_survives_code_compatible_runtime_rebuild --quiet
cargo test -p arcweft-runtime-host bundle_runner_rejects_unverified_bytecode_before_execution --quiet
cargo test -p arcweft-runtime-host bundle_runner_rejects_missing_bytecode_entrypoint_before_execution --quiet
cargo test -p arcweft-runtime-host --test bundle_runner --quiet
cargo test -p arcweft-runtime-host --test bundle_runner bundle_file_runner --quiet
cargo test -p arcweft-render-native pure_text --quiet
cargo test -p arcweft-adapter-context --quiet
cargo test -p arcweft-adapter-context --features sema --quiet
cargo test -p arcweft-adapter-desktop --quiet
cargo test -p arcweft-verify-lsp --quiet
cargo test -p arcweft-lsp diagnostics_use_profile_selected_adapter_environment --quiet
cargo test -p arcweft-player-native --quiet
cargo test -p arcweft-player-native default_input_requires_awfb_bundle --quiet
cargo test -p arcweft-player-native product_awfb_input_does_not_fall_back_to_legacy_json --quiet
cargo test -p arcweft-player-native native_patch_endpoint --quiet
cargo test -p arcweft-player-native native_patch_endpoint_accepts_cli_style_cwd_relative_patch_path --quiet
cargo test -p arcweft-cli watch_ --quiet
cargo test -p arcweft-cli watch_inputs_ --quiet
cargo test -p arcweft-cli watch_patch_transport --quiet
cargo test -p arcweft-cli patch_bundle_artifact_helper_diffs_base_and_next_awfb_bytes --quiet
cargo test -p arcweft-cli run_bundle_applies_awfb_patch_before_execution --quiet
cargo test -p arcweft-cli inspect --quiet
cargo test -p arcweft-cli cache --quiet
cargo test -p arcweft-cli fetch_populates_cache_from_file_release_manifest --quiet
cargo test -p arcweft-cli explain_rejects_invalid_digest_query --quiet
cargo test -p arcweft-cli release_project_diagnostics_reject_dynamic_goto --quiet
cargo test -p arcweft-core observation_records_generic_runtime_calls_for_adapters --quiet
cargo check -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --all-targets
cargo check -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets
cargo check -p arcweft-bundle --all-targets
cargo check -p arcweft-bundle --no-default-features --all-targets
cargo check -p arcweft-bundle --all-targets --no-default-features
cargo check -p arcweft-project --all-targets
cargo check -p arcweft-core -p arcweft-runtime-driver -p arcweft-runtime-host --all-targets
cargo check -p arcweft-compiler --all-targets
cargo check -p arcweft-render-native --all-targets
cargo check -p arcweft-player-native --all-targets
cargo check -p arcweft-cli --all-targets
cargo check -p arcweft-cli --all-targets --no-default-features
cargo check -p arcweft-runtime-host -p arcweft-cli -p arcweft-player-native --all-targets
cargo check -p arcweft-adapter-context --all-targets
cargo check -p arcweft-adapter-context --all-targets --features sema
cargo check -p arcweft-adapter-desktop --all-targets
cargo check -p arcweft-host-adapter -p arcweft-runtime-host -p arcweft-player-native --all-targets
cargo check -p arcweft-verify-lsp --all-targets
cargo check -p arcweft-lsp --all-targets
cargo check -p arcweft-player-web --all-targets
cargo check -p arcweft-cli --all-targets
cargo check -p arcweft-bundle -p arcweft-runtime-driver --all-targets
cargo check -p arcweft-core -p arcweft-bundle --all-targets
cargo check -p arcweft-runtime-driver --all-targets
cargo check -p arcweft-core -p arcweft-runtime-driver -p arcweft-runtime-host --all-targets
cargo check -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets
cargo check -p arcweft-project-loader -p arcweft-cli --all-targets
cargo check -p arcweft-launch -p arcweft-project-loader -p arcweft-cli --all-targets
cargo check -p arcweft-cli --all-targets
cargo check -p arcweft-core -p arcweft-runtime-driver --all-targets
cargo check -p arcweft-core -p arcweft-bundle -p arcweft-runtime-driver --all-targets
cargo check -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets
cargo check -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-bundle -p arcweft-cli --all-targets
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-hir -p arcweft-lang-sema --all-targets --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets --all-features
cargo clippy -p arcweft-bundle --all-targets --all-features
cargo clippy -p arcweft-core --all-targets --all-features
cargo clippy -p arcweft-core -p arcweft-bundle --all-targets --all-features
cargo clippy -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-runtime-host --all-targets --all-features
cargo clippy -p arcweft-core -p arcweft-runtime-driver -p arcweft-runtime-host --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-project-loader -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-launch -p arcweft-project-loader -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-project --all-targets --all-features
cargo clippy -p arcweft-compiler --all-targets --all-features
cargo clippy -p arcweft-core -p arcweft-runtime-driver --all-targets --all-features
cargo clippy -p arcweft-runtime-plan -p arcweft-render-native -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-adapter-context -p arcweft-adapter-desktop -p arcweft-host-adapter -p arcweft-runtime-host -p arcweft-player-native -p arcweft-verify-lsp -p arcweft-lsp --all-targets --all-features
cargo clippy -p arcweft-player-native --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-player-native --all-targets --all-features
cargo clippy -p arcweft-runtime-host -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-project-loader -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-bundle -p arcweft-project-loader -p arcweft-cli -p arcweft-lang-sema -p arcweft-runtime-plan --all-targets --all-features
cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema -p arcweft-bundle -p arcweft-cli --all-targets --all-features
cargo fmt --all -- --check
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-smoke-snapshot-out --watch --watch-iterations 1 --watch-poll-ms 1
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-cache-smoke-out
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-cache-json-smoke-out --json
cargo run -p arcweft-cli --quiet -- cache verify --root target/codex/project-build-cache-smoke-out/cache/v1 --json
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-cache-reuse-smoke-out --json
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-cache-reuse-smoke-out --json
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-watch-cache-manifest-src/arcw.toml --target-dir target/codex/project-build-watch-cache-manifest-out --watch --watch-iterations 10 --watch-poll-ms 400
cargo run -p arcweft-cli --quiet -- run samples/visual-novel-mini/src/game.arcw --runner native --watch --watch-iterations 1 --watch-poll-ms 1
cargo run -p arcweft-cli --quiet -- run target/codex/native-watch-change-src/src/game.arcw --runner native --watch --watch-iterations 8 --watch-poll-ms 500
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-incremental-hot-swap-2026-06-23
cargo +nightly -Zscript tools/arcweft-structure-audit.rs --root . --write docs/implementation/structure-audit-incremental-hot-swap-2026-06-23
cargo tree -p arcweft-player-native --edges normal -i arcweft-compiler
cargo tree -p arcweft-player-native --all-features --edges normal -i arcweft-compiler
cargo tree -p arcweft-player-native --edges normal -i arcweft-runtime-plan
cargo tree -p arcweft-player-native --all-features --edges normal -i arcweft-runtime-plan
cargo tree -p arcweft-player-native --edges normal -i arcweft-lang-syntax
cargo tree -p arcweft-player-native --all-features --edges normal -i arcweft-lang-syntax
cargo tree -p arcweft-player-native --edges normal -i arcweft-lang-sema
cargo tree -p arcweft-player-native --edges normal -i arcweft-lang-hir
cargo tree -p arcweft-player-native --edges normal -i arcweft-verify
cargo tree -p arcweft-player-native --edges normal -i arcweft-verify-lsp
cargo tree -p arcweft-player-web --edges normal -i arcweft-compiler
cargo tree -p arcweft-player-web --edges normal -i arcweft-runtime-plan
cargo tree -p arcweft-player-web --edges normal -i arcweft-lang-syntax
cargo tree -p arcweft-player-web --edges normal -i arcweft-lang-hir
cargo tree -p arcweft-player-web --edges normal -i arcweft-lang-sema
cargo tree -p arcweft-player-web --edges normal -i arcweft-verify
cargo run -p arcweft-cli --quiet -- bundle web\demo.arcw --output target\arcweft-check\demo.awfb
cargo run -p arcweft-cli --quiet -- inspect target\arcweft-check\demo.awfb --json
cargo run -p arcweft-cli --quiet -- cache stats --json
cargo run -p arcweft-cli --quiet -- cache verify --json
cargo run -p arcweft-cli --quiet -- cache prune --json
cargo run -p arcweft-cli --quiet -- run-bundle target\arcweft-check\demo.awfb --steps 1 --max-ops 8 --json
cargo run -p arcweft-cli --quiet -- bundle web\demo.arcw --output target\arcweft-check\format-direct.awfb
cargo run -p arcweft-cli --quiet -- run-bundle target\arcweft-check\format-direct.awfb --steps 1 --max-ops 8 --json
cargo run -p arcweft-cli --quiet -- bundle web\demo.arcw --output target\arcweft-check\dedup-validation.awfb
cargo run -p arcweft-cli --quiet -- run-bundle target\arcweft-check\dedup-validation.awfb --steps 1 --max-ops 8 --json
cargo run -p arcweft-player-native --quiet -- target\arcweft-check\dedup-validation.awfb --headless --json --steps 1
cargo run -p arcweft-cli --quiet -- run samples/visual-novel-mini/src/game.arcw --runner web --watch --watch-iterations 1 --watch-poll-ms 1
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-smoke-out
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-smoke-watch-out --watch --watch-iterations 1 --watch-poll-ms 1
cargo run -p arcweft-cli --quiet -- build --manifest target/codex/project-build-smoke-src/arcw.toml --target-dir target/codex/project-build-smoke-patch-out --patch-base target/codex/project-build-smoke-out/debug/build_smoke.awfb
cargo run -p arcweft-cli --quiet -- bundle web\demo.arcw --output target\arcweft-check\demo-base.awfb
cargo run -p arcweft-cli --quiet -- patch --base target\arcweft-check\demo-base.awfb --next target\arcweft-check\demo-base.awfb --output target\arcweft-check\demo-noop.patch.awfb --json
```

Additional `arcw build --watch` smoke validation used a temporary project under
`target/codex/project-build-watch-change-src`, waited for the initial build
AWFB, rewrote `src/main.arcw`, and observed a patch artifact:

```text
watch: patch target/codex/project-build-watch-change-out\debug\patches\build_watch_change-23c2898b36c0b0d5adec3faab339be0b0c7ac078fc82a6749e9351624e30906a-dbf97dad5a512e036dabdb4411f62c73b9833f559809692e80a40bd3df6288c0.awfb (2 operation(s), compatibility=code-generational)
```

Additional `arcw run --watch --runner native` smoke validation used a temporary
copy of `samples/visual-novel-mini` under `target/codex/native-watch-change-src`,
waited for the initial watch build, appended a comment to `src/game.arcw`, and
observed the in-process native endpoint apply the emitted content-only patch:

```text
watch: patch target/arcweft/run\patches\game-63a1c6adef02449eca305d6600922874ddcf7b26860796ed145d1703a9bbd76c-9903e9312fd32ee68255159f4a46b2026283a577eaba5e59c44dfd4f25e416ae.awfb (1 operation(s), compatibility=content-only, transport=target/arcweft/run\patches\game-63a1c6adef02449eca305d6600922874ddcf7b26860796ed145d1703a9bbd76c-9903e9312fd32ee68255159f4a46b2026283a577eaba5e59c44dfd4f25e416ae.transport.json, action=apply-patch, native_endpoint=applied)
```

`cargo tree -p arcweft-player-native --edges normal -i arcweft-compiler` and
`cargo tree -p arcweft-player-native --edges normal -i arcweft-runtime-plan`
are expected to exit with "package ID specification ... did not match any
packages"; that is the evidence that those crates are no longer present in the
native player normal dependency tree. The corresponding
`arcweft-lang-syntax`/`arcweft-lang-hir`/`arcweft-lang-sema` tree queries now
also exit with the same package-ID miss, which is the evidence that the native
player product tree no longer includes the language parser/HIR/sema stack
through adapter manifests. The same package-ID miss is now recorded for
`arcweft-player-web` against `arcweft-compiler`, `arcweft-runtime-plan`,
`arcweft-lang-syntax`, `arcweft-lang-hir`, `arcweft-lang-sema`, and
`arcweft-verify`.

`cargo run -p arcweft-cli --quiet -- cache explain not-a-digest --json` is a
negative CLI smoke and is expected to exit unsuccessfully with an
`invalid_query` report.

`cargo test -p arcweft-cli --test arcw_fixtures_check_run
spec_should_pass_check_fixtures_pass_after_refactor --quiet` was also tried
after updating asset-reference fixtures, but it currently fails before reaching
the changed fixture because `arcw_fixtures_check_run.rs` still invokes
`arcw check <path>` while the current CLI help exposes manifest-based
`arcw check [--manifest-path <MANIFEST>]` and no positional source path. The
source fixture itself was covered through the sema contract fixture test above.

This negative product-path smoke is expected to exit unsuccessfully with
`AWFB container magic does not match`:

```bash
cargo run -p arcweft-cli --quiet -- bundle web\demo.arcw --format json --output target\arcweft-check\legacy-json-product-path.awfb; cargo run -p arcweft-cli --quiet -- run-bundle target\arcweft-check\legacy-json-product-path.awfb --steps 1 --max-ops 8
```

This native product-player smoke is expected to exit unsuccessfully before
decode because explicit inspection exports are not product bundles:

```bash
cargo run -p arcweft-cli --quiet -- bundle web\demo.arcw --format json --output target\arcweft-check\inspection-only.awfb.json; cargo run -p arcweft-player-native --quiet -- target\arcweft-check\inspection-only.awfb.json --headless --json --steps 1
```

Structural audit result for this cut: `0` errors, `97` warnings across `784`
Rust files and `385567` physical Rust LOC. The warning set is recorded under
`docs/implementation/structure-audit-incremental-hot-swap-2026-06-23/`.

Resolved validation interruption:

```bash
cargo test -p arcweft-bundle --quiet
```

This broader crate test initially failed because D: had no remaining free
space while rustc was writing build output. After generated Cargo artifacts
under `target/debug` were removed, the command was rerun successfully:
`72 passed; 0 failed`.
