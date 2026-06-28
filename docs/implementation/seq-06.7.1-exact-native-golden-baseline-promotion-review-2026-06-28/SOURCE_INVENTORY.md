# Source inventory

Date: 2026-06-28
Repository: `Sanzentyo/arcweft`
Inspected revision: `b0b45b44b2dd34573d991839d950b58091c314b4`
Access path: GitHub connector inspection plus local package-generation probes.
Local checkout validation: not available; direct `git clone` in the sandbox failed
because DNS/network access to github.com was unavailable.

## Project context

Arcweft is a layered, verified, agent-native narrative engine written in Rust.
The design docs describe a wgpu-centered visual novel engine with native and web
paths, `.arcw` DSL inputs, a Sans I/O core, typed runtime/data boundaries, and
Agent observe/capture tooling.

## Repository policy inputs inspected

| Path | Evidence used | Connector SHA / note |
| --- | --- | --- |
| `AGENTS.md` | workflow, architecture, exact task completion rules | `379dd8dcaeaadd7e8fa999268a11e6099a4f500b` |
| `docs/README.md` | Arcweft overview and docs entry points | `300cd131bab5bdfcd7fe3adce4f0101474b2a42c` |
| `docs/00-overview/architecture.md` | Sans I/O core, adapter boundary, native/web render context | `3ac5cb6f13cc70b70ebd462bca2ada3d1c6b8941` |
| `docs/reviews/requests/2026-06-28-seq-06.7.1-exact-native-golden-baseline-promotion-review-package.md` | request source | `553c810d69716e2bd5485cf775b7758eff0af1af` |
| `docs/implementation/seq-06.7-exact-native-golden-drift-stabilization-2026-06-28.md` | seq06.7 workflow and prior no-promotion decision | `34135689678fd049779f59b1360c0f6b188c2820` |
| `fixtures/visual-smoke-goldens/exact-native-golden-policy.json` | policy, gates, fixture paths, required env | `ca11ccb6ec14d2a29b931c92ad5cbc723594ed45` |
| `fixtures/native-golden-drift/vertical_tutr_golden.seq06.6-drift.json` | historical drift metrics | `1327137a46c4ec2725892c1c38df880c9ce56c89` |
| `fixtures/native-golden-drift/README.md` | confirms metadata-only prior evidence | `8c93ca7419b7959a09e79264ae1e613df6cd7c3e` |
| `tests/fixtures/native_capture/README.md` | native fixture purpose and promotion rules | `8c691456a32180f1a09fd8ab66b11a157586937c` |
| `docs/implementation/test-execution-policy.md` | Tier 2 exact native semantics | `a23d649cd8d61288f886d29dbbfef758425b9914` |
| `docs/implementation/fixture-regeneration.md` | reviewed promotion and candidate retention | `d1467b2cb563331c9d780d43ae334ac9a621385f` |
| `Justfile` | exact native golden commands and artifact paths | `38a26282702662913c1c9cf3b0e3bc3758725cb3` |
| `tools/write-native-golden-fingerprint.rs` | environment fingerprint implementation | `095c642c16747b8bd65c9b407c0b8adfe318da4f` |
| `crates/arcweft-cli/tests/check/agent_observe_native/native_vertical.rs` | exact golden tests, thresholds, status classification | `dc37db6cfbe8cdf9abe20b3cc83a7517adcc76bb` |

## Native capture fixture inputs inspected

| Path | Role | Connector SHA / note |
| --- | --- | --- |
| `tests/fixtures/native_capture/vertical_tutr_golden.arcw` | review target source fixture | `878e3997fab3c239a5d3ca49f4ba31694b4b5edc` |
| `tests/fixtures/native_capture/vertical_tutr_golden.png` | checked-in reference PNG | `18615fcee9c1ad9f9c24a65d94ad937a8d02544f` |
| `tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.arcw` | sibling exact fixture source | `614f9961c533427c6213e5193e4e795339856c2c` |
| `tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.arcw` | sibling exact fixture source | `f4412b0ce14a91847b25bcbe0bf49e326a93821a` |
| `tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.arcw` | sibling exact fixture source | `aba8c17fceaa073032a5920c8cccde9f50d43783` |

## Artifacts unavailable in this package

These were required for a promotion/rejection review but were not produced here
because the environment was not Windows and had no pinned native stack:

- `target/arcweft-native-capture-artifacts/vertical_tutr_golden.candidate.png`
- `target/arcweft-native-capture-artifacts/vertical_tutr_golden.observe.json`
- `target/arcweft-native-capture-artifacts/vertical_tutr_golden.imq.json`
- `target/arcweft-native-capture-artifacts/exact-native-golden.environment.json`

The absence is the defer blocker, not an implicit rejection of the renderer.
