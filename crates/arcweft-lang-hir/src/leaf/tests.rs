use super::*;
use crate::identity::{
    HirDatabaseId, HirIdKind, HirModuleId, HirRevision, HirTypedId, RawHirId, SyntheticRole,
};
use core::num::{NonZeroU32, NonZeroU64};

fn database(value: u64) -> HirDatabaseId {
    HirDatabaseId::from_raw_for_test(NonZeroU64::new(value).expect("non-zero database"))
}

fn module(database: u64, slot: u32) -> HirModuleId {
    HirModuleId::new(
        self::database(database),
        NonZeroU32::new(slot).expect("non-zero module slot"),
    )
}

fn scope(module: HirModuleId, slot: u32) -> ScopeId {
    ScopeId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("non-zero scope slot"),
        HirIdKind::Scope,
    ))
}

fn type_id(module: HirModuleId, slot: u32) -> TypeId {
    TypeId::from_raw(RawHirId::new(
        module,
        NonZeroU32::new(slot).expect("non-zero type slot"),
        HirIdKind::Type,
    ))
}

fn name(value: &str) -> HirName {
    HirName::try_new(Box::<str>::from(value)).expect("valid test name")
}

fn big_uint(limbs: &[u32]) -> HirBigUint {
    HirBigUint::try_new(Box::<[u32]>::from(limbs)).expect("canonical test magnitude")
}

#[test]
fn names_preserve_validated_code_points_and_path_roots() {
    for valid in ["_", "alpha", "éclair", "名1"] {
        assert_eq!(name(valid).as_str(), valid);
    }
    for invalid in ["", "1alpha", "with-hyphen", "two words", "a/child"] {
        assert!(HirName::try_new(Box::<str>::from(invalid)).is_err());
    }

    for valid in ["project-1", "9patch", "外部_2"] {
        let segment = HirProjectSymbolSegment::try_new(Box::<str>::from(valid))
            .expect("valid external-capable segment");
        assert_eq!(segment.as_str(), valid);
    }
    for invalid in ["", "project/name", "project.name", "line\nfeed"] {
        assert!(HirProjectSymbolSegment::try_new(Box::<str>::from(invalid)).is_none());
    }

    assert_eq!(
        HirPath::try_new(HirPathRoot::Crate, Box::new([])),
        Err(HirPathIssue::Empty)
    );
    let path = HirPath::try_new(
        HirPathRoot::Super { depth: 0 },
        vec![
            HirPathSegment::Identifier(name("game")),
            HirPathSegment::ProjectSymbol(
                HirProjectSymbolSegment::try_new(Box::<str>::from("shared-assets"))
                    .expect("valid segment"),
            ),
        ]
        .into_boxed_slice(),
    )
    .expect("non-empty path");
    assert_eq!(path.root(), HirPathRoot::SelfModule);
    assert_eq!(path.segments().len(), 2);

    let explicit_super = HirPath::try_new(
        HirPathRoot::Super { depth: usize::MAX },
        vec![HirPathSegment::Identifier(name("root"))].into_boxed_slice(),
    )
    .expect("non-zero super depth is retained for resolution");
    assert_eq!(
        explicit_super.root(),
        HirPathRoot::Super { depth: usize::MAX }
    );
}

#[test]
fn path_resolution_context_rejects_a_foreign_scope() {
    let owner_module = module(1, 1);
    let snapshot = HirSnapshotId::new(owner_module, HirRevision::INITIAL);
    let owner_scope = scope(owner_module, 1);
    let context =
        HirPathResolutionContext::new(snapshot, owner_scope).expect("same-module scope resolves");
    assert_eq!(context.snapshot(), snapshot);
    assert_eq!(context.owner_scope(), owner_scope);

    assert_eq!(
        HirPathResolutionContext::new(snapshot, scope(module(1, 2), 1)),
        Err(HirPathIssue::ForeignScope)
    );
}

