# Tier 2 Agent/MCP Harness Reconciliation — 2026-07-19

## Scope

This cut reconciles the ignored Tier 2 Agent/MCP and native-capture harness
with the accepted dialogue View and prepared-text production contracts. It also
records when future broad integration cuts must run the exhaustive Tier 2
route.

The production dialogue contract remains:

```text
dialogue runtime state
  -> authored persistent View mount
  -> ResolvedTextDocument
  -> TextLayout
  -> PreparedTextBatch
  -> ViewPrimitive::Text
  -> ViewCompositor
  -> SharedRenderer
```

The harness must follow that contract. It must not restore ordinal dialogue
object IDs, frame-zero resource URIs, pre-View dialogue geometry, permissive
auxiliary-capture publication, or a second dialogue renderer.

## Tier 2 trigger

`AGENTS.md` and `docs/implementation/test-execution-policy.md` now require
`just test-tier2` before completing a broad integration cut when both of these
conditions hold:

1. the cut spans multiple crates or materially changes a public contract; and
2. it affects a runtime, render, Agent, MCP, or capture path.

An isolated small public-API edit does not qualify solely because it is public.
The gate is intended for integration-scale changes whose downstream slow
harness can otherwise remain stale while focused and normal workspace tests
pass.

When the Tier 2 harness exposes an obsolete expectation, the harness and its
canonical fixtures must follow the accepted production contract. Production
must not gain compatibility aliases, duplicate paths, or old semantic
identities merely to satisfy a slow test.

## Harness reconciliation

### Entry selection

Temporary and sample sources used by the MCP harness now expose explicit
entries, and Agent/MCP requests select those entries. This keeps the slow path
on the same launch contract as production instead of depending on an implicit
first flow.

### Semantic identity and capture URI discovery

Tests no longer reconstruct rich-text object IDs or capture URIs from ordinal
dialogue positions or a presumed `frame/0`.

The harness first asks the MCP observation boundary for the authored object,
layer, page, frame, and capture references. Subsequent capture and readback
requests use the returned semantic IDs and exact typed resource URIs. This
also verifies that semantic source references remain usable across equivalent
MCP subprocess sessions without treating an ordinal spelling as a second
identity system. Policy-derived public identity is verified separately at the
publication boundary described below.

Source capture references and policy-published resources deliberately have
different identity domains. The observation report retains the canonical
`arcweft://session/...` capture reference and authored semantic scope. A
successful source-alias read resolves that reference to the
`arcweft://moderated/...` URI advertised by `resources/list`; the published
descriptor may also replace its scope ID with a policy-derived opaque ID. The
harness therefore joins source reads to published descriptors by the returned
public URI, verifies the published scope and selected-capture metadata are
internally consistent, and reads the public URI back. It never equates the
authored semantic ID with the published opaque ID or hard-codes a moderated
hash.

Viewport readback similarly uses the URI listed by an MCP observation that
requested the viewport image. It does not synthesize a frame URI in the test.
This adds one discovery subprocess to the relevant tests; the extra Tier 2
cost is intentional because it validates the public MCP boundary rather than a
test-only in-process projection.

### Semantic rich-text pages and runtime pages

Rich-text page identity and prepared-frame runtime page selection are separate
contracts. A semantic child such as
`object.dialogue.<dialogue>.<entry>.page.<page>` retains its authored page in
`rich_text_ref.page`. Its capture reference addresses the currently retained
prepared frame and therefore does not append that semantic index as a
`?page=` runtime query.

The object capture-ref builder no longer accepts a runtime page argument.
Current-frame object refs always use the default runtime page, preventing
callers from accidentally projecting a rich-text semantic index into an
unsupported prepared-frame page selector. The Tier 2 regression discovers a
semantic page whose index is one, verifies that its public capture ref has no
runtime page query, and reads that exact published ref successfully. The read
path was not relaxed and no alias for the previously unreadable URI was added.

### Link-oriented observation results

`arcweft.observe` is intentionally link-oriented at the MCP boundary. Its tool
result publishes `resource_link` blocks rather than embedding the complete
observation report. The harness now keeps the same strict MCP process alive,
selects the resource link titled `Latest observation`, and reads it through
standard `resources/read` with the required privacy ceiling.

This behavior is not inferred only from the slow test. It is directly covered
by the lower-level
`resource_list_and_observe_tool_result_expose_resource_links` test in
`arcweft-agent-mcp`, and by the native MCP cache/read tests that verify a
listed moderated URI is readable. The `arcweft-agent-mcp` resource projection
also documents that observe results are link-oriented so clients can choose
which frame resources to fetch.

Capture assertions retain their stronger metadata-before-image ordering
requirement where that ordering is part of the direct capture response
contract.

### Strict policy and local development

The default MCP runner and the observation-resource discovery exchange keep the
strict content-policy profile. Tests that exercise semantic actions,
observation, hit testing, waits, tracing, and auxiliary-capture policy
therefore remain strict.

Only tests that must inspect actual color image bytes use the explicit
`local-dev` policy mode. Mask and object-ID requests under strict policy assert
the externally observable review receipt:

