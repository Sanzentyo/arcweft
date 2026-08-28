use std::sync::atomic::AtomicBool;

use arcweft_lang_hir::expr::HirExprKind;
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

use super::*;
use crate::final_analysis::tests::{analyze, fixture};

fn literal_domain_limits(max_transcript_bytes: u64) -> CheckedMatchLimits {
    CheckedMatchLimits::PRODUCTION
        .with_limit(CheckedMatchLimitKind::TranscriptBytes, max_transcript_bytes)
}

fn exercise_duplicate_literal_domain(
    max_transcript_bytes: u64,
) -> Result<(CheckedMatchWork, usize), CheckedMatchBuildError> {
    let fixture = fixture(
        concat!(
            "fn root(value: i64) -> i64 {\n",
            "    match value {\n",
            "        1i64 | 1i64 => 1i64\n",
            "        _ => 0i64\n",
            "    }\n",
            "}\n",
        ),
        None,
    );
    let analysis = analyze(&fixture).expect("checked literal fixture");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let (match_owner, authored) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Match(authored) => Some((owner, authored)),
            _ => None,
        })
        .expect("literal Match");
    let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
        analysis.accepted_root_catalog(),
        &analysis,
    );
    let match_path = coordinates.expression(match_owner).unwrap();
    let mut observed = Vec::new();
    for (ordinal, arm) in authored.arms().iter().enumerate() {
        let ordinal = u32::try_from(ordinal).unwrap();
        let root = StablePatternCoordinate::new([]);
        observed.push((
            arm.pattern(),
            StableSemanticCoordinate::pattern(match_path.clone(), ordinal, root.clone()),
        ));
        if let HirPatternKind::Or { alternatives } =
            module.resolve_pattern(arm.pattern()).unwrap().kind()
        {
            for (alternative, pattern) in alternatives.iter().enumerate() {
                let alternative = u32::try_from(alternative).unwrap();
                let relative = append_coordinate(
                    &root,
                    StablePatternCoordinateStep::OrAlternative(alternative),
                );
                observed.push((
                    *pattern,
                    StableSemanticCoordinate::pattern(match_path.clone(), ordinal, relative),
                ));
            }
        }
    }
    let cancellation = AtomicBool::new(false);
    let mut budget = CheckedMatchBudget::new(literal_domain_limits(max_transcript_bytes));
    let cache_len;
    {
        let mut analyzer = MatchCoverageAnalyzer::new(
            &analysis,
            module,
            FinalSemanticAnalysisControl::new(&cancellation),
            &mut budget,
            StableSemanticCoordinate::new(match_path.clone()),
            observed,
        );
        let ty = TypeKind::I64;
        let match_coordinate = StableSemanticCoordinate::new(match_path.clone());
        let domain = analyzer.domain(&ty, &match_coordinate)?;
        analyzer.domain_digest(&ty, &domain)?;
        for (ordinal, arm) in authored.arms().iter().enumerate() {
            let ordinal = u32::try_from(ordinal).unwrap();
            let arm_coordinate = StableMatchArmCoordinate::new(match_path.clone(), ordinal);
            analyzer.deconstruct(
                arm.pattern(),
                &ty,
                &arm_coordinate,
                StablePatternCoordinate::new([]),
                0,
            )?;
        }
        cache_len = analyzer.canonical_literals.len();
    }
    Ok((budget.work(), cache_len))
}

fn recursive_domain_patterns(
    recursive_type: &TypeKind,
    owner: SemanticTypeDigest,
    semantic_coordinate: &StableSemanticCoordinate,
) -> (
    CoverageConstructor,
    CoverageConstructor,
    DeconstructedPattern,
    DeconstructedPattern,
    DeconstructedPattern,
) {
    let base = CoverageConstructor::nullary(CoverageConstructorId::Other(owner));
    let recursive = CoverageConstructor {
        identity: CoverageConstructorId::Tuple { owner },
        field_types: vec![recursive_type.clone()].into_boxed_slice(),
        variant_payload: None,
    };
    let coordinate = StablePatternCoordinate::new([]);
    let base_pattern = DeconstructedPattern {
        coordinate: coordinate.clone(),
        semantic_coordinate: semantic_coordinate.clone(),
        kind: DeconstructedPatternKind::Constructor {
            constructor: base.identity.clone(),
            fields: Box::new([]),
        },
    };
    let recursive_pattern = DeconstructedPattern {
        coordinate: coordinate.clone(),
        semantic_coordinate: semantic_coordinate.clone(),
        kind: DeconstructedPatternKind::Constructor {
            constructor: recursive.identity.clone(),
            fields: vec![DeconstructedPattern::wildcard(
                coordinate.clone(),
                semantic_coordinate.clone(),
            )]
            .into_boxed_slice(),
        },
    };
    let wildcard = DeconstructedPattern::wildcard(coordinate, semantic_coordinate.clone());
    (base, recursive, base_pattern, recursive_pattern, wildcard)
}