#[test]
fn entity_reference_family_preserves_every_relative_depth() {
    assert_eq!(
        HirEntityReference::try_new(Box::<str>::from("")),
        Err(HirIdRefInvariantError::EmptyAbsolute)
    );
    let absolute = HirEntityReference::try_new(Box::<str>::from("flow.opening@sem:abc"))
        .expect("non-empty absolute body");
    assert_eq!(absolute.as_str(), "flow.opening@sem:abc");

    assert_eq!(
        HirIdSuffix::try_new(Box::<str>::from("")),
        Err(HirIdRefInvariantError::EmptySuffix)
    );
    assert_eq!(
        HirIdSuffix::try_new(Box::<str>::from("@.missing")),
        Err(HirIdRefInvariantError::AuthoredRelativeMarker)
    );
    assert_eq!(
        HirIdSuffix::try_new(Box::<str>::from("next..line")),
        Err(HirIdRefInvariantError::InvalidSuffix)
    );
    assert_eq!(
        HirIdFamily::try_new(Box::<str>::from("invalid-family")),
        Err(HirIdRefInvariantError::InvalidFamily)
    );

    let family = HirIdFamily::try_new(Box::<str>::from("scene")).expect("valid family");
    assert_eq!(family.as_str(), "scene");
    for depth in [0, 1, usize::MAX] {
        let relative = HirRelativeId::new(
            HirIdSuffix::try_new(Box::<str>::from("next.line")).expect("valid suffix"),
            depth,
        );
        assert_eq!(relative.suffix().as_str(), "next.line");
        assert_eq!(relative.parent_depth(), depth);

        let family_relative = HirFamilyRelativeId::new(family.clone(), relative.clone());
        assert_eq!(family_relative.family(), &family);
        assert_eq!(family_relative.relative(), &relative);

        let complete = [
            HirIdRef::absolute(absolute.clone()),
            HirIdRef::relative(relative),
            HirIdRef::family_relative(family_relative),
        ];
        assert_eq!(complete.len(), 3);
    }
}

#[test]
fn recovered_leaf_values_preserve_typed_shapes_without_dummy_semantics() {
    let id_recovery = HirIdRefRecovery::new(
        HirIdRefShape::FamilyRelative {
            parent_depth: 2,
            suffix_segment_count: 3,
        },
        HirIdRefIssue::Invalid(HirIdRefInvariantError::InvalidSuffix),
    );
    let id = HirIdRefValue::Recovered(id_recovery);
    assert!(id.as_resolved().is_none());
    assert_eq!(id.recovery(), Some(&id_recovery));
    assert_eq!(id.recovery_issue(), Some(id_recovery.issue()));

    let expression = crate::expr::HirExprKind::EntityReference(id.clone());
    let pattern = crate::pattern::HirPatternKind::EntityReference(id);
    assert!(matches!(
        (expression, pattern),
        (
            crate::expr::HirExprKind::EntityReference(HirIdRefValue::Recovered(_)),
            crate::pattern::HirPatternKind::EntityReference(HirIdRefValue::Recovered(_)),
        )
    ));

    let path_recovery = HirPathRecovery::new(
        HirPathRoot::Super { depth: 0 },
        2,
        HirPathIssue::InvalidSegment { ordinal: 1 },
    );
    let path = HirPathValue::Recovered(path_recovery.clone());
    assert!(path.as_resolved().is_none());
    assert_eq!(path.recovery(), Some(&path_recovery));
    assert_eq!(path_recovery.root(), HirPathRoot::SelfModule);
    assert_eq!(path_recovery.segment_count(), 2);

    let lifetime_recovery = HirLifetimePathRecovery::new(
        true,
        2,
        true,
        HirLifetimeRegistryIssue::InvalidKeySegment { ordinal: 1 },
    );
    let lifetime = HirLifetimePathValue::Recovered(lifetime_recovery);
    assert!(lifetime.as_resolved().is_none());
    let retained = lifetime.recovery().expect("typed lifetime recovery");
    assert!(retained.scope_present());
    assert_eq!(retained.segment_count(), 2);
    assert!(retained.optional_marker());
    assert_eq!(
        retained.issue(),
        HirLifetimeRegistryIssue::InvalidKeySegment { ordinal: 1 }
    );

    let short = HirShortVariantName::Recovered(HirNameInvariantError::InvalidIdentifier);
    assert!(short.as_resolved().is_none());
    assert_eq!(
        short.recovery_issue(),
        Some(HirNameInvariantError::InvalidIdentifier)
    );
}

