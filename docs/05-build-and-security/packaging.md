# Packaging and product flags

Packaging is split into two layers:

- Bundle format crates define Sans I/O data structures and deterministic codecs over `&[u8]`, `Vec<u8>`, and manifest strings.
- CLI/build/player adapters perform filesystem reads/writes, embedding, compression selection, signing, upload, and platform storage.

`.awfb` is a portable data artifact. Opening a path, watching a directory, fetching a remote bundle, or writing a crash report is never part of `arcweft-core` or the bundle data model.
Product readers use the AWFB-only decode path. Legacy JSON and other structured
formats are inspection/export surfaces selected by explicit extensions such as
`.awfb.json`; they are not probed as fallback shipping codecs.

## Feature flags

```toml
[features]
default = ["vm", "wgpu-render", "audio-basic"]

native = ["native-st", "wgpu-render", "audio-native"]
native-mt = ["tokio", "rayon"]
native-jit = ["arcweft-lang-jit-cranelift"]
web = ["web-st", "dom-view", "audio-web"]
web-mt = ["web-workers", "wasm-bindgen-rayon"]

agent-observe = []
agent-control = ["agent-observe"]
agent-debug-mutate = ["agent-control"]
agent-mcp = ["agent-observe"]

servo-view = []
dom-view = []
```

## Runtime flags

```text
--agent=off|observe|control|debug
--observe=off|metrics|logs|agent|debug
--debug-assertions=on|off
--audio-backend=native|web|dummy
--headless
```

## Bundle contents

Current CLI bundle slice:

```text
game.awfb
  schema_version
  manifest
    source_label
    profile_id
    profile_kind
    entry
    adapter
    adapter_manifest_ids
    required_host_calls
    runtime
      entry_flow
      flows
      bytecode_instructions
      line_task_groups
      stream_plans
      source_plans
  source
    label
    text
  bytecode
    encoding = structured_json
    program
      entry_flow
      entries
      flows
      pure_helpers
      line_task_groups
      stream_plans
      source_plans
  adapter_manifests[]
    id
    display_name
    effects
    host_calls[]
      id
      effects
  virtual_files[]
    space
    path
    bytes
  image_assets[]
    id
    file
      space
      path
    format = png|jpeg|gif|webp
    animation = static|animated
    dimensions?
      width
      height
```

The product `.awfb` codec is the AWFB v1 fixed-header container owned by
`arcweft-bundle`. It has a fixed magic/version/header, a canonical manifest byte
range, a canonical section index, embedded/external section descriptors, BLAKE3
stored/content digests, bundle-kind section validation, unknown required-section
rejection, optional unknown-section skipping, and read budgets. Product
manifests at schema version 4 and later declare
`executable_payload = "awbc_v1"`, and their `ProgramBytecode` section is exactly
canonical bytes from `AwbcProgram::encode_canonical()`. Structured
`BytecodeProgram` data remains an inspection/export surface only and is not read
by product `.awfb` runtime execution. Runtime types, entrypoints, adapter
requirements, content catalog, display catalog, and normalized source still use
section-specific typed JSON payloads. See `product-awfb-bytecode.md` and
`docs/schemas/product-awfb-awbc-v1.json` for the AWBC-only product payload
contract.

The same Sans I/O bundle model also exposes explicit JSON, TOML, YAML,
MessagePack, CBOR, and Avro encode/decode entrypoints for inspection and
interoperability. Use `.awfb.json` or `--format json` for JSON inspection
exports; `.awfb` is the product container path and is selected by default for
`arcw bundle` and by magic/version probing in `arcw run-bundle`.

`.awfr` release manifests are Sans I/O JSON manifests that locate external AWFB
files by digest. Each entry records the external bundle content root, whole-file
digest, byte length, bundle kind, and one or more mirror URIs. The manifest
never trusts mirror URLs as identity: after an adapter fetches or opens a
candidate AWFB, the bytes must match the recorded byte length and whole-file
digest before the bundle is considered available. Supported mirror URI schemes
are limited to adapter-owned `https:`, `http:`, `file:`, and `arcweft-cache:`
locations; network I/O, secret resolution, and cache writes remain outside the
bundle data model. The Sans I/O release API can produce a deterministic fetch
plan for a content root: mirrors are ordered by priority, and the same plan
verifies candidate bytes before an adapter publishes them into a cache or
runtime mount. Fetch policy can also carry network client policy: HTTPS-only
transport, proxy profile id, auth profile id, client profile id, and
user-agent. These are policy/profile names rather than secrets. The current
filesystem cache adapter can consume `arcweft-cache:`, `file:`, plain
`http://`, and TLS-validated `https://` fetch plans; its default network client
enforces HTTPS-only policy and refuses network mirrors that require unresolved
proxy, auth, or client profiles so it never silently bypasses release policy.
Release manifests can also require that candidate AWFB files carry a signature
block, optionally with a minimum byte length. When trusted signer ids are
listed, the signature block must decode as a release signature envelope and its
`signer_id` must match the manifest policy. The envelope also records the
bundle `content_root`, `kind`, and canonical AWFB signing digest. The signing
digest excludes the trailing signature block and normalizes the signature header
fields, so an envelope for one AWFB cannot satisfy the policy for another
container payload. Signature policy also carries a small algorithm registry:
`allowed_algorithms` defaults to `ed25519-v1`, rejects empty/duplicate/unknown
entries during manifest validation, and rejects envelopes whose algorithm is
not in that registry. When `trusted_public_keys` are configured, the same Sans
I/O release verifier checks the envelope's Ed25519 v1 signature payload against
the typed signing message and a matching trusted public key. The envelope
carries a deterministic `key_epoch`, and trusted keys can declare
`valid_from_key_epoch`, `valid_until_key_epoch`, and `revoked`; the verifier
selects only keys valid for that epoch before checking the Ed25519 payload. This
keeps release key rotation and revocation Sans I/O and avoids wall-clock reads
inside the bundle model. Signer-id-only policy remains a trust-root selection
gate for deployments that perform cryptographic verification in an external
release-verifier workflow, but the envelope algorithm still has to be supported
and allowed by the manifest. Patch AWFB decode/apply entry points can take the
same signature policy, allowing a player or host adapter to reject unsigned or
incorrectly signed patch bundles before materializing a target bundle.