- `review` disposition;
- `auxiliary_capture_not_publishable` reason;
- strict profile identity;
- unsanitized receipt metadata;
- moderated JSON resource publication.

Exact raw mask/object-ID bytes, geometry, capture-time, reveal, and attachment
behavior remain covered by `just test-native-aux-capture`. That recipe runs
the horizontal and vertical-lr text-combine mask and object-ID cases, plus both
writing-mode variants of the typewriter text-combine and ruby color/object-ID
timing cases. `just test-tier2` depends
on this target, so these lower-native assertions are part of the exhaustive
route rather than merely existing elsewhere in the integration binary. The MCP
policy tests do not pretend that a review receipt contains renderer pixels.

### Prepared-text coverage and bounded readback

Mask and object-ID captures now derive coverage from the same selected prepared
glyphs and renderer alpha used by color rendering. They no longer invert RGB
output or replace a failed glyph selection with a bounding rectangle. Mask
generation is independent of object-ID assignment, object-ID capture requires
an opaque selected ID, and painter order is applied while each selected region
is processed.

The offscreen renderer reuses one coverage texture. Each selected region is
rendered, cropped, read back, and stamped into the destination immediately;
it does not retain one full-frame GPU/readback buffer per region. The typed
budget fails closed at:

- at most 128 coverage passes;
- at most `32 * 1920 * 1080` checked `u64` rendered pixels; and
- at most 64 MiB of cropped RGBA readback, measured from the actual
  256-byte-padded GPU row stride rather than unpadded logical RGBA bytes.

Exact-limit, one-over-limit, and arithmetic-overflow tests cover those
boundaries, including a thin/tall region whose padded transfer is 64 times its
logical RGBA byte count. The semantic View-text identity also includes its
mount occurrence, and URI components use injective UTF-8 percent encoding, so
distinct semantic IDs such as `view/text` and `view_text` cannot collide.

Tier 2 additionally exposed a production Fx clock defect: dialogue Fx sampling
used snapshot-global time even though stage reveal and capture were
activation-relative. `DialogueFxResolver` now receives the typed stage-local
elapsed time. Typewriter text-combine and ruby mask/object-ID captures therefore
select exactly the glyphs visible at the requested capture time.

### Canonical standard dialogue placement

The standard dialogue View now owns its physical placement through canonical
typed Style:

```text
panel:
  position = absolute
  left = 57.6
  top = 460.8
  width = 1164.8
  height = 201.6

speaker local:
  left = 28
  top = 20
  global = (85.6, 480.8)

content local:
  left = 28
  top = 58
  global = (85.6, 518.8)

primary action local:
  (0, 0)
```

The shared outward-rounded capture bounds are therefore viewport origin
`(57, 460)` with size `1166 x 203`. The Tier 2 harness follows these authored
fractional bounds through the canonical floor-origin/ceil-edge conversion.
Physical geometry does not restore the deleted sidecar `x`/`y` fallback, and
the renderer, Agent geometry, layer capture, mask, and object-ID paths consume
the same placement result.

## Structural note

Measured at Jujutsu change `snvpuouypvtn`:

