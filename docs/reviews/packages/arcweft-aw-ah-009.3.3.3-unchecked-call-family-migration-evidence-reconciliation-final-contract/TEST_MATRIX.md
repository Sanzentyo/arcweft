# Test matrix

## 1. Test ownership and typed case model

Implement the matrix under the existing `arcweft-lang-sema` checker/signature
test ownership. Production semantics remain unchanged.

The test support uses typed records equivalent to the following. Exact field
privacy may follow the existing test module, but no string family identity,
source-file scan, repository-file scan, extension trait, or duplicate resolver
is allowed.

```rust
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MigrationCaseKind {
    Accepted,
    RejectedOrPoisoned,
    CleanRecovery,
}

#[cfg(test)]
struct MigrationCaseSpec {
    name: &'static str,
    family: CallableFamily,
    kind: MigrationCaseKind,
    build: fn() -> MigrationFixture,
}

#[cfg(test)]
struct MigrationFixture {
    document: Arc<SourceDocument>,
    project: HirProject,
    world: RegisteredSemanticWorld,
    call: SourceSpan,
    expected_candidate: CallableCandidateId,
    expected_result: Option<TypeKind>,
    expected_callable_diagnostic: Option<CallableDiagnosticCode>,
}

#[cfg(test)]
struct DispatchAuditReport {
    call_expressions: u64,
    shared_resolver_invocations: u64,
    old_dispatch_calls: u64,
    committed_argument_checks: BTreeMap<TypeExpressionId, u64>,
}
```

The fixture owns the exact typed candidate ID. Project, environment, lexical,
trait, data-last, and speaker IDs are returned by their typed fixture builders;
tests do not compare names or debug strings.

Each fixture contains exactly one target call. Supporting values (`Fx`, entity
refs, handles, maps, stage values, trait receiver, data-last receiver, and so
on) are introduced as typed fixture bindings rather than nested call
expressions. This makes the resolver and argument-check counters exact.

Dialogue uses the current authoritative typed Dialogue carrier and existing
checker/signature integration. The fixture must not restore superseded
speaker/content-call syntax, lower Dialogue into a fake ordinary `Expr::Call`,
or create a second expression arena. If a production surface is not applicable
to public signature projection, the typed `SignatureNotApplicable` outcome is
recorded separately and may not be used as a negative family case; the existing
native-facts route remains the only permitted path.

## 2. Exact case cardinality and taxonomy tests

| Test | Required assertion |
|---|---|
| `migration_classification_is_exhaustive_in_all_order` | iterating `CallableFamily::ALL` yields 23 distinct families and one exhaustive class for each, in the same order |
| `migration_classification_counts_are_exact` | `RejectingSchema == 20`, `IntentionallyUnchecked == 3` |
| `migration_unchecked_set_is_exact` | set equality with `{Drop, Promotion, Speaker}` |
| `migration_case_table_has_exact_cardinality` | 46 distinct case names; two rows per family |
| `migration_case_kinds_match_classification` | every family has `Accepted`; rejecting has `RejectedOrPoisoned`; unchecked has `CleanRecovery`; no other combination |
| `curried_candidate_uses_base_family` | every curried fixture reports the base family's classification and does not add a row |
| `typed_external_project_case_retains_project_family` | qualified/aliased external binding published by typed `ProjectSymbolPath` resolves to `Project`; no dotted-string reconstruction |

## 3. Closed family case table

The following rows are exact test obligations. `prebound` means the fixture
injects a typed value without an additional call expression.

