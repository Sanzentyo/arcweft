# persistent-cache-typecheck fixture

This fixture documents the expected typecheck-gate cache behavior. It is a small
fixture for focused persistent query tests rather than a complete AWFB product
fixture.

Expected cache explain shape for a valid gate hit:

- `query = type-check`
- `payload_kind = TypecheckGate`
- `persistent status = Hit`
- `cache record status = hit_then_rebuilt`
- `typecheck_gate_reuse_policy = conservative_rebuild`
- `recovery = RebuildFromSource`

Expected soft miss after changing `dep.arcw` public interface:

- `query = type-check`
- `status = Miss`
- `soft_miss_reason.kind = dependency_interface_digest_mismatch`
- snapshot invalidation reason includes `DependencyInterfaceChanged { module: "dep" }` when the changed dependency can be named.