#[test]
fn duplicate_literals_charge_only_exact_domain_transcript_writes() {
    // prefix/type/domain row = 80, canonical literal row = 62, Other row = 41.
    const EXACT_DOMAIN_BYTES: u64 = 183;
    let (work, cache_len) = exercise_duplicate_literal_domain(EXACT_DOMAIN_BYTES)
        .expect("the exact byte limit admits the canonical domain transcript");
    assert_eq!(
        work.observed(CheckedMatchLimitKind::TranscriptBytes),
        EXACT_DOMAIN_BYTES
    );
    assert_eq!(cache_len, 1, "duplicate literals share one canonical value");

    for _ in 0..2 {
        assert!(matches!(
            exercise_duplicate_literal_domain(EXACT_DOMAIN_BYTES - 1),
            Err(CheckedMatchBuildError::LimitExceeded {
                kind: CheckedMatchLimitKind::TranscriptBytes,
                limit,
                attempted,
            }) if limit == EXACT_DOMAIN_BYTES - 1 && attempted == EXACT_DOMAIN_BYTES
        ));
    }
}

#[test]
fn recursive_usefulness_distinguishes_covered_cycles_finite_witnesses_and_no_base() {
    let fixture = fixture(
        concat!(
            "fn root(flag: bool) -> i64 {\n",
            "    match flag {\n",
            "        true => 1i64\n",
            "        false => 0i64\n",
            "    }\n",
            "}\n",
        ),
        None,
    );
    let analysis = analyze(&fixture).expect("checked fixture");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let match_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
        analysis.accepted_root_catalog(),
        &analysis,
    );
    let match_path = coordinates.expression(match_owner).unwrap();
    let semantic_coordinate = StableSemanticCoordinate::new(match_path.clone());
    let cancellation = AtomicBool::new(false);
    let mut budget = CheckedMatchBudget::new(CheckedMatchLimits::PRODUCTION);
    let mut analyzer = MatchCoverageAnalyzer::new(
        &analysis,
        module,
        FinalSemanticAnalysisControl::new(&cancellation),
        &mut budget,
        semantic_coordinate.clone(),
        Vec::new(),
    );

    let recursive_type = TypeKind::Named("coverage-recursive-test".to_owned());
    let owner = recursive_type.semantic_identity_digest();
    let (base, recursive, base_pattern, recursive_pattern, wildcard) =
        recursive_domain_patterns(&recursive_type, owner, &semantic_coordinate);
    analyzer.domain_overrides.insert(
        owner,
        CoverageTypeDomain::Constructors(vec![base.clone(), recursive.clone()].into_boxed_slice()),
    );

    assert!(
        analyzer
            .useful(
                &vec![vec![base_pattern.clone()], vec![recursive_pattern]],
                std::slice::from_ref(&wildcard),
                std::slice::from_ref(&recursive_type),
                0,
                &mut Vec::new(),
            )
            .expect("covered recursive domain")
            .is_none()
    );

    let witness = analyzer
        .useful(
            &vec![vec![base_pattern]],
            std::slice::from_ref(&wildcard),
            std::slice::from_ref(&recursive_type),
            0,
            &mut Vec::new(),
        )
        .expect("recursive branch has the finite base witness")
        .expect("missing recursive branch is useful");
    assert!(matches!(
        witness.as_slice(),
        [CheckedCoverageWitness::Tuple(fields)]
            if matches!(fields.as_ref(), [CheckedCoverageWitness::Other { .. }])
    ));

    analyzer.domain_overrides.insert(
        owner,
        CoverageTypeDomain::Constructors(vec![recursive].into_boxed_slice()),
    );
    assert!(matches!(
        analyzer.useful(
            &Vec::new(),
            &[wildcard],
            &[recursive_type],
            0,
            &mut Vec::new(),
        ),
        Err(CheckedMatchBuildError::UnsupportedDomain { type_digest })
            if type_digest == owner
    ));
}

