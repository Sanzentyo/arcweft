# AW-AH-009.3.2 accepted HIR and request-lifecycle production reconciliation

`STATUS=READY_FOR_IMPLEMENTATION`

This archive is the decision-complete production contract for AW-AH-009.3.2. It fixes the one accepted-generation artifact, exact URI/source/module/HIR acquisition route, request stamp, cancellation and deadline owner, cache publication gate, invalidation hooks, build limits, and retained-generation ownership needed by AW-AH-009.3.

The governing request prohibits production changes in this delivery. Accordingly, this archive contains no patch, checkout, generated build output, compatibility layer, or claimed production test run. It contains the exact Rust ownership and API contract that the subsequent implementation cut shall apply.

## Inspected basis

- Request audit basis: Git `328e362f811896ebf866002c458fe0b970976654`, Jujutsu `wopypppm`.
- Current inspected Arcweft `main`: Git `8984661d5679efccf7a16255f921530cd0b7cacc`. GitHub reports this revision is two commits ahead of the request basis and contains it as the merge base.
- Original AW-AH-009.3 final contract SHA-256: `cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5`.
- Root `AGENTS.md`, the supplied Rust Skill, the request, current production owners/callers, and the current AW-AH-009.3 production audit were inspected before this contract was frozen.
- The predecessor ZIP byte stream was not mounted in this execution. Its supplied SHA sidecar/status/summary and the repository's recorded package audit were available; this delivery verifies that recorded identity, not a fresh predecessor unzip.

## Frozen result

1. `arcweft-lsp::profiles::accepted_project::AcceptedProjectSnapshot` is the one immutable accepted HIR/source/module carrier. It owns one `Arc<HirProject>`, the accepted source registry, and the typed source-to-module reverse index. It is retained by `AcceptedProfileEnvironment`, not by `RegisteredSemanticWorld`.
2. The exact `HirProject` assembled during profile rebuild is borrowed by character registration and then retained by the accepted snapshot. Signature help does not parse, lower, link, or construct a second project.
3. Open overlays are accepted transactionally. Changed bytes are unqueryable until a matching generation publishes. Identical bytes with a new LSP version use a metadata-only generation replacement and reuse the exact world/project Arcs without parsing or lowering.
4. One `AcceptedDocumentHirLease` supplies the exact accepted `&SourceDocument`, canonical `CanonicalModulePath`, document-bound `&HirModule`, and current `&RegisteredSemanticWorld`.
5. One server-owned `RequestControl` owns the `AtomicBool` borrowed by the sema query. Admission uses a fixed 250 ms deadline, four workers, and a global maximum of 32 admitted signature requests.
6. Cache return and insertion use the same final stamp validator and request publication gate. Replacement, remapping, close, workspace removal, shutdown, cancellation, or deadline expiry cannot redirect an old computation into a new cache or allow a stale insert into the old cache.
7. Accepted-build input is bounded before LSP-path parsing/lowering by 4,096 unique documents, 8,388,608 aggregate UTF-8 bytes, and the existing 262,144 project-symbol work limit. Accepted HIR construction is outside the per-query budget and is never hidden behind a cache miss.

## Reading order

1. `FINAL_STATUS.md`
2. `FINAL_CONTRACT.md`
3. `PRODUCTION_RECONCILIATION.md`
4. `IMPLEMENTATION_HANDOFF.md`
5. `TEST_MATRIX.md`
6. `REQUIREMENTS_TRACEABILITY.md`
7. `REPOSITORY_EVIDENCE.md`
8. `OPEN_QUESTIONS.md`
9. `MANIFEST.txt`

## Verification boundary

The archive member set, sorted manifest, every member SHA-256, the zero self-entry, exact `OPEN_QUESTIONS.md` bytes, ZIP structure, and outside ZIP SHA-256 are verified by the packaging step. The predecessor archive itself was not re-unzipped because its byte stream was not mounted; only its supplied digest/status/summary and repository-recorded audit were inspected. Rust compilation and production integration are intentionally not claimed because this is the production-prohibited contract stage. Exact commands required of the implementation assignee are frozen in `IMPLEMENTATION_HANDOFF.md`.
