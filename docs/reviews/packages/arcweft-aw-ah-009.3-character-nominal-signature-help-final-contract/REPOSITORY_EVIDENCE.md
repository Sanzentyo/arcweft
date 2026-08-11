# Repository evidence

## 1. Repository identity

- Repository: `Sanzentyo/arcweft` (private; inspected through the GitHub
  connector)
- Current main Git commit: `76d39983ad8770a87d6e81745785b6b362a381b4`
- Commit subject: `Refresh independent design requests and dispatch order`
- Request-recorded basis: `5a36cd0af83085179c299ef50ec8aa786ed731aa`
- Current main is one documentation-only commit ahead of that basis; production
  Rust evidence inspected below remains the current implementation substrate.
- Current Jujutsu change: `unavailable` because the GitHub connector does not
  expose the checkout's Jujutsu working-copy identity. The request-recorded
  `nowqxzkuptorvlnoltqxstxmqtvuwrkr` belongs to the earlier basis and is not
  reported as current.

## 2. Governing instructions inspected

| Input | Inspection | Applicable conclusions |
| --- | --- | --- |
| root `AGENTS.md` blob `c41ff4d2b3baadda3e9f975c7de3e5a6678f8758` | read through the final line | preserve syntax -> HIR -> sema -> tooling direction; prefer typed APIs; add behavior to owning Arcweft enums/types; no compatibility shims; no automated source gates; run direct tests and structural audit |
| uploaded `Rust Skill.txt` | all 56 lines read | stable Rust; careful visibility; typed newtypes; no unsafe/macros/unstable additions; use Clippy and rustfmt; avoid unnecessary compatibility and ad hoc code |
| uploaded AW-AH-009.3 request | all 392 lines read | design-only implementation outcome; exact ZIP/sidecars; one resolver; exact identities, recovery, cache, limits, errors, tests, and no open questions |

## 3. Required implementation notes inspected

| Path | Relevant evidence |
| --- | --- |
| `docs/implementation/2026-07-16-aw-ah-009-1-1-character-nominal-registration-production-reconciliation.md` | `RegisteredSemanticWorld` and accepted-generation publication are the canonical atomic semantic authority; failed rebuild preserves the previous accepted world |
| `docs/implementation/2026-07-14-aw-ah-009-character-nominal-type-identity.md` | `CharacterNominalType` owns structural family/character/part identity; display labels and aliases are not identity and have no inverse parser |
| `docs/implementation/2026-07-15-proof-concurrency-v6.1-surface-hir-identity.md` | `SourceSnapshotId` is a separate session-local syntax lineage; current stable typed AST attachment is incomplete |
| `docs/reviews/requests/2026-07-16-seq-proof-01.1.1-typed-ast-syntax-identity-proof-block-reconciliation.md` | proof 01.1.1 owns future typed AST to `SyntaxNodeId` attachment; AW-AH-009.3 may avoid that dependency when exact document/range identity is sufficient |

## 4. Production source evidence

