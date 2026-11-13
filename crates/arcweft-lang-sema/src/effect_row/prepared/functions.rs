//! Row functions share one cell inventory; inference is not a fake declaration.

use super::*;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct PreparedEffectUnknownRef(PreparedEffectReference);

impl PreparedEffectUnknownRef {
    pub(super) fn reference(&self) -> &PreparedEffectReference {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparedEffectUnknownEligibility {
    Bindable,
    Future,
}

#[derive(Clone, Debug)]
pub(super) enum FunctionRef {
    Scheme(PreparedEffectSchemeRef),
    Unknown(PreparedEffectUnknownRef),
    Imported(PreparedEffectImportedRef),
}

impl FunctionRef {
    pub(super) fn reference(&self) -> &PreparedEffectReference {
        match self {
            Self::Scheme(reference) => &reference.0,
            Self::Unknown(reference) => &reference.0,
            Self::Imported(reference) => &reference.0,
        }
    }
}

#[derive(Debug)]
pub(super) enum Function {
    Scheme(Scheme),
    Imported(completion::ImportedEffectFunction),
    /// Exactly the dependencies available when this unknown was introduced.
    Unknown {
        parameters: Box<[GenericEffectReference]>,
        eligibility: PreparedEffectUnknownEligibility,
    },
}

impl Function {
    pub(super) fn parameters(&self) -> &[GenericEffectReference] {
        match self {
            Self::Scheme(scheme) => &scheme.header.parameters,
            Self::Imported(imported) => imported.definition().parameters(),
            Self::Unknown { parameters, .. } => parameters,
        }
    }

    pub(super) fn domain(&self) -> Option<&EffectPredicate> {
        match self {
            Self::Scheme(scheme) => Some(&scheme.header.domain),
            Self::Imported(imported) => imported.definition().domain(),
            Self::Unknown { .. } => None,
        }
    }

    pub(super) fn is_future(&self) -> bool {
        matches!(
            self,
            Self::Unknown {
                eligibility: PreparedEffectUnknownEligibility::Future,
                ..
            }
        )
    }
}

impl PreparedEffectGraph {
    pub(super) fn validate_parameters<'p, C: DecisionControl>(
        parameters: &'p [GenericEffectReference],
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<&'p GenericEffectReference>, PreparedEffectFailure<C::Error>> {
        let mut unique = BTreeSet::new();
        for parameter in parameters {
            work.charge(DecisionWork::Visit)?;
            if !unique.insert(parameter) {
                return Err(PreparedEffectInvariant::DuplicateParameter.into());
            }
        }
        Ok(unique)
    }

    pub(super) fn insert_function<C: DecisionControl>(
        &mut self,
        function: Function,
        work: &mut Work<'_, C>,
    ) -> Result<u64, PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Node)?;
        let id = self.next_function;
        self.next_function = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.changed()?;
        self.functions.insert(id, Arc::new(function));
        Ok(id)
    }

    /// Unknowns have no declaration domain or body equation. Only bindable
    /// unknowns participate in this scope's joint minimization.
    pub(crate) fn declare_unknown<C: DecisionControl>(
        &mut self,
        dependencies: Vec<GenericEffectReference>,
        eligibility: PreparedEffectUnknownEligibility,
        control: &mut C,
    ) -> Result<PreparedEffectUnknownRef, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        Self::validate_parameters(&dependencies, &mut work)?;
        let id = self.insert_function(
            Function::Unknown {
                parameters: dependencies.into_boxed_slice(),
                eligibility,
            },
            &mut work,
        )?;
        Ok(PreparedEffectUnknownRef(self.reference(id)))
    }

    pub(super) fn function(
        &self,
        reference: &FunctionRef,
    ) -> Result<&Function, PreparedEffectInvariant> {
        let function = self
            .functions
            .get(&self.validate(reference.reference())?)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        match (reference, function) {
            (FunctionRef::Scheme(_), Function::Scheme(_))
            | (FunctionRef::Unknown(_), Function::Unknown { .. })
            | (FunctionRef::Imported(_), Function::Imported(_)) => Ok(function),
            _ => Err(PreparedEffectInvariant::StaleReference),
        }
    }

    pub(super) fn apply_function<C: DecisionControl>(
        &mut self,
        function: FunctionRef,
        arguments: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        if self.function(&function)?.parameters().len() != arguments.len() {
            return Err(PreparedEffectInvariant::ApplicationArity.into());
        }
        for row in &arguments {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            self.validate_row(row)?;
        }
        self.insert_row(
            RowTerm::Application {
                function,
                arguments: arguments.into_boxed_slice(),
            },
            control,
        )
    }

    pub(crate) fn unknown_application<C: DecisionControl>(
        &mut self,
        unknown: &PreparedEffectUnknownRef,
        arguments: Vec<PreparedEffectRow>,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.apply_function(FunctionRef::Unknown(unknown.clone()), arguments, control)
    }
}