#[test]
fn type_regions_bind_elision_to_the_exact_type_owner() {
    let owner_module = module(2, 1);
    let first = type_id(owner_module, 1);
    let second = type_id(owner_module, 2);
    let key = SyntheticKey::try_new(SyntheticOwner::Type(first), SyntheticRole::ElidedRegion, 0)
        .expect("valid type-owned elision key");
    let region = HirElidedRegion::try_new(first, key).expect("matching type owner");
    assert_eq!(region.owner_type(), first);
    assert_eq!(region.key(), key);

    assert_eq!(
        HirElidedRegion::try_new(second, key),
        Err(HirElidedRegionError::OwnerMismatch {
            expected: second,
            actual: SyntheticOwner::Type(first),
        })
    );

    let named = HirRegionName::new(name("story"));
    assert_eq!(named.name().as_str(), "story");
    assert!(matches!(
        HirTypeRegion::named(named),
        HirTypeRegion::Named(_)
    ));
    assert!(matches!(
        HirTypeRegion::elided(region),
        HirTypeRegion::Elided(_)
    ));
}

#[test]
fn runtime_registry_paths_remain_separate_from_type_regions() {
    let builtin_scopes = [
        HirLifetimeRegistryScope::Frame,
        HirLifetimeRegistryScope::Tick,
        HirLifetimeRegistryScope::Cue,
        HirLifetimeRegistryScope::Line,
        HirLifetimeRegistryScope::Scene,
        HirLifetimeRegistryScope::Flow,
        HirLifetimeRegistryScope::Session,
        HirLifetimeRegistryScope::Global,
        HirLifetimeRegistryScope::Persistent,
    ];
    for scope in builtin_scopes {
        let path = HirLifetimeRegistryPath::try_new(scope.clone(), Box::new([]), false);
        assert_eq!(path.scope(), &scope);
        assert!(path.segments().is_empty());
        assert!(!path.optional());
    }

    let named_scope = HirLifetimeRegistryScope::Named(name("inventory"));
    let path = HirLifetimeRegistryPath::try_new(
        named_scope.clone(),
        vec![name("party"), name("gold")].into_boxed_slice(),
        true,
    );
    assert_eq!(path.scope(), &named_scope);
    assert_eq!(
        path.segments()
            .iter()
            .map(HirName::as_str)
            .collect::<Vec<_>>(),
        ["party", "gold"]
    );
    assert!(path.optional());

    let modes = [
        HirLifetimeRegistryAccessMode::Read,
        HirLifetimeRegistryAccessMode::Write,
        HirLifetimeRegistryAccessMode::MoveOut,
        HirLifetimeRegistryAccessMode::Drop,
        HirLifetimeRegistryAccessMode::Expose,
    ];
    assert_eq!(modes.len(), 5);
}