| Family | Accepted case | Required second case | Exact retained outcome |
|---|---|---|---|
| `Fx` | `Fx::Conditional(condition = true, then = prebound Fx, else = prebound Fx)` | same candidate with `condition = 1i32` | accepted result `Fx`; second diagnostic `ArgumentTypeMismatch`, family `Fx` retained |
| `EnumConstructor` | expected nominal tuple variant with `1i32` payload | same expected variant with `"bad"` payload | expected nominal enum result; second `ArgumentTypeMismatch` |
| `ResultConstructor` | expected `Result<I32,String>`, `Ok(1i32)` | `Ok("bad")` under same expected type | exact expected Result; second `ArgumentTypeMismatch` |
| `OptionConstructor` | expected `Option<I32>`, `Some(1i32)` | `Some("bad")` under same expected type | exact expected Option; second `ArgumentTypeMismatch` |
| `Builtin` | `BuiltinCallableId::Sin` with `1.0f32` | same candidate with `"bad"` | result `F32`; second `ArgumentTypeMismatch` |
| `Agent` | `AgentIntrinsicSignatureId::Expect` with `true` | same candidate with `1i32` | result `Unit`; second `ArgumentTypeMismatch` |
| `Presentation` | `PresentationCallableId::Background` with a typed Asset ref | same candidate with `1i32` as asset | current background handle result; second `ArgumentTypeMismatch` |
| `Dialogue` | current typed `DialogueCallableId::SpeakerLine` carrier with accepted optional arguments | same current carrier with typed `view = 1i32` | result `Unit`; second `ArgumentTypeMismatch`; no old/fake surface |
| `Project` | typed project function `fn(I32)->I32` with `1i32` | same declaration with `"bad"` | exact project candidate/result; second `ArgumentTypeMismatch` |
| `Environment` | accepted published Standard/Adapter `fn(I32)->String` with `1i32` | same typed record with `"bad"` | exact environment ID/result; second `ArgumentTypeMismatch` |
| `Lexical` | typed `LocalCallableId` `fn(I32)->I32` with `1i32` | same local candidate with `"bad"` | exact local ID/result; second `ArgumentTypeMismatch` |
| `FunctionValue` | prebound `fn(I32)->I32` value with `1i32` | same value with `"bad"` | exact function-value ID/result; second `ArgumentTypeMismatch` |
| `CollectionMethod` | `Vec<I32>.contains(1i32)` | same receiver/candidate with `"bad"` | result `Bool`; second `ArgumentTypeMismatch` |
| `PresentationHandleMethod` | prebound handle `.hide()` | same handle `.hide(1i32)` | result `Unit`; second `TooManyPositionalArguments` |
| `IntegerMethod` | `I32.min(1i32)` | same receiver/candidate with `"bad"` | result `I32`; second `ArgumentTypeMismatch` |
| `DomainMethod` | prebound `Map<I32,String>.get(1i32)` | same `MapGet` candidate with `"bad"` | result `String`; second `ArgumentTypeMismatch` |
| `TraitMethod` | one visible typed trait method `fn(I32)->R` with `1i32` | same exact trait candidate with `"bad"` | exact `TraitCallableId`/result; second `ArgumentTypeMismatch` |
| `DataLast` | data-last base callable with receiver injected and remaining `I32` argument `1i32` | same `DataLastCallableId` with remaining argument `"bad"` | exact base/data-last identity; second `ArgumentTypeMismatch` |
| `CapacityMethod` | one-arity current capacity method such as `reserve(1usize)` | same one-arity candidate with one authored spread argument | current result; second `UnsupportedSpread`; arity identity remains one |
| `StageMethod` | prebound `StageApi.acquire(prebound PresentationLifetime)` | same `StageMethodId::Acquire` with `1i32` | result `StageActorHandle`; second `ArgumentTypeMismatch` |
| `Drop` | selected `.drop()` or equivalent exact `DropCallableId::Drop` fixture | same candidate with unresolved expression `missing_drop_arg` | result `Unit`; second is `Selected`, clean poison, no callable diagnostic, unresolved slot expected/inferred `None` |
| `Promotion` | `PromotionCallableId::Promote` with any resolved value | same candidate with unresolved expression `missing_promote_arg` | result `Promoted`; second is `Selected`, clean poison, no callable diagnostic, unresolved slot expected/inferred `None` |
| `Speaker` | resolved character speaker/preset callable with no arguments | same exact speaker candidate with unresolved positional and optional open-named values | result `SpeakerPreset(Character)`; second is `Selected`, clean poison, no callable diagnostic, every unresolved slot expected/inferred `None` |

For the 20 rejecting rows, a family-specific existing recovery path that keeps
`Selected` may satisfy the second row only when its documented poison is
non-clean and the exact typed callable diagnostic is asserted. An unknown or
non-callable target never satisfies the row.

For the three unchecked rows, the unresolved names are argument expressions,
not callees. The callee is fully resolved before argument checking. This is the
critical distinction between truthful clean recovery and an unknown-target
negative.

## 4. Exact unchecked schema and result tests

