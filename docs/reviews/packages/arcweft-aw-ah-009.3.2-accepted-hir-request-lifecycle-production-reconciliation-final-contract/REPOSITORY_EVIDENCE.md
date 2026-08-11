# Repository evidence

## 1. Evidence boundary

Sanzentyo/arcweft is private. Repository evidence was read through the configured GitHub connector at exact Git objects. The supplied request, Rust Skill, and project precondition were read from the provided files. The current checkout was not modified and no Rust production command was executed for this design-only delivery.

Request basis is Git `328e362f811896ebf866002c458fe0b970976654`, Jujutsu `wopypppm`. Current inspected `main` is `8984661d5679efccf7a16255f921530cd0b7cacc`. GitHub comparison reports `main` is `ahead` by 2 and `behind` by 0, with the request basis as merge base.

The original final package identity is:

```text
arcweft-aw-ah-009.3-character-nominal-signature-help-final-contract.zip
sha256 cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5
```

The predecessor ZIP byte stream was not mounted in this execution, so it was not freshly extracted or CRC-tested here. Its supplied SHA-256 sidecar, status file, summary, and the current repository audit were available and inspected. That audit records valid predecessor membership, manifest hashes, zero self-entry, and exact `OPEN_QUESTIONS.md`. This contract therefore verifies the recorded predecessor identity/integrity evidence, not a new unzip, and does not claim to have modified or repackaged it.

## 2. Policy evidence

| Evidence | Exact object | Relevant conclusion |
| --- | --- | --- |
| root `AGENTS.md` | blob `c41ff4d2b3baadda3e9f975c7de3e5a6678f8758` at current main | typed owners/newtypes, direct inherent implementation on repository-owned types/enums, no ad hoc extension traits/string gates/compatibility shims, small compiling cuts, fmt/Clippy/tests/structural audit |
| supplied Rust Skill | `/mnt/data/Rust Skill.txt`, 56 lines, read through end | standard Rust APIs, careful visibility, newtypes, no unsafe/leak/forget, fmt and Clippy, no unstable feature without approval |
| supplied project precondition | `/mnt/data/前提(Sanzentyo-arcweft).txt` | inspect current Arcweft philosophy and structure before design |
| AW-AH-009.3.2 request | supplied 182-line Markdown | production changes prohibited; exact carrier/lifecycle/test/output requirements |

## 3. Production source evidence