#[test]
fn arbitrary_integer_and_decimal_carriers_are_canonical() {
    let zero = big_uint(&[]);
    assert!(zero.is_zero());
    assert!(zero.limbs_le().is_empty());

    let beyond_u128 = big_uint(&[0, 0, 0, 0, 1]);
    assert_eq!(beyond_u128.limbs_le(), [0, 0, 0, 0, 1]);
    assert!(!beyond_u128.is_zero());
    for invalid in [vec![0], vec![1, 0], vec![u32::MAX, 0]] {
        assert!(HirBigUint::try_new(invalid.into_boxed_slice()).is_none());
    }

    for valid in [vec![0], vec![1], vec![1, 0, 2]] {
        assert!(HirDecimalDigits::try_new(valid.into_boxed_slice()).is_some());
    }
    for invalid in [Vec::new(), vec![0, 0], vec![0, 1], vec![1, 0], vec![10]] {
        assert!(HirDecimalDigits::try_new(invalid.into_boxed_slice()).is_none());
    }

    let coefficient =
        HirDecimalDigits::try_new(vec![1, 2, 3].into_boxed_slice()).expect("canonical digits");
    let decimal =
        HirDecimal::try_new(coefficient, 65_536, -1_000_000).expect("exact decimal limits");
    assert_eq!(decimal.coefficient().digits(), [1, 2, 3]);
    assert_eq!(decimal.scale(), 65_536);
    assert_eq!(decimal.exponent10(), -1_000_000);

    let literal = HirIntegerLiteral::Value {
        magnitude: beyond_u128,
        radix: HirIntegerRadix::Hexadecimal,
        suffix: Some(HirIntegerSuffix::USize),
    };
    assert!(matches!(literal, HirIntegerLiteral::Value { .. }));
}

#[test]
fn decimal_constructor_enforces_every_closed_limit_and_canonical_zero() {
    let coefficient = |count: usize| {
        HirDecimalDigits::try_new(vec![1; count].into_boxed_slice())
            .expect("all-one coefficient is canonical")
    };
    assert!(HirDecimal::try_new(coefficient(65_536), 65_536, 1_000_000).is_ok());
    assert_eq!(
        HirDecimal::try_new(coefficient(65_537), 0, 0),
        Err(HirDecimalInvariantError::CoefficientDigits {
            observed: 65_537,
            maximum: 65_536,
        })
    );
    assert_eq!(
        HirDecimal::try_new(coefficient(1), 65_537, 0),
        Err(HirDecimalInvariantError::Scale {
            observed: 65_537,
            maximum: 65_536,
        })
    );
    assert_eq!(
        HirDecimal::try_new(coefficient(1), 0, 1_000_001),
        Err(HirDecimalInvariantError::ExponentAbs {
            observed: 1_000_001,
            maximum: 1_000_000,
        })
    );
    let zero = HirDecimalDigits::try_new(Box::new([0])).expect("canonical zero coefficient");
    assert_eq!(
        HirDecimal::try_new(zero, 1, 0),
        Err(HirDecimalInvariantError::NonCanonicalZero)
    );
}

#[test]
fn floats_and_units_retain_exact_typed_values_without_narrowing() {
    let digits = HirDecimalDigits::try_new(vec![5].into_boxed_slice()).expect("canonical digit");
    let decimal = HirDecimal::try_new(digits, 1, 0).expect("canonical decimal");
    let float = HirFloatLiteral::Value {
        decimal: decimal.clone(),
        explicit_width: Some(HirFloatWidth::F32),
    };
    assert!(matches!(
        float,
        HirFloatLiteral::Value {
            explicit_width: Some(HirFloatWidth::F32),
            ..
        }
    ));

    let checked_f32 = CheckedFloatLiteral::new(HirFloatBits::F32(0x7f7f_ffff));
    let checked_f64 = CheckedFloatLiteral::new(HirFloatBits::F64(0x7fef_ffff_ffff_ffff));
    assert_eq!(checked_f32.bits(), HirFloatBits::F32(0x7f7f_ffff));
    assert_eq!(checked_f64.bits(), HirFloatBits::F64(0x7fef_ffff_ffff_ffff));

    let units = [
        HirUnitNumberUnit::Percent,
        HirUnitNumberUnit::Px,
        HirUnitNumberUnit::Pt,
        HirUnitNumberUnit::Em,
        HirUnitNumberUnit::Rem,
        HirUnitNumberUnit::Vw,
        HirUnitNumberUnit::Vh,
        HirUnitNumberUnit::Deg,
        HirUnitNumberUnit::Rad,
        HirUnitNumberUnit::Turn,
        HirUnitNumberUnit::Db,
        HirUnitNumberUnit::Lufs,
        HirUnitNumberUnit::Bpm,
        HirUnitNumberUnit::Bars,
    ];
    for unit in units {
        assert!(matches!(
            HirUnitNumberLiteral::Value {
                decimal: decimal.clone(),
                unit,
            },
            HirUnitNumberLiteral::Value { .. }
        ));
    }
}

