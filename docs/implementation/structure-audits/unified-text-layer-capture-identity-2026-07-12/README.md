# Unified text: layer capture identity structural audit

Audit scope: Jujutsu change `vkwpxwkn`, which separates direct selected objects
from descendant pixel coverage for native Agent captures and makes layer
object-ID pixels and metadata use the same identity boundary.

Canonical command:

```bash
cargo +nightly -Zscript tools/structure-audit.rs --root . \
  --write docs/implementation/structure-audits/unified-text-layer-capture-identity-2026-07-12
```

The audit scans 2,637 files and records 1,244 Rust files / 613,593 physical Rust LOC, 91 package
manifests, 0 errors, and 128 tracked warnings. No Cargo manifest, dependency
edge, serialized protocol shape, or public Rust boundary type changed.

## Changed-file measurements

| Path | Bytes | Physical LOC | Classification and responsibility |
| --- | ---: | ---: | --- |
| `crates/arcweft-cli/src/app/agent/native/capture.rs` | 33,571 | 985 | production; retained-frame scope selection, attachment masking, and selected-capture metadata |
| `crates/arcweft-cli/src/app/agent/native/runtime_observation.rs` | 25,976 | 646 | production; native observation output records |
| `crates/arcweft-cli/src/app/agent/native/tests.rs` | 110,745 | 3,181 | crate test module; native Agent behavior and capture contracts |
| `crates/arcweft-cli/tests/check/agent_observe_native/visual_smoke.rs` | 13,400 | 360 | integration tests; real shared-renderer capture pixels and metadata |

The capture implementation remains below the 1,200-LOC production warning
threshold. The test module is a warning-level test inventory; this cut adds one
focused behavioral case and does not add production responsibility to it. The
new internal selection record names the two concepts already required by the
capture algorithm: direct scope roots determine published identity, while
expanded descendants determine which retained object-ID pixels belong to the
selection.
