use std::convert::Infallible;

use arcweft_lang_hir::{
    expr::HirExprKind,
    pattern::{HirPatternBinding, HirPatternKind},
};

use super::*;
use crate::{
    effect_row::{
        DecisionControl, DecisionWork, EffectPredicate,
        prepared::{PreparedEffectGraph, PreparedEffectPredicate},
    },
    final_analysis::{
        CheckedTypeSelection, CheckedValueResolution, prepared::PreparedResolvedExpression,
    },
    types::GenericBinder,
};

struct Setup;

impl DecisionControl for Setup {
    type Error = Infallible;

    fn charge(&mut self, _: DecisionWork) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[test]
fn local_use_retains_a_prepared_callback_before_its_effect_body_exists() {
    let fixture =
        crate::final_analysis::tests::fixture("fn owner(value: i64) -> i64 { value }\n", None);
    let (_, module) = fixture
        .project
        .analysis_view()
        .unwrap()
        .modules()
        .next()
        .unwrap();
    let local = module
        .patterns()
        .find_map(|(_, pattern)| match pattern.kind() {
            HirPatternKind::Binding(HirPatternBinding::Bound { local, .. }) => Some(*local),
            _ => None,
        })
        .unwrap();
    let owner = module
        .expressions()
        .find_map(|(owner, expr)| matches!(expr.kind(), HirExprKind::Path(_)).then_some(owner))
        .unwrap();
    let mut effects = PreparedEffectGraph::default();
    let declaration = effects
        .declare(vec![], EffectPredicate::unconstrained(), &mut Setup)
        .unwrap();
    let terminal = effects
        .application(&declaration, vec![], &mut Setup)
        .unwrap();
    let callback = TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [TypeKind::I64],
        TypeKind::I64,
        terminal,
    );
    let fact = PreparedResolvedExpression::value(
        callback,
        CheckedTypeSelection::Inferred,
        crate::effects::EffectSet::new(),
        CheckedExpressionResolution::Value(CheckedValueResolution::Local(local)),
    )
    .into();
    let captures = CheckedCaptureExpression::from_prepared(
        owner,
        &fact,
        |_| None,
        |_| None,
        CaptureAccess::Read,
    )
    .unwrap();
    let [capture] = captures.sources.as_slice() else {
        panic!("one qualified direct local use");
    };
    let PreparedCaptureType::Source(source, ty) = &capture.ty else {
        panic!("capture type must still refer to the original graph");
    };
    assert_eq!(*source, owner.into());
    assert_eq!(capture.site(), CheckedLocalUseSite::Expression(owner));
    assert_eq!(capture.local(), local);
    assert!(std::ptr::eq(*ty, fact.source_value_type().unwrap()));
}