#[test]
fn duration_structural_identity_retains_authored_unit() {
    let semantic = HirDurationSemanticValue::try_new(big_uint(&[1_000_000_000]));
    let one_second = HirDurationValue::new(semantic.clone(), HirDurationUnit::Seconds);
    let thousand_millis = HirDurationValue::new(semantic, HirDurationUnit::Millis);

    assert_ne!(one_second, thousand_millis);
    assert_eq!(
        one_second.semantic_value(),
        thousand_millis.semantic_value()
    );
    assert_eq!(one_second.authored_unit(), HirDurationUnit::Seconds);
    assert_eq!(thousand_millis.authored_unit(), HirDurationUnit::Millis);
    assert_eq!(
        one_second.semantic_value().nanoseconds().limbs_le(),
        [1_000_000_000]
    );
}

#[test]
fn compact_numeric_sequence_keeps_idless_order_and_typed_recovery() {
    let elements = vec![
        HirNumericSequenceElement::new(big_uint(&[1]), HirIntegerRadix::Decimal),
        HirNumericSequenceElement::new(big_uint(&[0, 0, 0, 0, 1]), HirIntegerRadix::Hexadecimal),
    ];
    let sequence = HirNumericSequence::try_new(
        elements.into_boxed_slice(),
        Some(HirIntegerSuffix::U128),
        HirNumericSequenceRecovery::Complete,
    )
    .expect("complete compact sequence");
    assert_eq!(sequence.elements().len(), 2);
    assert_eq!(sequence.elements()[0].magnitude().limbs_le(), [1]);
    assert_eq!(sequence.elements()[1].radix(), HirIntegerRadix::Hexadecimal);
    assert_eq!(sequence.common_suffix(), Some(HirIntegerSuffix::U128));
    assert_eq!(sequence.recovery(), &HirNumericSequenceRecovery::Complete);

    let missing = HirNumericSequence::try_new(
        Box::new([]),
        None,
        HirNumericSequenceRecovery::MissingFinalElement { ordinal: 0 },
    )
    .expect("missing final element owns the next source ordinal");
    assert_eq!(
        missing.recovery(),
        &HirNumericSequenceRecovery::MissingFinalElement { ordinal: 0 }
    );
    assert_eq!(missing.source_element_count(), 1);
    let invalid = HirNumericSequenceRecovery::InvalidElement {
        ordinal: 7,
        issue: HirIntegerIssue::InvalidDigit,
    };
    let conflicting = HirNumericSequenceRecovery::ConflictingSuffix {
        ordinal: 8,
        first: HirIntegerSuffix::I32,
        conflicting: HirIntegerSuffix::U32,
    };
    assert_ne!(invalid, conflicting);

    assert_eq!(
        HirNumericSequence::try_new(
            Box::new([]),
            None,
            HirNumericSequenceRecovery::MissingFinalElement { ordinal: 1 },
        ),
        Err(HirNumericSequenceInvariantError::MissingFinalOrdinal {
            expected: 0,
            actual: 1,
        })
    );
    assert_eq!(
        HirNumericSequence::try_new(
            Box::new([]),
            None,
            HirNumericSequenceRecovery::InvalidElement {
                ordinal: 1,
                issue: HirIntegerIssue::InvalidDigit,
            },
        ),
        Err(HirNumericSequenceInvariantError::InvalidElementOrdinal {
            maximum: 0,
            actual: 1,
        })
    );
    assert_eq!(
        HirNumericSequence::try_new(
            Box::new([]),
            None,
            HirNumericSequenceRecovery::ConflictingSuffix {
                ordinal: 0,
                first: HirIntegerSuffix::I32,
                conflicting: HirIntegerSuffix::U32,
            },
        ),
        Err(HirNumericSequenceInvariantError::ConflictingSuffixOrdinal {
            retained_len: 0,
            actual: 0,
        })
    );
}