| Path | Class | Bytes | Physical LOC | Major responsibility |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-cli/src/app/agent/native.rs` | production facade | 15,798 | 333 | native Agent module wiring |
| `crates/arcweft-cli/src/app/agent/native/image_mapping.rs` | production | 30,709 | 917 | capture refs and image metadata |
| `crates/arcweft-cli/src/app/agent/native/player_observation.rs` | production | 36,735 | 1,056 | prepared-player observation projection |
| `crates/arcweft-cli/src/app/agent/native/prepared_text_observation.rs` | production | 40,997 | 1,201 | dialogue prepared-text semantic objects |
| `crates/arcweft-cli/src/app/agent/native/prepared_text_observation/view.rs` | production | 17,461 | 479 | authored View prepared-text objects |
| `crates/arcweft-cli/src/app/agent/native/player_observation/capture.rs` | production | 16,887 | 461 | selected-capture planning and semantic coverage |
| `crates/arcweft-cli/tests/check.rs` | integration-test root | 27,806 | 801 | CLI integration harness and includes |
| `crates/arcweft-cli/tests/check/agent_observe_native.rs` | integration-test facade | 1,124 | 24 | native observation test modules |
| `crates/arcweft-cli/tests/check/agent_observe_native/core.rs` | integration test | 60,021 | 1,689 | observation and hit-test contracts |
| `crates/arcweft-cli/tests/check/agent_observe_native/mcp_native_capture.rs` | integration test | 87,513 | 2,367 | MCP policy, capture, and readback |
| `crates/arcweft-cli/tests/check/agent_observe_native/selected_capture_metadata.rs` | integration test | 2,537 | 61 | selected-capture metadata |
| `crates/arcweft-cli/tests/check/agent_script_debug.rs` | integration test | 196,265 | 5,257 | Agent scripts, trace, search, and RAG |
| `crates/arcweft-render-wgpu/src/offscreen.rs` | production | 42,211 | 1,170 | offscreen render orchestration |
| `crates/arcweft-render-wgpu/src/offscreen/budget.rs` | production | 4,205 | 118 | checked coverage work/readback limits |
| `crates/arcweft-render-wgpu/src/offscreen/readback.rs` | production | 6,568 | 181 | cropped GPU readback |
| `crates/arcweft-render-wgpu/src/offscreen/tests.rs` | unit test | 26,710 | 846 | offscreen capture and limit contracts |
| `crates/arcweft-render-wgpu/src/view_scene/core.rs` | production | 22,388 | 754 | View primitives and selected-text paint ancestry |
| `crates/arcweft-runtime-driver/src/display.rs` | production | 37,395 | 1,015 | atomic display update, typed background decoding, and dialogue Fx ownership |
| `crates/arcweft-runtime-driver/src/display/tests.rs` | unit test | 38,467 | 1,126 | display, background rejection, and stage-local Fx contracts |

Three direct dependency changes are intentional:

- `arcweft-agent-protocol -> blake3` seals canonical trace publication against
  the complete typed resource body;
- `arcweft-render-text -> arcweft-view` carries semantic `ViewId` in the
  serialized display contract; and
- `arcweft-runtime-plan -> arcweft-view` validates authored dialogue View
  identity during lowering.

No Cargo feature was added. The renderer split keeps GPU readback and work
budgets inside `arcweft-render-wgpu`; the display split keeps runtime lifecycle
and stage-local Fx ownership inside `arcweft-runtime-driver`.

`prepared_text_observation.rs` is one line above the production warning
threshold after the semantic identity changes. It remains one cohesive
dialogue prepared-text projection and has no embedded tests, transport,
renderer, or persistence work. This audit records the warning; a later change
that adds another responsibility must split it rather than claim this narrow
exception indefinitely.

`crates/arcweft-cli/tests/check/agent_script_debug.rs` is a large integration
test file and remains above the integration-test warning threshold. This cut
only adjusts the trace publication assertion and entry selection. A future
structural slice should separate at least:

- Agent script execution and persistence;
- MCP trace/resource publication;
- debug search and RAG scenarios.

That split is not required to preserve the observable contracts fixed here and
is intentionally not mixed into this harness reconciliation.

## Validation history

The stale baseline reported 25 slow MCP tests with 9 passing and 16 failing.
After entry, policy, identity, and geometry reconciliation, an intermediate
run reached 23 tests with 15 passing and 8 failing. The reduced test count
comes from consolidating three misleading auxiliary-renderer assertions into
two strict policy-boundary scenarios.

A later 23-test run overlapped production edits and reported 11 passing and 12
failing. Eleven failures shared one harness cause: observation is now
intentionally resource-link-only, while fixture discovery still expected an
inline text block. Discovery now follows the public link/read contract in the
same strict MCP session. The remaining animated-image failure was a production
expected-type/initialization issue handled by the owning implementation slice.

The animated-image production regression is resolved. Presentation-call sema
tests pass 15/15, including public-ID positive/negative and token-scalar
preservation cases; the focused compiler runtime-plan test passes 1/1,
`arcw check samples/image-animation.arcw` succeeds with 9 flows and no warnings
or obligations, and all-target Clippy passes for the sema and compiler crates.
The fix preserves `opacity = 0.5` as a typed floating-point argument rather
than weakening the sample or adding a special-case parser path.

Final commands and outcomes:

```text
cargo check --workspace --all-targets --all-features
  PASS

cargo clippy --workspace --all-targets --all-features -- -D warnings
  PASS

just test-workspace
  PASS

just test-tier2
  PASS
  MCP stdio: 23/23
  slow Agent observe: 1/1
  native auxiliary capture: 16/16
  visual smoke: 2/2
  fixture integrity: 1/1
  exact visual goldens: 4/4

cargo test -p arcweft-player-web --test parity
  PASS: 7/7

cargo test -p arcweft-bundle --test standard_dialogue_view
  PASS: 13/13

cargo clippy -p arcweft-cli --features native-capture --test check -- -D warnings
  PASS

cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS: 3,269 files, 1,673 Rust files, 770,310 physical Rust LOC,
  92 manifests, 0 errors, 132 warnings

cargo fmt --all -- --check
  PASS

git diff --check
  PASS
```

Strict Clippy identified three overlong test/support functions while the
shared changes were settling. The standard dialogue style assertions, ruby
mask report assertions, and Web Fx dialogue-mount assertions were extracted
into responsibility-named helpers. No lint suppression was added.

## Compatibility and deviations

No compatibility alias, dual reader, removed-syntax recognizer, source gate,
old semantic ID, or alternate dialogue/text rendering path was added.

There is no design deviation from the accepted dialogue View ownership model.
The deliberate test-cost increase consists of MCP response-driven identity and
URI discovery plus the lower-native auxiliary matrix exercised by
`just test-native-aux-capture`. The latter keeps animated-image, text-combine,
typewriter, ruby, mask, and object-ID renderer evidence in Tier 2 instead of
pretending that strict MCP review receipts contain pixel data.
