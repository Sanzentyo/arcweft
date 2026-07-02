# Seq04.8.3 bytecode/link producer identity closure

Date: 2026-07-01
Base evidence: seq04.8.1 actual reuse payloads and full-build actual builder integration evidence in `crates/arcweft-cli/src/app/project_commands.rs`.

## Summary

This implementation closes producer identity gaps for bytecode/link reuse by making every producer family explicit and typed.  Ready full-build bytecode/link records remain actual reusable records.  Producers without complete identity either keep a conservative typed reason or skip bytecode/link record writing until identity is available.

## Producer matrix

| Family | Classification | Record behavior |
| --- | --- | --- |
| `full_build` single-module/single-unit | `actual_reusable_ready` | `VerifiedReusable`, read-through `Hit` |
| `full_build` multi-module/multi-unit | `conservative_required` | `ConservativeRebuild`, reason `full_build_multi_module_product_awbc_not_narrowed` |
| `full_build_watch` single-module/single-unit | `actual_reusable_ready` | Same as full build |
| `full_build_watch` multi-module/multi-unit | `conservative_required` | Same conservative reason as full build |
| `direct_bundle` | `actual_reusable_after_identity_work` | No bytecode/link persistent record until `DirectBundleProducerIdentity` exists |
| `single_source_compile` | `not_a_bytecode_link_producer` | No bytecode/link record |
| `patch_bundle` | `conservative_required` for future target records; patch bytes are not bytecode/link records | No bytecode/link record until target product identity exists |
| `agent_script` | `actual_reusable_after_identity_work` | No bytecode/link record until `AgentScriptProducerIdentity` exists |
| `runtime_driver` | `not_a_bytecode_link_producer` | Consumer only |
| `fixture_regeneration` | `conservative_required` outside fixtures | Fixture-only output; no production `VerifiedReusable` |
| `persistent_cache_test_builder` | `actual_reusable_ready` in tests only | Synthetic actual payloads allowed only under `#[cfg(test)]`/fixtures |

## Conservative reasons

Typed reasons are defined in `BytecodeLinkConservativeReason`.  Each reason owns:

- a stable policy string;
- exact missing identity;
- exact consumer boundary;
- follow-up sequence number when applicable.

## Validation plan

```bash
cargo fmt --all -- --check
cargo test -p arcweft-project persistent_object --all-features
cargo test -p arcweft-project-loader persistent_query --all-features
cargo test -p arcweft-cli cache --all-features
cargo check -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-bundle -p arcweft-cli --all-targets --all-features
cargo clippy -p arcweft-project -p arcweft-project-loader -p arcweft-compiler -p arcweft-bundle -p arcweft-cli --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root .
git diff --check
```

## Remaining producer-specific work

- `seq04.8.3.1`: direct bundle producer identity.
- `seq04.8.3.2`: full-build multi-module per-unit AWBC identity.
- `seq04.8.3.3`: agent-script producer identity.
- `seq04.8.3.4`: patch target materialization product identity.