| Path / blob | Observation | Contract consequence |
| --- | --- | --- |
| `crates/arcweft-lang-sema/src/types/character_nominal.rs` | structural character nominal family/owner/part type and source label already exist | reuse exact identity; no label parser or replacement type |
| `crates/arcweft-lang-sema/src/types.rs` blob `0ef388137934ff34c95147dc9a44dada08ad9ff1` | `TypeKind::CharacterNominal` exists; no dedicated unknown/any variant | result distinguishes `Known`, `Unconstrained`, and typed `Unavailable` instead of forging `Named("_")` |
| `crates/arcweft-lang-sema/src/env/base.rs` blob `1feb4981996d201d55548d25538d829c3e5c6350` | typed `FunctionSignature`, params, curried groups, functions, methods, Rust packages exist | normalize every accepted callable into one typed candidate catalog |
| `crates/arcweft-lang-sema/src/registration/model.rs` blob `65ae261d8941ef9e0f6889466ce16e1b2002f9d7` | registered world contains symbol table/environment; environment records world, symbol revision, character digest/revision and character variants | cache and stale checks use existing typed values, not strings |
| `crates/arcweft-lang-sema/src/checker/expr.rs` blob `4da716f2be073ae9604ac257a019fc282a18d54c` | call dispatch is currently FX -> enum -> builtin -> Agent -> presentation -> named/path/selected -> speaker/function value | extract this order into one shared sema resolver; do not add an LSP resolver |
| `crates/arcweft-lang-sema/src/checker/presentation.rs` blob `27efaa81a80e7becf2de4cddd52516d478b7d06b` | current special names are `view`, `menu`, `overlay`, `bg`, `image`, `player_viewport`, `show`, `ref.bg`, `ref.show`, `clear.bg`, and `hide`; `show` discovers `look` but calls ordinary `check_expr` | `show.look` is an applicable dynamic nominal parameter, and checker/query must share a typed schema |
| `crates/arcweft-lang-sema/src/checker/module.rs` blob `56d7ab5cbc97820662e73f3f680c45dcdad41bf8` | dialogue `look` is preserved and checked as an ordinary expression; project source functions are normalized into typed signatures | dialogue is applicable; expected nominal checking is a real production gap |
| `crates/arcweft-lang-syntax/src/expr.rs` and `expr/pratt.rs` | `Expr::Call` currently retains semantic args but not exact nested call/argument/separator ranges | parser must retain ranges while parsing; no post-parse source search |
| `crates/arcweft-lang-syntax/src/expr/source_ranges.rs` | existing best-effort range recovery splits/searches authored text | this helper is not an acceptable authority for signature help |
| `crates/arcweft-lang-syntax/src/ast/dialogue.rs` | `SpeakerLine`, `SpeakerLineSurface`, `LineOptions`, `ContentCall`, and named `look` are typed surface records | add generic `ArgumentListSyntax` to their owning range records |
| `crates/arcweft-lang-hir/src/model.rs` blob `1871bf3637043bf5f41c3c4a57927362a9f123ef` | HIR keeps dialogue fields and exact source-document binding; `source_span` is revision-bound | expose source identity/module path and preserve call ranges |
| `crates/arcweft-lang-hir/src/lower.rs` blob `d6f3125fa7e303840e301eaff2813e4c21a71c76` | `lower_document_to_hir` rejects a typed tree whose source differs from the document | exact document/range branch is safe without snapshot/node identity |
| `crates/arcweft-lang-hir/src/symbol/table.rs` | canonical project symbol table/revision exists | add module source identity lookup to the owning table and use it for stale checks |
| `crates/arcweft-lsp/src/features/signature.rs` | current handler extracts one word at cursor and delegates only to Rust adapter metadata | replace it completely and delete word fallback |
| `crates/arcweft-verify-lsp/src/lib.rs` | current adapter helper selects Rust signature metadata by name and constructs active 0/0 | delete resolver/result construction after migration |
| `crates/arcweft-lsp/src/documents.rs` | `DocumentSnapshot` retains URI, LSP version, exact `SourceDocument`, and `LineIndex` | form exact request stamp from one snapshot |
| `crates/arcweft-lsp/src/positions.rs` | current line index maps negotiated positions, with existing non-signature behavior | add a checked inherent conversion for signature requests; do not change other features |
| `crates/arcweft-lsp/src/profiles/cache.rs` blob `23375f80ad0aecabd5e4dc0df7eec6d8abe810e6` | accepted environment has monotonic generation, immutable world, fresh per-generation cache namespace, and shutdown clearing; current cache is placeholder strings | replace placeholder with one typed bounded cache |
| `crates/arcweft-lsp/src/session.rs` blob `c12a9316ef39d12443ff09816917d321f458119c` | signature requests currently return an `Option`; close removes document/profile and shuts profile state | add typed request error mapping and cache invalidation |
| `crates/arcweft-adapter-context/src/manifest.rs` | adapter metadata already applies typed functions/methods to semantic environment | retain typed provenance/docs and produce `EnvironmentCallableId` during the same path |
| `crates/arcweft-adapter-context/Cargo.toml` blob `ec752ae3728838c33c622d6973fb534b0c8e6df4` | sema/HIR/syntax/source dependencies already exist behind the sema feature | no new dependency required |
| `crates/arcweft-lsp/Cargo.toml` blob `df74a13cd0bc39fb91522b7fe4e8ca96cf3a9e5e` | LSP already depends on adapter-context, HIR, sema, syntax, source, and verify-lsp | implementation can migrate without adding a dependency |
| `crates/arcweft-lang-sema/Cargo.toml` blob `d49fb75eb88601efff15bd9b01329b0074b9cbd8` | sema already depends on character, HIR, syntax, source, and `thiserror` | public query/errors fit current dependency direction |

## 5. Request-to-current-main discrepancy

The request's “current evidence” says the checker assigns a structural
`CharacterNominalType` expectation to presentation look arguments. The inspected
current main does not do so: `check_character_look_arg` calls `check_expr`, and
dialogue `look` also calls `check_expr`. This is not grounds for an explicit
non-goal because the owner and argument surfaces are real and registered
character identity is available. It is the reason the final design requires a
shared typed presentation/dialogue schema used by checker and query.

## 6. Verification performed for this delivery

Performed:

- full input and policy reading;
- current private-repository source inspection through the connector;
- current Git identity and one-commit comparison with the request basis;
- cross-file API/ownership/dependency reconciliation;
- archive required-membership check;
- UTF-8/LF and forbidden placeholder scan;
- `OPEN_QUESTIONS.md` exact-byte check;
- manifest digest recomputation;
- ZIP central-directory/integrity test;
- external ZIP SHA-256 generation and verification.

Not performed and not claimed:

- production Rust edits;
- Cargo build, Clippy, rustfmt, or runtime tests against a local checkout;
- Jujutsu working-copy inspection;
- implementation benchmark or production cache measurement.

Those commands belong to the later production implementation and are listed in
`IMPLEMENTATION_HANDOFF.md`. No fabricated implementation log is included.