| Repository path | Blob SHA | Observed production fact used by the contract |
| --- | --- | --- |
| `crates/arcweft-launch/src/model.rs` | `84b6e05d18c21644b49f8cc825f3522094423163` | existing typed `ProfileId` owns launch-profile identity; it is reused rather than keeping an LSP profile string key |
| `crates/arcweft-lsp/src/profiles/cache.rs` | `de4bbb6ec0b1d57f5969f58ff18ea82bea80b159` | accepted environment owns generation/profile/world/sources/overlays/caches; source registry has identity/URI indices; no HIR; profile replacement is Arc-based and old readers are memory-safe |
| `crates/arcweft-lsp/src/profiles/environment.rs` | `5b40a1cc5747987b1b5425dbb12bc8f5e341e9f8` | overlay-selected documents run `parse_source` and `lower_document_to_hir`; one `HirProject` is already assembled, borrowed for registration, then not retained |
| `crates/arcweft-lang-hir/src/project.rs` | `4db63fe28d279ceea3e3ee9a75e80e4b7c460221` | canonical module -> HIR/source map exists; duplicate modules/root invariant; current module constructor panics on source mismatch and needs typed replacement |
| `crates/arcweft-lang-hir/src/model.rs` | `2560ccdab9c610000e3c2d6c2fd4cb3ac18a3a6d` | `HirModule` retains `HirSourceMap` document and exposes source identity/module path but not document accessor |
| `crates/arcweft-source/src/document.rs` | `218a9552bfb6fd04268f57e79eeb67aed133e895` | exact source revision is BLAKE3 over UTF-8; identity includes ID/revision/length; source text and identity are Arc-backed; source-byte limit is exactly 8,388,608 |
| `crates/arcweft-lang-hir/src/symbol/table.rs` | `2c038443967f5a9b5585889d4a76321540e7aca9` | symbol table provides canonical module -> source identity, including declaration-free modules; no reverse source/URI authority; work limit is 262,144 and diagnostics 128 |
| `crates/arcweft-lang-sema/src/registration/model.rs` | `ed098b5bec07713217b2611b0ec638163e46fca0` | registered world owns symbols/environment/character index; environment exposes world, symbol revision, character digest/revision; registrar borrows `&HirProject` |
| `crates/arcweft-lang-sema/src/registration/limits.rs` | `17da9a5c23816b8ae24ae27e3c7d5a2846f0598f` | production document limit is 4,096 and source bytes reuse `MAX_REGISTRATION_SOURCE_BYTES` |
| `crates/arcweft-lsp/src/documents.rs` | `ad2675270a124bdd38819ac760454abc9a12970a` | live snapshot owns URI/version/document/line index; store and overlay errors use String URI authority; rebind can bind live bytes to accepted logical ID |
| `crates/arcweft-lsp/src/session.rs` | `d11d020a5b819c79897cd7b6317b854fb7570c36` | session owns document/profile maps and a cancelled-ID set; signature dispatch receives only live snapshot/profile; close/shutdown/rebuild are current lifecycle seams |
| `crates/arcweft-lsp/src/server.rs` | `7707294e29aa3e0d7bf85b8b260ba91c5afb0e93` | request handling is synchronous on message-intake loop, so in-progress cancellation cannot be delivered |
| `crates/arcweft-lsp/src/features/signature.rs` | `25904224e93ae20fe02933dbd2f65fb5fa9c8ec3` | current feature uses word-at-position and Rust adapter fallback; deletion must wait for full sema path |
| `crates/arcweft-lsp/src/features/character_definition.rs` | `f3c396557e191bf0d846276c7e24fec3a1b45fca` | current caller imports the accepted cache module and uses accepted source lookup; module split/typed URI migration must update it mechanically without choosing signature syntax |
| `crates/arcweft-project-loader/src/project.rs` | `9857b5991779ccce482e9bfb66398facc82c6756` | typed project source/module inventory exists; current full file reads and parsing need a pre-parse aggregate LSP limit |
| `crates/arcweft-project/src/sources.rs` | `1cb89faca8c9810979dfdccfc071319339e18bfa` | canonical typed module map and duplicate-module rejection already exist; source hash is exact BLAKE3 |
| `crates/arcweft-lang-syntax/src/parser.rs` | `c3a05346e10ca6c25a42d364434a2d9e961aaa13` | `parse_source` is a whole-source compiler entry point and must remain in build, not signature feature/cache miss |
| `docs/implementation/2026-07-16-aw-ah-009-3-production-reconciliation-audit.md` | `fd2b2a2d65de13e008c8ac9016f320c99597da70` | current audit explicitly identifies accepted HIR acquisition and live request cancellation as AW-AH-009.3.2 blockers and prohibits an implicit fallback |

## 4. Source-reported validation evidence

The current repository audit records successful focused fmt/check/Clippy/tests and a canonical structural audit for the already-landed substrate at its stated production checkout. It also records the untracked test-font limitation encountered in an independent Jujutsu workspace and the later successful root-checkout commands with the asset present.

Those are repository-recorded historical facts, not commands rerun by this package. The future implementation must run the exact command set in `IMPLEMENTATION_HANDOFF.md` at its own final revision and preserve raw exit/status evidence.

## 5. Design deductions grounded in the evidence

1. Retaining the already-built `HirProject` in LSP is the only selected route that avoids a second parser/project and avoids putting URI/version policy into sema.
2. Because source documents are immutable and Arc-backed, retaining the project/source registry by Arc gives safe old readers without copying HIR or UTF-8 buffers for each request.
3. Forward module-to-source evidence is already canonical; one validated reverse map in the accepted snapshot is sufficient and does not invent module identity.
4. Transactional profile publication is already the correct atomic boundary; HIR must join that candidate rather than appear as a lazy cache.
5. Current synchronous dispatch and cancelled-ID set cannot satisfy during-work cancellation; an active request object plus bounded worker dispatch is required.
6. Existing document/source/symbol production limits provide the correct non-signature-specific build authorities once enforced before LSP parse/lower.

No deduction depends on source spelling tests or an unverified external API.