fn assert_unit_variant_witness(owner: SemanticTypeDigest, coordinate: &StableSemanticCoordinate) {
    let shape = VariantPayloadShape::Unit;
    let case = AcceptedVariantCaseSemanticId::issue(
        crate::types::VariantPayloadOwnerFamily::BuiltinClosed,
        owner,
        0,
        &shape,
    );
    let constructor = CoverageConstructor {
        identity: CoverageConstructorId::Variant {
            owner,
            case,
            ordinal: 0,
        },
        field_types: Box::new([]),
        variant_payload: Some(shape),
    };
    assert!(matches!(
        MatchCoverageAnalyzer::constructor_witness(&constructor, Vec::new(), coordinate)
            .expect("unit witness"),
        CheckedCoverageWitness::Variant {
            case: actual,
            payload: CheckedVariantCoverageWitness::Unit,
        } if actual == case
    ));
}

fn assert_tuple_variant_witness(owner: SemanticTypeDigest, coordinate: &StableSemanticCoordinate) {
    let shape = VariantPayloadShape::try_tuple(
        crate::types::VariantPayloadOwnerFamily::BuiltinClosed,
        owner,
        1,
        [TypeKind::Bool, TypeKind::Bool],
    )
    .expect("two-field tuple payload");
    let case = AcceptedVariantCaseSemanticId::issue(
        crate::types::VariantPayloadOwnerFamily::BuiltinClosed,
        owner,
        1,
        &shape,
    );
    let constructor = CoverageConstructor {
        identity: CoverageConstructorId::Variant {
            owner,
            case,
            ordinal: 1,
        },
        field_types: variant_payload_field_types(&shape),
        variant_payload: Some(shape),
    };
    assert!(matches!(
        MatchCoverageAnalyzer::constructor_witness(
            &constructor,
            vec![
                CheckedCoverageWitness::Bool(false),
                CheckedCoverageWitness::Bool(true),
            ],
            coordinate,
        )
        .expect("tuple witness"),
        CheckedCoverageWitness::Variant {
            case: actual,
            payload: CheckedVariantCoverageWitness::Tuple(fields),
        } if actual == case
            && matches!(fields.as_ref(), [
                CheckedCoverageWitness::Bool(false),
                CheckedCoverageWitness::Bool(true),
            ])
    ));
}

fn assert_record_variant_witness(owner: SemanticTypeDigest, coordinate: &StableSemanticCoordinate) {
    let shape = VariantPayloadShape::try_record(
        crate::types::VariantPayloadOwnerFamily::BuiltinClosed,
        owner,
        2,
        [
            ("diagnostic-z".to_owned(), TypeKind::Bool),
            ("diagnostic-a".to_owned(), TypeKind::Bool),
        ],
    )
    .expect("two-field record payload");
    let record_ids = shape
        .record_fields()
        .expect("record schema")
        .iter()
        .map(crate::types::VariantPayloadRecordField::semantic_id)
        .collect::<Vec<_>>();
    let case = AcceptedVariantCaseSemanticId::issue(
        crate::types::VariantPayloadOwnerFamily::BuiltinClosed,
        owner,
        2,
        &shape,
    );
    let constructor = CoverageConstructor {
        identity: CoverageConstructorId::Variant {
            owner,
            case,
            ordinal: 2,
        },
        field_types: variant_payload_field_types(&shape),
        variant_payload: Some(shape),
    };
    let witness = MatchCoverageAnalyzer::constructor_witness(
        &constructor,
        vec![
            CheckedCoverageWitness::Bool(true),
            CheckedCoverageWitness::Bool(false),
        ],
        coordinate,
    )
    .expect("record witness");
    let CheckedCoverageWitness::Variant {
        case: actual,
        payload: CheckedVariantCoverageWitness::Record(rows),
    } = &witness
    else {
        panic!("record case retains a record witness")
    };
    assert_eq!(*actual, case);
    assert!(matches!(rows.as_ref(), [first, second]
        if first.semantic_id == record_ids[0]
            && first.value == CheckedCoverageWitness::Bool(true)
            && second.semantic_id == record_ids[1]
            && second.value == CheckedCoverageWitness::Bool(false)));
    let debug = format!("{witness:?}");
    assert!(!debug.contains("diagnostic-z"));
    assert!(!debug.contains("diagnostic-a"));
}

#[test]
fn variant_witnesses_preserve_unit_tuple_and_name_free_record_rows() {
    let fixture = fixture(
        concat!(
            "fn root(flag: bool) -> i64 {\n",
            "    match flag {\n",
            "        true => 1i64\n",
            "        false => 0i64\n",
            "    }\n",
            "}\n",
        ),
        None,
    );
    let analysis = analyze(&fixture).expect("checked witness fixture");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let match_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
        analysis.accepted_root_catalog(),
        &analysis,
    );
    let semantic_coordinate = StableSemanticCoordinate::new(
        coordinates
            .expression(match_owner)
            .expect("stable Match path"),
    );
    let owner = TypeKind::Named("WitnessOwner".to_owned()).semantic_identity_digest();

    assert_unit_variant_witness(owner, &semantic_coordinate);
    assert_tuple_variant_witness(owner, &semantic_coordinate);
    assert_record_variant_witness(owner, &semantic_coordinate);
}

