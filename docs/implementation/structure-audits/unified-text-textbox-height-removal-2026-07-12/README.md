# Unified text: Agent TextBox-height removal structural audit

Audit scope: Jujutsu change `xxwwmruw`, which removes the unconsumed Agent
`textbox_height` request field from the CLI and MCP tool schemas and removes its
stale tests and command plumbing.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-textbox-height-removal-2026-07-12
```

The audit scans 2,635 files, including 1,244 Rust files / 613,514 physical Rust
LOC and 91 package manifests. It reports 0 errors and 128 pre-existing tracked
warnings. No Cargo manifest, workspace dependency edge, public Rust boundary
type, renderer contract, or persistence format changed.

## Changed-file measurements

| Path | Bytes | Physical LOC | Classification and responsibility |
| --- | ---: | ---: | --- |
| `crates/arcweft-agent-mcp/src/tools.rs` | 45,734 | 810 | production; typed MCP tool-schema inventory |
| `crates/arcweft-agent-mcp/src/tests.rs` | 44,302 | 1,237 | crate test module; schema and resource behavior |
| `crates/arcweft-cli/src/app/agent.rs` | 22,907 | 707 | production; Agent command-line surface |
| `crates/arcweft-cli/src/app/agent/native/observe.rs` | 48,927 | 1,375 | production; native observation orchestration and input validation |
| `crates/arcweft-cli/tests/check/agent_observe_native/shared.rs` | 44,659 | 1,231 | integration-test support; shared native Agent command helpers |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_units.rs` | 143,206 | 4,177 | integration tests; published JLREQ unit cases |
| `crates/arcweft-cli/tests/check/agent_observe_native/published_jlreq_class_mix.rs` | 220,473 | 6,109 | integration tests; published JLREQ class-pair matrix |

The remaining five edited native Agent files are each one-field plumbing
removals and remain below the production warning threshold. The two large
JLREQ files were already warning-level integration-test inventories; this cut
only removes the obsolete positional helper argument and does not add another
responsibility. `observe.rs` remains a tracked warning-level orchestration
module, but this cut shrinks it and does not mix in layout behavior. The MCP
schema test proves the removed option is absent from the public generated
schema rather than checking implementation source text.
