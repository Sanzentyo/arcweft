# Lang-01.6 trusted-proof surface diagnostics cut 1c

## Scope and package evidence

This cut implements the Stage-4-independent source-surface portion of
`arcweft-lang-01.6-trusted-axiom-surface-final-contract.zip`.

- package path:
  `D:/sanze/Downloads/arcweft-lang-01.6-trusted-axiom-surface-final-contract.zip`;
- SHA-256:
  `7DD9E9282EE8B54B7F8E176D34AA5BC9A6766924C6F51C4D0520659B3CC9E600`;
- all 31 archive entries were inspected and all 30 manifest hash/size rows
  matched.

The dedicated `trusted axiom` declaration had already been removed. This cut
does not restore a recognizer, CST/AST/HIR kind, compatibility reader,
spelling-specific diagnostic, or source gate for it.

## Implemented

- `verify.trusted` is a proof-only reserved outer attribute. Attaching it to a
  function, flow, entity, source, Style, source file, or any other non-proof
  owner emits `syntax.proof.trusted.not_proof`; the invalid attribute is not
  retained by that owner.
- Trusted-proof metadata now owns the eight required structured syntax
  diagnostics:
  `not_proof`, `duplicate`, `reason_missing`, `reason_duplicate`,
  `reason_not_string`, `reason_empty`, `unknown_argument`, and
  `positional_argument`.
- Missing and empty argument lists, duplicate attributes and names, unknown and
  positional arguments, non-string expressions, empty strings, and
  Unicode-whitespace-only decoded strings cannot construct `ProofItem`.
- `DecodedStringLiteral` is the syntax-owned semantic decoder for string
  literal bodies. Proof trust reasons and runtime expression lowering use the
  same API, so valid escape semantics cannot diverge between the authoring and
  execution paths. The proof AST retains the exact decoded value after checking
  `trim().is_empty()`; it does not normalize accepted whitespace.
- `ProofTrust::Trusted` also owns the exact `attribute_range: TextRange`.
  Current HIR lowering clones the complete `ProofItem`, so the source range is
  retained through the present HIR boundary while the final typed `HirProof`
  awaits Proof Stage 4.
- The ordinary expression grammar has no interpolated-string AST variant.
  Trusted reasons accept only `Expr::Literal(Literal::String(_))`. A call or
  other expression is rejected as `reason_not_string`, while text such as
  `$(` inside an ordinary quoted literal remains literal text rather than being
  rejected by a spelling scan.
- Legacy `trusted axiom` input continues through ordinary current-grammar
  recovery with `syntax.parse` and cannot construct a proof.

## Existing enforcement retained

The pre-existing semantic pass propagates trusted dependencies transitively
through its bounded fixed point, and the verifier/CLI currently warn or deny
them according to the selected mode. The release bundle crate already owns the
strict signed authority, admission, evidence, revocation, generation, channel,
and size-limit wire substrate implemented by cuts 1a and 1b.

Those pieces are useful evidence, but they do not complete Lang-01.6: the
current verifier switch is still the broad
`VerificationPolicy::allow_trusted_proofs: bool`, not the final signed
admission policy.

## Stage-4 blockers and mandatory next order

The remaining work must wait for the atomic Proof Stage 4 public switch rather
than binding signed release authority to provisional string identities:

1. `arcweft-lang-syntax::ast::proof::ProofItem` still owns `IdRef`, raw
   `body: String`, and `Vec<ProofClause>`. It does not yet own the final
   ordinary proof name, typed parameters/contracts, or exact `ProofBlock`.
2. `HirTopLevelDecl::Proof` still clones that syntax `ProofItem`; there is no
   `HirProof` bound to the accepted `CallableDeclarationId` owner.
3. There is no canonical `ProofContractDigest`, `TrustedProofRoot`,
   `VerificationSubject`, or opaque `ValidatedBuildVerification`.
4. `SemanticProofSummary` and verifier reports still project proof IDs and
   trusted dependencies as `String`/`Vec<String>` rather than canonical direct
   and effective trust roots.
5. `SemanticPolicy` and `VerificationPolicy` still expose
   `allow_trusted_proofs: bool`; the final
   `TrustedProofPolicy::{Deny, WarnUnadmitted, RequireAdmissions(...)}` and
   exact admission matching cannot be connected safely before items 1–4.

The required implementation order remains:

```text
Proof Stage 4 ordinary proof AST/HIR identity
  -> canonical semantic contract digest and trusted-root closure
  -> verifier policy enum and exact signed admission matching
  -> mandatory bundle/AWFR audit and cache identities
  -> release/dependency/player startup gates
  -> CLI/LSP/Agent inspection and final boolean/string deletion
```

No package requirement behind those blockers is counted complete by this cut.

## Structural measurement