#[test]
fn invalid_checked_constructor_precedes_the_witness_budget() {
    let fixture = fixture(
        concat!(
            "fn root(flag: bool) -> i64 {\n",
            "    match flag {\n",
            "        true => 1i64\n",
            "        false => 0i64\n",
            "    }\n",
            "}\n",
        ),
        None,
    );
    let analysis = analyze(&fixture).expect("checked invalid-row fixture");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let match_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
        analysis.accepted_root_catalog(),
        &analysis,
    );
    let semantic_coordinate = StableSemanticCoordinate::new(
        coordinates
            .expression(match_owner)
            .expect("stable Match path"),
    );
    let cancellation = AtomicBool::new(false);
    let limits = CheckedMatchLimits::PRODUCTION.with_limit(CheckedMatchLimitKind::WitnessNodes, 0);
    let mut budget = CheckedMatchBudget::new(limits);
    let mut analyzer = MatchCoverageAnalyzer::new(
        &analysis,
        module,
        FinalSemanticAnalysisControl::new(&cancellation),
        &mut budget,
        semantic_coordinate.clone(),
        Vec::new(),
    );
    let invalid = DeconstructedPattern {
        coordinate: StablePatternCoordinate::new([]),
        semantic_coordinate: semantic_coordinate.clone(),
        kind: DeconstructedPatternKind::Constructor {
            constructor: CoverageConstructorId::Unit,
            fields: Box::new([]),
        },
    };
    assert!(matches!(
        analyzer.useful(
            &Vec::new(),
            &[invalid],
            &[TypeKind::Bool],
            0,
            &mut Vec::new(),
        ),
        Err(CheckedMatchBuildError::InvalidCheckedRow { coordinate })
            if coordinate == semantic_coordinate
    ));
    drop(analyzer);
    assert_eq!(
        budget.work().observed(CheckedMatchLimitKind::WitnessNodes),
        0
    );
}

fn observed_work(work: CheckedMatchWork, kind: CheckedMatchLimitKind) -> u64 {
    work.observed(kind)
}

fn uniform_limits(maximum: u64) -> CheckedMatchLimits {
    CheckedMatchLimits::uniform(maximum)
}

#[test]
fn every_checked_match_counter_is_exact_one_over_and_overflow_safe() {
    const KINDS: [CheckedMatchLimitKind; CheckedMatchLimitKind::COUNT] = CheckedMatchLimitKind::ALL;

    for kind in KINDS {
        let exercise = || {
            let mut budget = CheckedMatchBudget::new(uniform_limits(2));
            if kind == CheckedMatchLimitKind::Depth {
                budget.observe_depth(2)?;
                let error = budget
                    .observe_depth(3)
                    .expect_err("first one-over must reject");
                Ok::<_, CheckedMatchBuildError>((budget.work(), error))
            } else {
                budget.charge(kind, 2)?;
                let error = budget
                    .charge(kind, 1)
                    .expect_err("first one-over must reject");
                Ok((budget.work(), error))
            }
        };
        let (work, first) = exercise().expect("exact limit must succeed");
        let (_, repeated) = exercise().expect("repeat must be deterministic");
        assert_eq!(observed_work(work, kind), 2, "exact counter {kind:?}");
        assert_eq!(first, repeated, "deterministic counter {kind:?}");
        assert_eq!(
            first,
            CheckedMatchBuildError::LimitExceeded {
                kind,
                limit: 2,
                attempted: 3,
            }
        );

        let mut overflow = CheckedMatchBudget::new(uniform_limits(u64::MAX));
        overflow
            .charge(kind, u64::MAX)
            .expect("maximum representable charge succeeds");
        assert_eq!(
            overflow.charge(kind, 1),
            Err(CheckedMatchBuildError::ArithmeticOverflow { kind })
        );
    }
}

fn execute_match_work(
    source: &str,
    limits: CheckedMatchLimits,
) -> Result<CheckedMatchWork, crate::final_analysis::semantic_transcript::SemanticTranscriptError> {
    let fixture = fixture(source, None);
    let analysis = analyze(&fixture).expect("checked executable-limit fixture");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let reference = analysis.checked_match_ref(module, &fixture.symbols, owner)?;
    analysis
        .build_checked_match_for_ref(project, &fixture.symbols, reference, limits)
        .map(|checked| checked.coverage().work)
}

