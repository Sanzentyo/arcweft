# Follow-up Request: AWFR adapter wiring and release verifier

## Context

The seq-02.6 / seq-02.7 boundary package introduced Sans I/O AWFR archive, external payload carrier, and typed signing policy models. It intentionally left key access, clocks, network fetch, remote publication, and platform trust stores in adapters.

## Required follow-up

1. Implement HTTP(S) external payload fetch with the same network-policy boundaries as release bundle fetching.
2. Implement a release-publish adapter that stages AWFB, patch, external payload, AWFR, and signatures atomically.
3. Implement a release-consume verifier that returns typed signing/payload states without duplicating policy logic in players.
4. Thread external payload materialization mode through patch target materialization adapters.
5. Extend CLI smoke beyond the implemented `cache fetch-external` path to cover release publish, release verify, and target signing flows.

## Acceptance tests

- AWFR archive deterministic bytes remain stable after round-trip.
- External payload cache key changes when epoch, bundle root, descriptor id, or compressed digest changes.
- Local file and cache external payload fetch succeed.
- Digest mismatch and size mismatch fail before cache record publication.
- Changed target bytes require target signature under release policy.
- Local-dev policy allows unsigned target only with explicit `UnsignedAllowed` state.
- Offline inspection does not fetch payload bytes unless policy demands them.
- CLI smoke covers release fetch, external payload fetch, sign bundle, and release verify.