| Test | Required assertion |
|---|---|
| `drop_schema_is_exact_variadic_unchecked` | exact one-group/one-rest-unchecked/open-unchecked/spread-unchecked shape; validator Drop; result Unit |
| `promotion_schemas_are_exact_variadic_unchecked` | enumerate Promote, PromoteUnchecked, Assume; exact unchecked shape; exact validator ID; results Promoted, Promoted, Unit |
| `speaker_schemas_are_exact_variadic_unchecked` | enumerate character speaker and preset forms; exact unchecked shape; validator Speaker; result `SpeakerPreset(Character)` |
| `unchecked_recovery_retains_clean_candidate_and_result` | for all three family recovery cases: `Selected`, exact ID, exact result/effects, `CallPoison::Clean`, no callable diagnostic |
| `unchecked_recovery_slots_are_checked_once_without_expectation` | every recovery slot has one committed check, `expected == None`, `inferred == None`, slot/argument poison Clean |
| `unchecked_unknown_named_and_spread_policy_stays_open` | typed open-named and unchecked-spread samples remain clean; no fabricated parameter coordinate or diagnostic |

These tests assert current production behavior. They do not add a branch to
force clean status and they do not suppress ordinary expression diagnostics.

## 5. Checker/signature and counter tests

Run every applicable matrix case through the public checker and public
signature query against the same accepted document/HIR/world lease.

| Test | Required assertion |
|---|---|
| `migration_matrix_uses_one_shared_resolver_per_call` | `shared_resolver_invocations == call_expressions` for accepted, rejected/poisoned, and clean-recovery batches |
| `migration_matrix_never_uses_old_dispatch` | `old_dispatch_calls == 0` using typed dispatcher-boundary instrumentation, not source scanning |
| `migration_checker_signature_primary_candidate_parity` | selected ID parity for accepted/recovery; deterministic first retained ID parity for rejected/poisoned; no parity assertion for missing/non-callable/terminal dispositions |
| `migration_argument_checks_are_exactly_once` | transaction-aware multiset has count 1 for every slot published in `CallTargetFacts`; no extra committed expression ID |
| `rejected_case_keeps_diagnostic_candidate_order` | checker `Rejected.candidates` order equals public signature order; active signature is index zero; no candidate is relabelled selected |
| `clean_recovery_query_is_help_not_not_applicable` | Drop/Promotion/Speaker queries return Help with exact primary and clean signature poison |
| `candidate_probe_rollback_does_not_leak_counts_or_facts` | rejected speculative candidates contribute no committed argument count, diagnostic, effect, warning, or target fact |

The audit recorder is test-only and transaction-aware. It observes existing
boundaries; it does not alter candidate viability, ranking, result, diagnostic,
poison, or replay.

## 6. Non-family disposition guard tests

| Test | Input | Required assertion |
|---|---|---|
| `unknown_target_is_not_family_rejection` | unknown callee | `CallTargetFact::Missing`; no family classification credit |
| `non_callable_shadow_is_not_family_rejection` | lexical/project non-callable with same spelling as environment callable | `CallTargetFact::NonCallable`; no environment/family negative credit |
| `unsupported_surface_is_not_family_rejection` | removed/superseded Dialogue spelling | production parser/HIR outcome only; no fake candidate, no matrix credit |
| `terminal_error_is_not_family_rejection` | cancellation/deadline/work/limit/world/source failure | typed terminal error; no partial fact/help/cache and no family credit |
| `wrong_receiver_unknown_method_is_not_capacity_or_domain_rejection` | known spelling on unsupported receiver | missing/unknown method; not a Capacity/Domain row |

## 7. Compile and category drift tests

| Test | Failure detected |
|---|---|
| exhaustive `CallableFamily::migration_validator_class` match | a new enum variant has no explicit class: compile failure |
| `migration_classification_is_exhaustive_in_all_order` | omitted, duplicate, reordered, or stale table entry |
| exact 20/3 and exact unchecked-set tests | implicit class drift or a new family added without matrix decision |
| `drop/promotion/speaker` schema-shape tests | validator/argument policy becomes typed or rejecting without contract update |
| 20 genuine negative case tests | a formerly rejecting family no longer has the documented production rejection |
| 46-case taxonomy test | missing accepted/second case or duplicated family/case kind |

No `trybuild` fixture is required for the new-family guard: the exhaustive
inherent match is the compile-time gate. Runtime table tests then close
cardinality and semantic drift.

## 8. Focused implementation validation

The implementing change must run at least:

```text
cargo fmt --all -- --check
git diff --check
cargo test -p arcweft-lang-sema migration_classification --lib
cargo test -p arcweft-lang-sema migration_case_table --lib
cargo test -p arcweft-lang-sema migration_unchecked --lib
cargo test -p arcweft-lang-sema migration_matrix --lib
cargo test -p arcweft-lang-sema --test call_target_facts_public_api
cargo test -p arcweft-lang-sema --test call_surface_signature_matrix
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run the repository-required workspace/Tier 2/structure-audit gates for the
coherent implementation cut. This design archive does not claim those commands
were executed here.