fn exact_work_limits(work: CheckedMatchWork) -> CheckedMatchLimits {
    CheckedMatchLimitKind::ALL
        .into_iter()
        .fold(CheckedMatchLimits::uniform(0), |limits, kind| {
            limits.with_limit(kind, work.observed(kind))
        })
}

fn with_one_less(limits: CheckedMatchLimits, kind: CheckedMatchLimitKind) -> CheckedMatchLimits {
    let limit = limits
        .limit(kind)
        .checked_sub(1)
        .expect("fixture exercises every counter");
    limits.with_limit(kind, limit)
}

#[test]
fn executable_match_paths_honor_exact_and_one_less_for_all_eleven_limits() {
    const SOURCE: &str = r"
fn root(items: Vec<bool>, ready: bool) -> i64 {
    match items {
        [] => 0i64
        [true] | [false] => 1i64
        [true] => 2i64
        [_, ..] when ready => 3i64
        [_, ..] => 4i64
    }
}
";
    const KINDS: [CheckedMatchLimitKind; CheckedMatchLimitKind::COUNT] = CheckedMatchLimitKind::ALL;

    let baseline = execute_match_work(SOURCE, CheckedMatchLimits::PRODUCTION)
        .expect("production executable Match");
    for kind in KINDS {
        assert!(
            observed_work(baseline, kind) > 0,
            "fixture counter {kind:?}"
        );
    }
    let exact = exact_work_limits(baseline);
    assert_eq!(
        execute_match_work(SOURCE, exact).expect("all exact limits succeed"),
        baseline
    );
    for kind in KINDS {
        let limits = with_one_less(exact, kind);
        let expected_limit = observed_work(baseline, kind) - 1;
        for _ in 0..2 {
            assert!(
                matches!(
                    execute_match_work(SOURCE, limits),
                    Err(crate::final_analysis::semantic_transcript::SemanticTranscriptError::MatchBuild(
                        CheckedMatchBuildError::LimitExceeded {
                            kind: actual,
                            limit,
                            attempted,
                        }
                    )) if actual == kind && limit == expected_limit && attempted > limit
                ),
                "one-less executable counter {kind:?}"
            );
        }
    }
}

#[derive(Clone, Debug)]
enum OraclePattern {
    Any,
    Unit,
    Bool(bool),
    Tuple(Box<[OraclePattern]>),
    Constructor {
        index: usize,
        fields: Box<[OraclePattern]>,
    },
}

#[derive(Clone, Debug)]
enum OracleValue {
    Unit,
    Bool(bool),
    Tuple(Box<[OracleValue]>),
    Constructor {
        index: usize,
        fields: Box<[OracleValue]>,
    },
}

fn oracle_matches(pattern: &OraclePattern, value: &OracleValue) -> bool {
    match (pattern, value) {
        (OraclePattern::Any, _) | (OraclePattern::Unit, OracleValue::Unit) => true,
        (OraclePattern::Bool(pattern), OracleValue::Bool(value)) => pattern == value,
        (OraclePattern::Tuple(patterns), OracleValue::Tuple(values)) => {
            patterns.len() == values.len()
                && patterns
                    .iter()
                    .zip(values.iter())
                    .all(|(pattern, value)| oracle_matches(pattern, value))
        }
        (
            OraclePattern::Constructor {
                index: pattern_index,
                fields: patterns,
            },
            OracleValue::Constructor {
                index: value_index,
                fields: values,
            },
        ) => {
            pattern_index == value_index
                && patterns.len() == values.len()
                && patterns
                    .iter()
                    .zip(values.iter())
                    .all(|(pattern, value)| oracle_matches(pattern, value))
        }
        (
            OraclePattern::Unit
            | OraclePattern::Bool(_)
            | OraclePattern::Tuple(_)
            | OraclePattern::Constructor { .. },
            _,
        ) => false,
    }
}

