# Verification record

## 1. Verification scope summary

| Artifact/assertion | Verification performed | Result/confidence |
|---|---|---|
| Request/premise/Rust Skill ingestion | each local input was opened and consumed through EOF; byte count, line count, final non-empty line, and SHA-256 recorded below | high for completeness of read |
| Latest `AGENTS.md` | every `AGENTS.md` found in the materialized repository was read through EOF and hashed | not locally verifiable because repository clone was unavailable |
| Current private repository | authenticated clone/fetch attempt; exact HEAD/status recorded when materialized | not verified locally; design decisions remain complete but source anchors are unavailable |
| Source evidence | mechanical UTF-8 scan of Rust/TOML/Markdown at exact HEAD; line anchors recorded | not performed |
| Design consistency | traceability matrix, owner model, API, persistence grammar, crash matrix, concurrency, implementation plan, tests, rollout cross-checked by package generator/review pass | high for internal contract consistency; implementation not compiled |
| Production patch | none created or applied | verified by package contents; repo status captured when available |
| Rust compilation/tests | no production implementation exists in this package, so compile/test gates were not run against a patch | intentionally not applicable; commands are specified as implementation gates |
| ZIP integrity | performed after packaging with `zip -T`/`unzip -t` and manifest/hash generation | see `ZIP-VERIFICATION.txt` at package root |

## 2. Full-input EOF evidence

| Input | Bytes | Lines | SHA-256 | Last non-empty line (evidence of tail read) | Fully consumed |
|---|---:|---:|---|---|---|
| `request.md` | 9622 | 185 | `62654cfcfadb1359523c3dba2ba97663f813cba3d3a8930fedd22ed660f0ba68` | `and `OPEN_QUESTIONS` is exactly `none`.` | yes |
| `premise.txt` | 250 | 1 | `cfa897a0ad93deb92fd454079df0a789edbbd40d85c8377324da703c8aefe0a1` | `このプロジェクトでは、Sanzentyo/arcweft に関連した質問が為されます。明示されない限りは、必ず最初にarcweftの理念や構造などについて、最新のものを把握・分析するところから始めます` | yes |
| `rust-skill.txt` | 5045 | 57 | `1a28f552adf5efde95205bee8d56590aeb82346c48ebdf3fdbbaff5deca33665` | `- cpalクレートを用いる場合は、 `#[derive(Parser)]` を用いてコマンドライン引数のパーサーを定義すること。サブコマンドが必要な場合は、 `#[derive(Subcommand)]` を用いてサブコマンドの列挙型を定義すること。shortについてもユーザーの利便性を考慮して適切に指定すること。` | yes |

## 3. Repository identity and `AGENTS.md`

- Repository: `Sanzentyo/arcweft`
- Exact inspected Git SHA: `UNAVAILABLE (authenticated clone was not available in the execution container)`
- Checkout state: `UNAVAILABLE`
- `AGENTS.md` rows are listed in `02-current-source-evidence.md` with full-file hashes.

## 4. Captured execution logs

| Log | Content | Meaning |
|---|---|---|
| `clone-method.txt` | `FAILED` | captured execution evidence |
| `capabilities.txt` | `date=2026-08-22T14:33:25Z git=/usr/bin/git gh=MISSING codex=MISSING claude=MISSING aider=MISSING opencode=MISSING` | captured execution evidence |
| `agent-used.txt` | `NONE` | captured execution evidence |
| `reviewer-used.txt` | `NONE` | captured execution evidence |

## 5. Static package checks

The packaging script performs these checks and records results outside then inside the ZIP:

1. all required Markdown files and `MANIFEST.txt` exist and are non-empty;
2. `OPEN_QUESTIONS=0` appears in the package index/coverage;
3. the exact repository SHA or explicit unavailability marker appears in evidence/verification;
4. every detected numbered request item appears in `01-request-coverage.md`;
5. crash rows CP-00..CP-11 and corresponding test rows exist;
6. no file under the repository working tree is copied into the production patch because no patch is included;
7. file SHA-256 manifest is generated;
8. archive central directory/content test passes.

## 6. What is not claimed

- This design-only archive does not claim that proposed Rust signatures compile unchanged against current source.
- It does not claim runtime tests passed, because no implementation was produced.
- A mechanical source anchor is not represented as a semantic proof beyond its line/symbol.
- When the private repository could not be materialized, the package says so rather than fabricating a SHA or source fact.

## 7. Verification confidence by deliverable

| Deliverable | Confidence | Basis |
|---|---|---|
| ownership/two-phase protocol | high | closed invariants and ordering/crash analysis |
| persisted logical grammar | high as normative design; medium for exact integration names | canonical constraints fixed; actual codec API must be wired to current owner |
| Rust API shape | high as contract; medium for exact imports | concrete ownership/signatures; no compile run |
| current-source placement | low/unverified | fallback paths only |
| test matrix | high as coverage design | deterministic actions/assertions specified |
| production readiness | design-ready, not implementation-verified | implementation and workspace gates remain future work |
