# AW-AH-009.3 production reconciliation audit

## Status

The goal remains open. This audit was performed at Git commit
`328e362f811896ebf866002c458fe0b970976654`, Jujutsu change `wopypppm`, against
`arcweft-aw-ah-009.3-character-nominal-signature-help-final-contract.zip`
(SHA-256
`cdd1d7b764da238a6e4e8f3e774a3384017c8da5ffaea1969f2af279102a7cd5`).

The archive still has 11 exact members, 11 valid manifest entries, sorted
membership, matching SHA-256 values, the required zero self-entry, and
`OPEN_QUESTIONS.md` containing exactly `none\n`.

No additional Rust slice is both independently end-to-end and faithful to the
accepted contract after the already-landed checked-position and source-identity
substrate. The remaining production path crosses three unselected boundaries.
Implementing through any one of them now would require a fake authored range,
an unaccepted per-request HIR build, an incomplete resolver that drops current
method families, or a transitional compatibility path. Those are prohibited by
the package and repository policy.

## Requirement-by-requirement evidence

| Contract area | Current production evidence | Status |
| --- | --- | --- |
| exact UTF-8/UTF-16 LSP position conversion | `LineIndex::try_byte_offset_from_position` and direct scalar/bounds tests are landed | implemented |
| HIR document identity and canonical module path | `HirModule::source_identity` and `HirModule::module_path` are landed through document-bound lowering | implemented |
| project module source identity | `ProjectSymbolTable::source_identity` records every linked module, including declaration-free modules | implemented |
| exact parenthesized call/argument/recovery ranges | semantic `Expr::Call` still also represents postfix callback blocks and public source-less construction | blocked by [AW-AH-009.3.1](../reviews/requests/2026-07-16-aw-ah-009.3.1-call-surface-syntax-production-reconciliation.md) |
| accepted document/HIR tuple for the sema request | accepted generations retain source documents and semantic world but no document-bound HIR registry or URI-to-HIR authority | blocked by [AW-AH-009.3.2](../reviews/requests/2026-07-16-aw-ah-009.3.2-accepted-hir-request-lifecycle-production-reconciliation.md) |
| complete candidate IDs/result constructors | several required opaque IDs, `SignatureOrigin`, resolver records, and invariant constructors have no selected Rust shape | blocked by [AW-AH-009.3.3](../reviews/requests/2026-07-16-aw-ah-009.3.3-callable-catalog-shared-resolver-production-reconciliation.md) |
| one resolver shared by checker and query | current checker has many free-call and method families absent from the package's concrete resolver product | blocked by AW-AH-009.3.3 |
| structural `show.look` and dialogue `look` checking | current checker still calls ordinary expression checking; the final shared schema API and owner-resolution input are not selected | blocked by AW-AH-009.3.3 |
| project/environment/adapter callable catalog | `RegisteredTypeCheckEnv` retains the accepted base environment but not the required ordered typed candidate records/provenance | blocked by AW-AH-009.3.3 |
| accepted-generation request stamp, cancellation, and cache | key/value policy is specified, but accepted HIR acquisition and the live cancellation owner are not | blocked by AW-AH-009.3.2 and AW-AH-009.3.3 result types |
| LSP formatting and error mapping | deterministic label policy is specified, but there is no semantic result to format safely | dependent on all three reconciliations |
| delete word-only Rust fallback | `arcweft-lsp::features::signature` still delegates to `arcweft_verify_lsp`; deletion before the sema path lands would remove the only current feature | correctly deferred until migration |
| full direct test matrix and completion validation | only the decided substrate suites have run; the package-wide matrix is not implemented | open |

## Concrete production contradictions

### Authored call identity

`Expr::Call { callee, args }` is produced both by ordinary `(...)` parsing and
postfix callback-block sugar. `Expr::call` and `Expr::selected_call` are public
and can construct the same variant without source. A mandatory
`ArgumentListSyntax` therefore cannot be added without first selecting the
final authored-vs-generated model. The structured proof shadow parser does not
change this semantic AST fact.

### Accepted HIR acquisition

The public sema request requires `&HirModule`, but
`AcceptedProfileEnvironment`, `AcceptedSourceDocument`, and `DocumentSnapshot`
do not retain it. Parsing/lowering inside signature help, adding HIR to the
registered world, and adding a generation-owned HIR registry have different
stale, failure, memory, and work-accounting behavior. The package does not
select one.