fn lower_oracle_pattern(
    pattern: &OraclePattern,
    ty: &TypeKind,
    coordinate: &StableSemanticCoordinate,
) -> DeconstructedPattern {
    let relative = StablePatternCoordinate::new([]);
    let kind = match (pattern, ty) {
        (OraclePattern::Any, _) => DeconstructedPatternKind::Wildcard,
        (OraclePattern::Unit, TypeKind::Unit) => DeconstructedPatternKind::Constructor {
            constructor: CoverageConstructorId::Unit,
            fields: Box::new([]),
        },
        (OraclePattern::Bool(value), TypeKind::Bool) => DeconstructedPatternKind::Constructor {
            constructor: CoverageConstructorId::Bool(*value),
            fields: Box::new([]),
        },
        (OraclePattern::Tuple(patterns), TypeKind::Tuple(types)) => {
            assert_eq!(patterns.len(), types.len());
            DeconstructedPatternKind::Constructor {
                constructor: CoverageConstructorId::Tuple {
                    owner: ty.semantic_identity_digest(),
                },
                fields: patterns
                    .iter()
                    .zip(types)
                    .map(|(pattern, ty)| lower_oracle_pattern(pattern, ty, coordinate))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            }
        }
        (OraclePattern::Constructor { .. }, _) => {
            panic!("family constructor requires its admitted domain")
        }
        _ => panic!("oracle pattern/type mismatch"),
    };
    DeconstructedPattern {
        kind,
        coordinate: relative,
        semantic_coordinate: coordinate.clone(),
    }
}

fn cartesian<T: Clone>(columns: &[Vec<T>]) -> Vec<Vec<T>> {
    let mut rows = vec![Vec::new()];
    for column in columns {
        let mut next = Vec::new();
        for row in &rows {
            for value in column {
                let mut expanded = row.clone();
                expanded.push(value.clone());
                next.push(expanded);
            }
        }
        rows = next;
    }
    rows
}

fn oracle_patterns_for_type(ty: &TypeKind) -> Vec<OraclePattern> {
    match ty {
        TypeKind::Unit => vec![OraclePattern::Any, OraclePattern::Unit],
        TypeKind::Bool => vec![
            OraclePattern::Any,
            OraclePattern::Bool(false),
            OraclePattern::Bool(true),
        ],
        TypeKind::Tuple(fields) => {
            let columns = fields
                .iter()
                .map(oracle_patterns_for_type)
                .collect::<Vec<_>>();
            let mut patterns = vec![OraclePattern::Any];
            patterns.extend(
                cartesian(&columns)
                    .into_iter()
                    .map(|fields| OraclePattern::Tuple(fields.into_boxed_slice())),
            );
            patterns
        }
        _ => vec![OraclePattern::Any],
    }
}

fn oracle_values_for_type(ty: &TypeKind) -> Vec<OracleValue> {
    match ty {
        TypeKind::Unit => vec![OracleValue::Unit],
        TypeKind::Bool => vec![OracleValue::Bool(false), OracleValue::Bool(true)],
        TypeKind::Tuple(fields) => {
            let columns = fields
                .iter()
                .map(oracle_values_for_type)
                .collect::<Vec<_>>();
            cartesian(&columns)
                .into_iter()
                .map(|fields| OracleValue::Tuple(fields.into_boxed_slice()))
                .collect()
        }
        _ => panic!("finite oracle has no scalar universe for {ty:?}"),
    }
}

fn family_patterns(constructors: &[CoverageConstructor]) -> Vec<OraclePattern> {
    let mut patterns = vec![OraclePattern::Any];
    for (index, constructor) in constructors.iter().enumerate() {
        let columns = constructor
            .field_types
            .iter()
            .map(oracle_patterns_for_type)
            .collect::<Vec<_>>();
        patterns.extend(
            cartesian(&columns)
                .into_iter()
                .map(|fields| OraclePattern::Constructor {
                    index,
                    fields: fields.into_boxed_slice(),
                }),
        );
    }
    patterns
}

fn family_values(constructors: &[CoverageConstructor]) -> Vec<OracleValue> {
    let mut values = Vec::new();
    for (index, constructor) in constructors.iter().enumerate() {
        let columns = constructor
            .field_types
            .iter()
            .map(oracle_values_for_type)
            .collect::<Vec<_>>();
        values.extend(
            cartesian(&columns)
                .into_iter()
                .map(|fields| OracleValue::Constructor {
                    index,
                    fields: fields.into_boxed_slice(),
                }),
        );
    }
    values
}