`arcweft-bundle` performs no filesystem, clock, network, signing, or
compression work. CLI/build/player adapters are responsible for turning source
trees and virtual file roots into bundle values, and for materializing bundle
values into a runnable host workspace. `arcw run-bundle` executes the decoded
bytecode section directly and does not parse, typecheck, or lower the source
text again.

The Avro bundle codec uses an Avro Object Container envelope whose payload is
the stable JSON bundle representation. Full schema-native Avro sections remain
available through the separate `arcweft-codec-avro` data adapter when a caller
has a concrete Avro schema for tabular or streaming data.

The CLI includes the manifest-selected authored `assets/` root by default and
can opt into `.arcweft/save`, `.arcweft/temp`, and `.arcweft/export` from the
local state root. Authored `content/` is an input to typed bundle content/View
sections rather than mutable local state. Packaged virtual paths use only
normal relative components. Parent traversal, absolute paths, and host path
prefixes are rejected or omitted before encoding. See
[Authored resource and local state storage](authored-resource-storage.md) for
the source-tree, version-control, and external-asset policy.

Image assets are typed bundle records that bind a stable asset id to one
encoded asset virtual file. Static PNG/JPEG/WebP and animated GIF/WebP use the
same `image_assets[]` section. The bundler decodes the encoded asset once while
building the bundle to record actual static/animated state and intrinsic
dimensions; adapters decode bytes again after looking up the referenced virtual
file for frame upload and playback, so bundle execution can use encoded payloads
without re-reading or re-lowering source files.

The CLI bundler derives image asset records for image files under the authored
asset root: `bg/room.png` becomes `asset.bg.room`, `view/logo.webp` becomes
`asset.view.logo`, and the record points at the matching asset virtual file.
PNG/JPEG are marked `static`; GIF/WebP are marked from decoded frame count, so
static WebP remains a normal one-frame image while multi-frame GIF/WebP is
marked `animated`.

When the lowered runtime plan contains a statically known
`asset.image(@asset:.id)` or `asset.image("asset.id")` request, `arcw bundle`
requires that id to exist in `image_assets[]`. Dynamic image asset expressions
remain runtime/adapter responsibility, but static source references cannot
silently produce a bundle with no matching encoded image asset.

`arcw run-bundle` validates `image_assets[]` before materializing the temporary
bundle workspace. An image asset whose referenced virtual file is missing,
whose encoded bytes cannot be decoded as the declared format, or whose recorded
static/animated state or dimensions contradict the decoded payload is a bundle
structure error and fails before bytecode execution starts.

Future product bundle slices can replace the remaining structured
`BytecodeProgram` payload inside the `AWBC` envelope with compact deterministic
opcode tables and can replace JSON resource payloads with compact graph
indexes, entity tables, source maps, contracts, shaders, View, audio, and text
resources as typed AWFB v1 sections. Those are separate design tracks: the
executable `AWBC` table is tracked by
`docs/reviews/requests/2026-06-24-awbc-executable-compact-table-design.md`,
and compact resource section codecs are tracked by
`docs/reviews/requests/2026-06-24-product-resource-section-codecs-design.md`.

The compact opcode table verifier already lives as a Sans I/O core model under
`arcweft-core::compact_bytecode`: it validates raw opcodes and all operand
indices against explicit function, constant, content-unit, and runtime-type
table bounds. Product `AWBC` payloads carry that compact validation table now;
the final executable constant/function/type table design remains pending before
runtime execution can drop the structured program payload.

```text
game.awfb
  manifest
  bytecode.vm
  graph.index
  entities
  assets
  shaders
  view
  audio
  text
  source_maps
  contracts
```

Patch bundles are AWFB bundles with `BundleKind::Patch` and must carry a
`PatchPlan` section. The Sans I/O patch model compares old and new section
descriptors by section id and logical content digest, yielding deterministic
add/replace/remove operations plus exact base and target content roots. A player
must reject a patch whose base content root does not match the mounted
generation before preparing any runtime swap. The current patch artifact codec
stores a JSON patch manifest plus an embedded typed JSON `PatchPlan` section;
the manifest records base/target content roots and the supported runtime ABI
range, plus a conservative hot-swap compatibility class derived from changed
section kinds: content/catalog payloads are `content_only`, program bytecode is
`code_generational`, and runtime type/entrypoint/adapter-requirement changes
are `restart_required` until a later verifier can prove a narrower live
transition. Add/replace operations for embedded sections carry the changed
section bytes as `AssetBlob` carrier sections inside the patch AWFB; decode
maps those carriers back to the logical PatchPlan descriptors and verifies
carrier shape, logical descriptor fields, and content digests before exposing
the plan. `Patch` bundles do not carry executable sections directly.
External section operations carry descriptor metadata only: size and digest are
preserved in the target section index, but fetching bytes from mirrors or a
local content-addressed cache remains a host/release-manifest responsibility.

## Debug bundle

```text
bug-report/
  engine.json
  bundle-manifest.toml
  state-before.bin
  trace.arcwx
  agent-actions.ndjson
  logs.jsonl
  signals.json
  metrics.json
  screenshot.png
  overlay.png
  audio-state.json
  diagnostics.json
```