#[test]
fn literal_poison_families_do_not_fabricate_values() {
    let string = HirLiteral::String(HirStringLiteral::Invalid(HirStringIssue::Unterminated));
    let character = HirLiteral::Character(HirCharacterLiteral::Invalid(
        HirCharacterIssue::MultipleScalars,
    ));
    let integer = HirLiteral::Integer(HirIntegerLiteral::Invalid(HirIntegerIssue::MissingDigits));
    let float = HirLiteral::Float(HirFloatLiteral::Invalid(HirFloatIssue::Decimal(
        HirDecimalIssue::InvalidDigit,
    )));
    let unit = HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(
        HirUnitNumberIssue::InvalidUnit,
    ));
    let duration = HirLiteral::Duration(HirDurationLiteral::Invalid(
        HirDurationIssue::FractionalNanosecond,
    ));
    let boolean = HirLiteral::Boolean(true);

    assert!(matches!(
        string,
        HirLiteral::String(HirStringLiteral::Invalid(_))
    ));
    assert!(matches!(
        character,
        HirLiteral::Character(HirCharacterLiteral::Invalid(_))
    ));
    assert!(matches!(
        integer,
        HirLiteral::Integer(HirIntegerLiteral::Invalid(_))
    ));
    assert!(matches!(
        float,
        HirLiteral::Float(HirFloatLiteral::Invalid(_))
    ));
    assert!(matches!(
        unit,
        HirLiteral::UnitNumber(HirUnitNumberLiteral::Invalid(_))
    ));
    assert!(matches!(
        duration,
        HirLiteral::Duration(HirDurationLiteral::Invalid(_))
    ));
    assert_eq!(boolean, HirLiteral::Boolean(true));

    assert_eq!(
        HirLiteralIssue::Float(HirFloatIssue::InvalidSuffix).to_string(),
        "float literal has an invalid suffix"
    );
    assert_eq!(
        HirLiteralIssue::Duration(HirDurationIssue::FractionalNanosecond).to_string(),
        "Duration literal has a fractional nanosecond"
    );
}

#[test]
fn closed_path_and_region_issue_inventories_remain_exhaustive() {
    let segment = HirProjectSymbolSegment::try_new(Box::<str>::from("external-project"))
        .expect("valid project segment");
    let path_roots = [
        HirPathRoot::ImplicitCrate,
        HirPathRoot::Crate,
        HirPathRoot::SelfModule,
        HirPathRoot::Super { depth: 1 },
    ];
    assert_eq!(path_roots.len(), 4);

    let path_issues = [
        HirPathIssue::Empty,
        HirPathIssue::InvalidSegment { ordinal: 0 },
        HirPathIssue::SuperEscapesCrate {
            depth: 2,
            available: 1,
        },
        HirPathIssue::UnknownAlias {
            segment: segment.clone(),
        },
        HirPathIssue::AmbiguousAlias {
            segment: segment.clone(),
        },
        HirPathIssue::UnknownExternalProject { segment },
        HirPathIssue::UnpublishedTarget,
        HirPathIssue::StaleSnapshot,
        HirPathIssue::ForeignScope,
    ];
    assert_eq!(path_issues.len(), 9);

    let region_issues = [
        HirTypeRegionIssue::InvalidNamedRegion,
        HirTypeRegionIssue::InvalidElisionOwner,
    ];
    assert_eq!(region_issues.len(), 2);
    let registry_issues = [
        HirLifetimeRegistryIssue::InvalidNamedScope,
        HirLifetimeRegistryIssue::InvalidKeySegment { ordinal: 0 },
        HirLifetimeRegistryIssue::OptionalNonReadAccess,
        HirLifetimeRegistryIssue::MissingScope,
    ];
    assert_eq!(registry_issues.len(), 4);
}