fn lower_family_pattern(
    pattern: &OraclePattern,
    ty: &TypeKind,
    constructors: &[CoverageConstructor],
    coordinate: &StableSemanticCoordinate,
) -> DeconstructedPattern {
    let OraclePattern::Constructor { index, fields } = pattern else {
        return lower_oracle_pattern(pattern, ty, coordinate);
    };
    let constructor = &constructors[*index];
    assert_eq!(fields.len(), constructor.field_types.len());
    DeconstructedPattern {
        kind: DeconstructedPatternKind::Constructor {
            constructor: constructor.identity.clone(),
            fields: fields
                .iter()
                .zip(constructor.field_types.iter())
                .map(|(pattern, ty)| lower_oracle_pattern(pattern, ty, coordinate))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
        coordinate: StablePatternCoordinate::new([]),
        semantic_coordinate: coordinate.clone(),
    }
}

fn oracle_matrices(patterns: &[OraclePattern]) -> Vec<Vec<OraclePattern>> {
    let mut matrices = vec![Vec::<OraclePattern>::new()];
    matrices.extend(patterns.iter().cloned().map(|pattern| vec![pattern]));
    for first in patterns {
        for second in patterns {
            matrices.push(vec![first.clone(), second.clone()]);
        }
    }
    matrices
}

fn assert_oracle_matrices<F>(
    analyzer: &mut MatchCoverageAnalyzer<'_, '_>,
    ty: &TypeKind,
    patterns: &[OraclePattern],
    values: &[OracleValue],
    label: &str,
    mut lower: F,
) where
    F: FnMut(&OraclePattern) -> DeconstructedPattern,
{
    for matrix in oracle_matrices(patterns) {
        for query in patterns {
            let expected = values.iter().any(|value| {
                oracle_matches(query, value)
                    && !matrix.iter().any(|covered| oracle_matches(covered, value))
            });
            let lowered_matrix = matrix
                .iter()
                .map(|pattern| vec![lower(pattern)])
                .collect::<Vec<_>>();
            let lowered_query = [lower(query)];
            let actual = analyzer
                .useful(
                    &lowered_matrix,
                    &lowered_query,
                    std::slice::from_ref(ty),
                    0,
                    &mut Vec::new(),
                )
                .expect("finite matrix usefulness")
                .is_some();
            assert_eq!(
                actual, expected,
                "family={label} type={ty:?} matrix={matrix:?} query={query:?}"
            );
        }
    }
}

fn assert_bool_and_tuple_oracle(
    analysis: &FinalSemanticAnalysis,
    module: &HirModule,
    coordinate: &StableSemanticCoordinate,
) {
    let bool_patterns = vec![
        OraclePattern::Any,
        OraclePattern::Bool(false),
        OraclePattern::Bool(true),
    ];
    let mut tuple_patterns = vec![OraclePattern::Any];
    for left in &bool_patterns {
        for right in &bool_patterns {
            tuple_patterns.push(OraclePattern::Tuple(
                vec![left.clone(), right.clone()].into_boxed_slice(),
            ));
        }
    }
    let cases = [
        (
            TypeKind::Bool,
            bool_patterns,
            vec![OracleValue::Bool(false), OracleValue::Bool(true)],
        ),
        (
            TypeKind::Tuple(vec![TypeKind::Bool, TypeKind::Bool]),
            tuple_patterns,
            vec![
                OracleValue::Tuple(
                    vec![OracleValue::Bool(false), OracleValue::Bool(false)].into_boxed_slice(),
                ),
                OracleValue::Tuple(
                    vec![OracleValue::Bool(false), OracleValue::Bool(true)].into_boxed_slice(),
                ),
                OracleValue::Tuple(
                    vec![OracleValue::Bool(true), OracleValue::Bool(false)].into_boxed_slice(),
                ),
                OracleValue::Tuple(
                    vec![OracleValue::Bool(true), OracleValue::Bool(true)].into_boxed_slice(),
                ),
            ],
        ),
    ];

    for (ty, patterns, values) in cases {
        let cancellation = AtomicBool::new(false);
        let limits = CheckedMatchLimits::PRODUCTION
            .with_limit(CheckedMatchLimitKind::MatrixRows, 1_000_000)
            .with_limit(CheckedMatchLimitKind::Specializations, 1_000_000)
            .with_limit(CheckedMatchLimitKind::WitnessNodes, 1_000_000);
        let mut budget = CheckedMatchBudget::new(limits);
        let mut analyzer = MatchCoverageAnalyzer::new(
            analysis,
            module,
            FinalSemanticAnalysisControl::new(&cancellation),
            &mut budget,
            coordinate.clone(),
            Vec::new(),
        );
        assert_oracle_matrices(
            &mut analyzer,
            &ty,
            &patterns,
            &values,
            "primitive/product",
            |pattern| lower_oracle_pattern(pattern, &ty, coordinate),
        );
    }
}

fn assert_family_oracle(
    analysis: &FinalSemanticAnalysis,
    module: &HirModule,
    coordinate: &StableSemanticCoordinate,
    project_enum: TypeKind,
) {
    let sequence = TypeKind::Vec(Box::new(TypeKind::Bool));
    let sequence_owner = sequence.semantic_identity_digest();
    let limits = CheckedMatchLimits::PRODUCTION
        .with_limit(CheckedMatchLimitKind::MatrixRows, 5_000_000)
        .with_limit(CheckedMatchLimitKind::Specializations, 5_000_000)
        .with_limit(CheckedMatchLimitKind::WitnessNodes, 5_000_000);
    let mut budget = CheckedMatchBudget::new(limits);
    let cancellation = AtomicBool::new(false);
    let mut analyzer = MatchCoverageAnalyzer::new(
        analysis,
        module,
        FinalSemanticAnalysisControl::new(&cancellation),
        &mut budget,
        coordinate.clone(),
        Vec::new(),
    );
    analyzer.domain_overrides.insert(
        sequence_owner,
        CoverageTypeDomain::Constructors(
            vec![
                CoverageConstructor {
                    identity: CoverageConstructorId::Sequence {
                        owner: sequence_owner,
                        partition: SequencePartition::Exact(0),
                    },
                    field_types: Box::new([]),
                    variant_payload: None,
                },
                CoverageConstructor {
                    identity: CoverageConstructorId::Sequence {
                        owner: sequence_owner,
                        partition: SequencePartition::Exact(1),
                    },
                    field_types: vec![TypeKind::Bool].into_boxed_slice(),
                    variant_payload: None,
                },
                CoverageConstructor {
                    identity: CoverageConstructorId::Sequence {
                        owner: sequence_owner,
                        partition: SequencePartition::Interval {
                            lower: 2,
                            upper_exclusive: None,
                        },
                    },
                    field_types: vec![TypeKind::Bool, TypeKind::Bool].into_boxed_slice(),
                    variant_payload: None,
                },
            ]
            .into_boxed_slice(),
        ),
    );
    let families = [
        ("Bool", TypeKind::Bool),
        ("Option", TypeKind::Option(Box::new(TypeKind::Bool))),
        (
            "Result",
            TypeKind::Result {
                ok: Box::new(TypeKind::Bool),
                error: Box::new(TypeKind::Unit),
            },
        ),
        ("project enum", project_enum),
        (
            "Choice",
            TypeKind::Choice(vec![TypeKind::Bool, TypeKind::Unit]),
        ),
        (
            "product",
            TypeKind::Tuple(vec![TypeKind::Bool, TypeKind::Bool]),
        ),
        (
            "array",
            TypeKind::Array {
                item: Box::new(TypeKind::Bool),
                len: ArrayLength::Const(2),
            },
        ),
        ("sequence", sequence),
    ];
    for (family, ty) in families {
        let CoverageTypeDomain::Constructors(constructors) = analyzer
            .domain(&ty, coordinate)
            .expect("finite family domain")
        else {
            panic!("{family} must be inhabited");
        };
        let patterns = family_patterns(&constructors);
        let values = family_values(&constructors);
        assert_oracle_matrices(&mut analyzer, &ty, &patterns, &values, family, |pattern| {
            lower_family_pattern(pattern, &ty, &constructors, coordinate)
        });
    }
}

#[test]
fn finite_pattern_oracle_agrees_with_matrix_usefulness() {
    let fixture = fixture(
        concat!(
            "enum OracleEnum {\n",
            "    Empty,\n",
            "    Full,\n",
            "}\n",
            "fn root(value: OracleEnum) -> i64 {\n",
            "    match value {\n",
            "        .Empty => 0i64\n",
            "        .Full => 1i64\n",
            "    }\n",
            "}\n",
        ),
        None,
    );
    let analysis = analyze(&fixture).expect("checked oracle fixture");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let match_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let coordinates = crate::semantic_coordinate::SemanticCoordinateIndex::new(
        analysis.accepted_root_catalog(),
        &analysis,
    );
    let coordinate = StableSemanticCoordinate::new(coordinates.expression(match_owner).unwrap());

    assert_bool_and_tuple_oracle(&analysis, module, &coordinate);

    let match_types = analysis
        .expressions()
        .filter_map(|(_, expression)| expression.match_fact())
        .filter_map(|fact| analysis.expression(fact.scrutinee()))
        .map(|expression| expression.ty().clone())
        .collect::<Vec<_>>();
    let project_enum = match_types
        .iter()
        .find(|&ty| matches!(ty, TypeKind::ProjectNominal(_)))
        .cloned()
        .unwrap_or_else(|| panic!("project enum parameter type: {match_types:?}"));
    assert_family_oracle(&analysis, module, &coordinate, project_enum);
}
