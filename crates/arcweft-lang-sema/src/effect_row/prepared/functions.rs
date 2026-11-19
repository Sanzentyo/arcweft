//! Row functions share one cell inventory; inference is not a fake declaration.

use super::*;

/// Opaque authority for the actual lexical opening that introduced a Γ
/// parameter. The graph retains identity, never a source-layer resolver.
pub(crate) trait PreparedEffectParameterScope: std::fmt::Debug + Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn comparison_work(&self) -> usize;
    fn same_scope(&self, other: &dyn PreparedEffectParameterScope) -> bool;
}

#[derive(Debug)]
pub(super) struct ParameterOrigin {
    variable: GenericEffectReference,
    scope: Arc<dyn PreparedEffectParameterScope>,
}

impl ParameterOrigin {
    pub(super) fn variable(&self) -> &GenericEffectReference {
        &self.variable
    }

    pub(super) fn same_origin<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        for _ in 0..self.scope.comparison_work() {
            work.charge(DecisionWork::Visit)?;
        }
        Ok(self.variable == other.variable && self.scope.same_scope(other.scope.as_ref()))
    }
}

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
    /// The original named declaration is free in a frozen source relation.
    /// Its body state never becomes a local minimization variable.
    SchemeInput(PreparedEffectDeclaration),
    Imported(completion::ImportedEffectFunction),
    /// Exactly the dependencies available when this unknown was introduced.
    Unknown {
        parameters: Box<[GenericEffectReference]>,
        eligibility: PreparedEffectUnknownEligibility,
        origin: Option<ParameterOrigin>,
    },
}

impl Function {
    pub(super) fn parameters(&self) -> &[GenericEffectReference] {
        match self {
            Self::Scheme(scheme) => &scheme.header.parameters,
            Self::SchemeInput(declaration) => declaration.parameters(),
            Self::Imported(imported) => imported.definition().parameters(),
            Self::Unknown { parameters, .. } => parameters,
        }
    }

    pub(super) fn domain(&self) -> Option<&EffectPredicate> {
        match self {
            Self::Scheme(scheme) => Some(&scheme.header.domain),
            Self::SchemeInput(declaration) => Some(declaration.domain()),
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

    pub(super) fn is_input(&self) -> bool {
        self.is_future() || matches!(self, Self::SchemeInput(_))
    }
}

impl PreparedEffectDefinitions {
    /// Recover a retained unknown's operational handle from this exact frozen
    /// inventory. This validates the role without issuing a new identity.
    pub(super) fn unknown_reference(
        &self,
        id: u64,
    ) -> Result<PreparedEffectUnknownRef, PreparedEffectInvariant> {
        match self.functions.get(&id).map(Arc::as_ref) {
            Some(Function::Unknown { .. }) => Ok(PreparedEffectUnknownRef(self.reference(id))),
            _ => Err(PreparedEffectInvariant::StaleReference),
        }
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
        let next = id
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::OrdinalExhausted)?;
        self.changed(work.0)?;
        self.next_function = next;
        self.publish_function(id, Arc::new(function));
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
                origin: None,
            },
            &mut work,
        )?;
        Ok(PreparedEffectUnknownRef(self.reference(id)))
    }

    /// Register an actual opened Γ variable once. Repeated admissions of that
    /// opening share its graph function; a changed scope/signature is an error.
    /// Lookup happens only at declaration. Consumers retain the issued handle,
    /// so a rolled-back handle cannot resolve to a later declaration by name.
    pub(crate) fn declare_parameter<C: DecisionControl>(
        &mut self,
        variable: GenericEffectReference,
        scope: Arc<dyn PreparedEffectParameterScope>,
        dependencies: Vec<GenericEffectReference>,
        eligibility: PreparedEffectUnknownEligibility,
        control: &mut C,
    ) -> Result<PreparedEffectUnknownRef, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let unique = Self::validate_parameters(&dependencies, &mut work)?;
        if !matches!(variable, GenericEffectReference::Inference(_)) || unique.contains(&variable) {
            return Err(PreparedEffectInvariant::ParameterContract.into());
        }
        let mut existing = None;
        let read = self.read();
        for id in read.visible_function_ids() {
            work.charge(DecisionWork::Visit)?;
            let definition = read
                .function_definition(id)
                .ok_or(PreparedEffectInvariant::StaleReference)?;
            let Function::Unknown {
                parameters,
                eligibility: previous_eligibility,
                origin: Some(previous),
            } = definition.as_ref()
            else {
                continue;
            };
            if previous.variable != variable {
                continue;
            }
            // Bill the signature comparison before inspecting its elements.
            for _ in parameters.iter() {
                work.charge(DecisionWork::Visit)?;
            }
            for _ in 0..previous.scope.comparison_work() {
                work.charge(DecisionWork::Visit)?;
            }
            if !previous.scope.same_scope(scope.as_ref())
                || parameters.as_ref() != dependencies.as_slice()
                || previous_eligibility != &eligibility
                || existing.is_some_and(|previous_id| previous_id != id)
            {
                return Err(PreparedEffectInvariant::ParameterContract.into());
            }
            existing = Some(id);
        }
        if let Some(id) = existing {
            return Ok(PreparedEffectUnknownRef(self.reference(id)));
        }
        let id = self.insert_function(
            Function::Unknown {
                parameters: dependencies.into_boxed_slice(),
                eligibility,
                origin: Some(ParameterOrigin { variable, scope }),
            },
            &mut work,
        )?;
        Ok(PreparedEffectUnknownRef(self.reference(id)))
    }

    pub(super) fn function(
        &self,
        reference: &FunctionRef,
    ) -> Result<&Function, PreparedEffectInvariant> {
        self.read().function(reference)
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