### Resolver completeness

The current checker resolves environment methods, built-in collection and
domain methods, presentation-handle and integer methods, trait methods,
data-last callable fallback, and capacity methods before its final environment
fallback. The package's shared resolver section does not give these families
candidate identities or schema records. It also leaves several public result
and internal resolver types opaque. Extracting only the listed subset would
change checker behavior and leave two successful resolvers.

## Dispatch order

The following three design requests can be sent to separate assignees in
parallel because they own disjoint result-changing boundaries:

1. [AW-AH-009.3.1 call-surface syntax production reconciliation](../reviews/requests/2026-07-16-aw-ah-009.3.1-call-surface-syntax-production-reconciliation.md)
2. [AW-AH-009.3.2 accepted HIR and request-lifecycle production reconciliation](../reviews/requests/2026-07-16-aw-ah-009.3.2-accepted-hir-request-lifecycle-production-reconciliation.md)
3. [AW-AH-009.3.3 callable catalog and shared resolver production reconciliation](../reviews/requests/2026-07-16-aw-ah-009.3.3-callable-catalog-shared-resolver-production-reconciliation.md)

After all three return `READY_FOR_IMPLEMENTATION`, apply them in numeric order:
009.3.1 syntax/HIR ranges, 009.3.2 accepted HIR/request lifecycle, then 009.3.3
catalog/resolver and the original AW-AH-009.3 sema/LSP/cache/fallback migration.
The designs may run in parallel; production edits should remain sequential at
the shared AST, registered-world, and checker boundaries.

## Changes and deviations

- Updated AW-AH-009.3.1 with the current production re-audit.
- Added two new independently throwable reconciliation requests.
- Added no Rust, Cargo, schema, fixture, compatibility, source-gate,
  CSS/Takumi, or removed-syntax code.
- The only deviation from the original package's claimed
  `OPEN_RESULT_CHANGING_DECISIONS=0` is evidence-backed: current production
  cannot satisfy its query inputs or one-resolver requirement using the exact
  types the archive actually defines.

## Validation

All Cargo commands used `CARGO_INCREMENTAL=0`.

```bash
cargo fmt --all -- --check
cargo check -p arcweft-lang-sema -p arcweft-lsp --lib --all-features
cargo clippy -p arcweft-lang-sema -p arcweft-lsp --lib --all-features -- -D warnings
cargo test -p arcweft-lsp positions --lib
cargo test -p arcweft-lang-hir source_identity --lib
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo +nightly -Zscript tools/structure-audit.rs --root . --write docs/implementation/structure-audits/aw-ah-009-3-production-reconciliation-2026-07-16
```

All listed commands passed. The focused Cargo check, Clippy, and tests ran on
the immediately preceding `a347bffe80c2` parent before this docs-only change
was rebased onto `328e362f8118`; the position suite ran 6 tests and the exact
HIR source-identity filter ran 1 test. The final parent records its own passing
workspace check, workspace Clippy, syntax tests, format check, and diff check
in the Stage 1 bracket/call implementation note. After the final rebase onto
`354cc0964a21`, the root production checkout reran workspace check, workspace
Clippy, format, diff check, and the structural audit successfully.

The canonical structural report is stored under
`structure-audits/aw-ah-009-3-production-reconciliation-2026-07-16/`. It
scanned 2,958 files, including 1,457 Rust files, 681,517 physical Rust LOC,
and 90 package manifests, and reported zero errors and 129 existing warnings.

The broader command was also attempted:

```bash
cargo check --workspace --all-targets --all-features
```

The first attempt did not complete in the independent Jujutsu workspace because the ignored
test font `web/assets/noto-sans-jp-vf.ttf` is present in the root checkout but
is not a tracked file copied by `jj workspace add`. The failing
`include_bytes!` sites were `arcweft-glyphon`'s `shared_text_layout` test and
`arcweft-render-wgpu`'s dialogue prepared tests; no Rust type or lint diagnostic
failed first. The same command and workspace Clippy subsequently passed in the
root production checkout with the asset present. The independent-workspace
failure remains recorded because it is a real portability limitation of the
untracked test asset, not a Rust failure in this slice.

## Remaining work

All production work after the previously landed substrate remains open until
the three reconciliation contracts select their exact models. Completion still
requires the original focused suites, workspace check/clippy/test, fallback
deletion, package-wide direct test matrix, and canonical structural audit.
