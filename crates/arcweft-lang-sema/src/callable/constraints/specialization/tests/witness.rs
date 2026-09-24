use super::*;
use crate::callable::{
    CheckedFunctionSpecialization, specialization::FunctionSpecializationSealFailure,
};
use crate::types::{TypeProjectionControl, TypeProjectionNodeKind};

#[derive(Debug, thiserror::Error)]
#[error("projection budget exhausted")]
struct Exhausted;

struct Budget {
    left: usize,
    visits: usize,
}

impl TypeProjectionControl for Budget {
    type Error = Exhausted;
    fn check(&mut self) -> Result<(), Exhausted> {
        if self.left == 0 {
            Err(Exhausted)
        } else {
            Ok(())
        }
    }
    fn visit_node(&mut self, _: TypeProjectionNodeKind, _: u64) -> Result<(), Exhausted> {
        self.check()?;
        self.left -= 1;
        self.visits += 1;
        Ok(())
    }
    fn visit_binding(&mut self) -> Result<(), Exhausted> {
        self.visit_node(TypeProjectionNodeKind::Type, 1)
    }
}

#[test]
fn witness_rejects_foreign_source_and_owner_and_charges_the_same_scoped_fold() {
    let graph = PreparedCallGraph::<()>::new();
    let source = scheme();
    let sources = [Source::ReturnedScheme(source.clone())];
    let cancellation = AtomicBool::new(false);
    let solved = drive(
        &graph,
        Client {
            graph: &graph,
            application_owners: Vec::new(),
            sources: &sources,
            observations: Arc::new(Mutex::new(Vec::new())),
            cancellation: &cancellation,
            cancel_during_probe: false,
            check_foreign_graph: false,
        },
        TypeConstraintParameterScope::empty(),
        &[concrete(TypeKind::I64, 4, &["fs.read"])],
        PRODUCTION_CALLABLE_LIMITS,
    )
    .unwrap();
    let application = solved
        .component
        .applications()
        .find_map(|(owner, _)| {
            matches!(owner, CallableConstraintApplication::Specialize(_)).then_some(owner)
        })
        .unwrap();
    let result = solved.component.projection(application, &0).unwrap();
    let mut full = Budget {
        left: usize::MAX,
        visits: 0,
    };
    let witness = CheckedFunctionSpecialization::seal(&result, &mut full).unwrap();
    assert!(full.visits > 0);
    let mut short = Budget {
        left: full.visits - 1,
        visits: 0,
    };
    assert!(matches!(
        CheckedFunctionSpecialization::seal(&result, &mut short),
        Err(FunctionSpecializationSealFailure::Projection(
            TypeProjectionError::Control(Exhausted)
        )),
    ));
    let mut exact = Budget {
        left: full.visits,
        visits: 0,
    };
    let replay = CheckedFunctionSpecialization::seal(&result, &mut exact).unwrap();
    assert_eq!(replay.digest(), witness.digest());
    let mut altered = source.clone();
    let TypeKind::Function { return_type, .. } = &mut altered else {
        unreachable!()
    };
    **return_type = TypeKind::Bool;
    let foreign_fact = crate::final_analysis::CheckedExpression::value(
        altered,
        crate::final_analysis::CheckedTypeSelection::Inferred,
        EffectSet::new(),
        crate::final_analysis::CheckedExpressionResolution::Call,
    );
    assert!(matches!(
        foreign_fact.with_function_specialization(witness.owner(), Arc::clone(&witness)),
        Err(CallConstraintInvariant::PreparedFunctionTypeMismatch),
    ));
    let origin = result.source().unwrap();
    assert!(matches!(
        CheckedFunctionSpecialization::seal(&origin, &mut full),
        Err(FunctionSpecializationSealFailure::Invariant(
            CallConstraintInvariant::PreparedCallSiteMismatch
        )),
    ));
    let fact = crate::final_analysis::CheckedExpression::value(
        source,
        crate::final_analysis::CheckedTypeSelection::Inferred,
        EffectSet::new(),
        crate::final_analysis::CheckedExpressionResolution::Call,
    );
    let foreign_owner = solved
        .component
        .applications()
        .find_map(|(owner, _)| {
            (owner.expression() != witness.owner()).then_some(owner.expression())
        })
        .unwrap();
    assert!(matches!(
        fact.clone()
            .with_function_specialization(foreign_owner, Arc::clone(&witness)),
        Err(CallConstraintInvariant::PreparedCallSiteMismatch)
    ));
    let specialized = fact
        .with_function_specialization(witness.owner(), Arc::clone(&witness))
        .unwrap();
    assert!(matches!(
        specialized.with_function_specialization(witness.owner(), witness),
        Err(CallConstraintInvariant::PreparedFunctionTypeMismatch)
    ));
}