#[test]
fn closed_literal_scalar_inventories_remain_exhaustive() {
    assert!(matches!(
        HirStringLiteral::Value(Box::<str>::from("decoded")),
        HirStringLiteral::Value(_)
    ));
    assert_eq!(
        HirCharacterLiteral::Value('語'),
        HirCharacterLiteral::Value('語')
    );

    let radices = [
        HirIntegerRadix::Binary,
        HirIntegerRadix::Octal,
        HirIntegerRadix::Decimal,
        HirIntegerRadix::Hexadecimal,
    ];
    assert_eq!(radices.len(), 4);
    let suffixes = [
        HirIntegerSuffix::I8,
        HirIntegerSuffix::I16,
        HirIntegerSuffix::I32,
        HirIntegerSuffix::I64,
        HirIntegerSuffix::I128,
        HirIntegerSuffix::ISize,
        HirIntegerSuffix::U8,
        HirIntegerSuffix::U16,
        HirIntegerSuffix::U32,
        HirIntegerSuffix::U64,
        HirIntegerSuffix::U128,
        HirIntegerSuffix::USize,
    ];
    assert_eq!(suffixes.len(), 12);
    assert_ne!(HirFloatWidth::F32, HirFloatWidth::F64);

    let duration_units = [
        HirDurationUnit::Nanos,
        HirDurationUnit::Micros,
        HirDurationUnit::Millis,
        HirDurationUnit::Seconds,
        HirDurationUnit::Minutes,
        HirDurationUnit::Hours,
    ];
    assert_eq!(duration_units.len(), 6);
    let duration = HirDurationValue::new(
        HirDurationSemanticValue::try_new(big_uint(&[])),
        HirDurationUnit::Nanos,
    );
    assert!(matches!(
        HirDurationLiteral::Value(duration),
        HirDurationLiteral::Value(_)
    ));
}

#[test]
fn closed_literal_issue_inventories_remain_exhaustive() {
    let string_issues = [HirStringIssue::InvalidEscape, HirStringIssue::Unterminated];
    let character_issues = [
        HirCharacterIssue::InvalidEscape,
        HirCharacterIssue::Unterminated,
        HirCharacterIssue::Empty,
        HirCharacterIssue::MultipleScalars,
    ];
    let integer_issues = [
        HirIntegerIssue::MissingDigits,
        HirIntegerIssue::InvalidDigit,
    ];
    let decimal_issues = [
        HirDecimalIssue::MissingCoefficient,
        HirDecimalIssue::InvalidDigit,
    ];
    assert_eq!(string_issues.len(), 2);
    assert_eq!(character_issues.len(), 4);
    assert_eq!(integer_issues.len(), 2);
    assert_eq!(decimal_issues.len(), 2);

    let float_issues = [
        HirFloatIssue::Decimal(HirDecimalIssue::InvalidDigit),
        HirFloatIssue::NonFinite,
        HirFloatIssue::InvalidSuffix,
    ];
    let unit_issues = [
        HirUnitNumberIssue::Decimal(HirDecimalIssue::InvalidDigit),
        HirUnitNumberIssue::InvalidUnit,
    ];
    let duration_issues = [
        HirDurationIssue::Decimal(HirDecimalIssue::InvalidDigit),
        HirDurationIssue::InvalidUnit,
        HirDurationIssue::FractionalNanosecond,
    ];
    assert_eq!(float_issues.len(), 3);
    assert_eq!(unit_issues.len(), 2);
    assert_eq!(duration_issues.len(), 3);

    let aggregate = [
        HirLiteralIssue::String(HirStringIssue::InvalidEscape),
        HirLiteralIssue::Character(HirCharacterIssue::Empty),
        HirLiteralIssue::Integer(HirIntegerIssue::InvalidDigit),
        HirLiteralIssue::Float(HirFloatIssue::NonFinite),
        HirLiteralIssue::UnitNumber(HirUnitNumberIssue::InvalidUnit),
        HirLiteralIssue::Duration(HirDurationIssue::InvalidUnit),
    ];
    assert_eq!(aggregate.len(), 6);
}