The final review was prepared against parent revision `e6e8cce33d4c`. No Cargo
dependency or feature edge was added: runtime-plan already consumes the syntax
surface through the HIR syntax namespace.

| Path | Owner / kind | Bytes | Physical LOC | Responsibility in this cut |
| --- | --- | ---: | ---: | --- |
| `crates/arcweft-lang-syntax/src/ast/proof.rs` | syntax / production | 3,366 | 171 | decoded reason and exact trust-attribute range |
| `crates/arcweft-lang-syntax/src/expr/string_literal.rs` | syntax / production | 3,060 | 100 | sole decoded string-literal semantic value |
| `crates/arcweft-lang-syntax/src/expr.rs` | syntax / production | 46,143 | 1,633 | existing expression API; only publishes the responsibility module |
| `crates/arcweft-lang-syntax/src/parser.rs` | syntax / production | 25,195 | 748 | pending-attribute ownership and typed rejection |
| `crates/arcweft-lang-syntax/src/parser/top_level.rs` | syntax / production | 11,481 | 280 | proof-only attachment dispatch |
| `crates/arcweft-lang-syntax/src/parser/proof.rs` | syntax / production | 13,221 | 401 | trusted-proof argument grammar, range, and diagnostics |
| `crates/arcweft-lang-syntax/src/parser/recovery.rs` | syntax / production | 39,589 | 1,013 | closed typed parser-diagnostic inventory |
| `crates/arcweft-lang-syntax/tests/trusted_proof_attribute.rs` | syntax / integration test | 5,409 | 225 | positive, negative, exact-range, attachment, and removal behavior |
| `crates/arcweft-lang-sema/src/semantic.rs` | sema / production | 78,957 | 2,121 | existing trust projection; one exhaustive match updated |
| `crates/arcweft-lang-sema/src/tests/declarations.rs` | sema / unit test module | 41,233 | 1,357 | HIR-clone and decoded trust assertions |
| `crates/arcweft-runtime-plan/src/expr.rs` | runtime-plan / production | 84,530 | 2,382 | existing runtime expression lowering; duplicate decoder removed |
| `crates/arcweft-runtime-plan/src/expr/tests.rs` | runtime-plan / unit test module | 33,180 | 976 | shared-decoder runtime evidence |

The existing `expr.rs` owners exceed the 1,200-LOC warning threshold but remain
below the 2,500-LOC error threshold. This cut reduces runtime-plan
responsibility by removing its local decoder and places the new cohesive
algorithm in a 100-line syntax responsibility module; it does not add another
responsibility to either large owner.

## Validation

Executed with `CARGO_INCREMENTAL=0`:

```text
cargo test -p arcweft-lang-syntax --test trusted_proof_attribute
  PASS: 4 passed, 0 failed

cargo test -p arcweft-lang-syntax
  PASS: 311 syntax unit tests plus integration, UI, and doc tests

cargo clippy -p arcweft-lang-syntax -p arcweft-lang-sema \
  -p arcweft-runtime-plan --all-targets --all-features -- -D warnings
  PASS

cargo test -p arcweft-runtime-plan \
  runtime_string_lowering_uses_the_shared_syntax_decoder --lib
  PASS: 1 passed, 0 failed

cargo clippy -p arcweft-runtime-plan --all-targets --all-features -- -D warnings
  PASS

cargo test -p arcweft-bundle verification_trust --lib
  PASS: 20 passed, 0 failed

cargo test -p arcweft-lang-sema \
  parses_verified_and_trusted_proofs_as_one_structured_declaration_family --lib
cargo test -p arcweft-lang-sema \
  malformed_trust_metadata_and_separate_axiom_declarations_do_not_lower_as_proofs --lib
cargo test -p arcweft-lang-sema \
  semantic_pass_propagates_trust_through_proof_dependencies --lib
  PASS: 3 focused tests

cargo test -p arcweft-verify \
  trusted_proof_evidence_is_transitive_auditable_and_policy_controlled --lib
  PASS: 1 passed, 0 failed

cargo +nightly -Zscript tools/structure-audit.rs --root .
  PASS: 3,348 files; 1,721 Rust files; 794,222 physical Rust LOC;
        0 errors, 129 warnings
```

`just test-tier2` was also attempted because the shared decoder crosses the
syntax/runtime boundary. It stopped during compilation, before any Tier 2 test
ran, on an independent in-flight Lang-01.5 project-loader migration:

```text
crates/arcweft-project-loader/src/topology/model.rs
  unresolved `adapter_manifest`
  unresolved `rust_metadata`
  missing `AdapterCallableModelError`
```

This cut does not touch that owner, and production trust/string contracts were
not reverted to satisfy the blocked slow harness. Tier 2 remains a recorded
verification gap until the single-manifest project-loader cut compiles.
