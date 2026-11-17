//! The mathematical relation contains no mutation or publication capability.
//! Live preparation and immutable completion read this same definition owner.

use super::*;

#[derive(Debug)]
pub(in crate::effect_row::prepared) struct PreparedEffectDefinitions {
    pub(in crate::effect_row::prepared) issuer: Arc<PreparedEffectIssuer>,
    pub(in crate::effect_row::prepared) choices: BTreeSet<u64>,
    pub(in crate::effect_row::prepared) functions: BTreeMap<u64, Arc<Function>>,
    pub(in crate::effect_row::prepared) obligations: Vec<Arc<ScopedObligation>>,
    pub(in crate::effect_row::prepared) rows: BTreeMap<u64, Arc<RowTerm>>,
    pub(in crate::effect_row::prepared) predicates: BTreeMap<u64, Arc<PredicateTerm>>,
}

impl PreparedEffectDefinitions {
    /// Keep the exact immutable definitions behind a verified closure. Copy
    /// only the owned indexes, sharing each original definition and charging
    /// before every retained entry. Later preparation cannot change this input.
    pub(super) fn retain<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<Self>, PreparedEffectFailure<C::Error>> {
        fn index<T, C: DecisionControl>(
            input: &BTreeMap<u64, Arc<T>>,
            work: &mut Work<'_, C>,
        ) -> Result<BTreeMap<u64, Arc<T>>, PreparedEffectFailure<C::Error>> {
            let mut retained = BTreeMap::new();
            for (&id, value) in input {
                work.charge(DecisionWork::Node)?;
                retained.insert(id, Arc::clone(value));
            }
            Ok(retained)
        }
        let mut choices = BTreeSet::new();
        for choice in &self.choices {
            work.charge(DecisionWork::Node)?;
            choices.insert(*choice);
        }
        let mut obligations = Vec::new();
        for obligation in &self.obligations {
            work.charge(DecisionWork::Node)?;
            obligations.push(Arc::clone(obligation));
        }
        let functions = index(&self.functions, work)?;
        let rows = index(&self.rows, work)?;
        let predicates = index(&self.predicates, work)?;
        work.charge(DecisionWork::Node)?;
        Ok(Arc::new(Self {
            issuer: Arc::clone(&self.issuer),
            choices,
            functions,
            obligations,
            rows,
            predicates,
        }))
    }

    /// Original relation identity, rather than logical equivalence. Separate
    /// freezes can share the same source definitions; an issuer alone cannot
    /// identify them after more local constraints have been prepared.
    pub(super) fn same_original<C: DecisionControl>(
        &self,
        other: &Self,
        control: &mut C,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        work.charge(DecisionWork::Visit)?;
        if !Arc::ptr_eq(&self.issuer, &other.issuer)
            || self.functions.len() != other.functions.len()
            || self.obligations.len() != other.obligations.len()
            || self.choices.len() != other.choices.len()
        {
            return Ok(false);
        }
        for ((left_id, left), (right_id, right)) in self.functions.iter().zip(&other.functions) {
            work.charge(DecisionWork::Visit)?;
            if left_id != right_id {
                return Ok(false);
            }
            let same = match (left.as_ref(), right.as_ref()) {
                (Function::SchemeInput(left), Function::SchemeInput(right)) => left == right,
                _ => Arc::ptr_eq(left, right),
            };
            if !same {
                return Ok(false);
            }
        }
        for (left, right) in self.obligations.iter().zip(&other.obligations) {
            work.charge(DecisionWork::Visit)?;
            if !Arc::ptr_eq(left, right) {
                return Ok(false);
            }
        }
        for (left, right) in self.choices.iter().zip(&other.choices) {
            work.charge(DecisionWork::Visit)?;
            if left != right {
                return Ok(false);
            }
        }
        Ok(Self::same_terms(&self.rows, &other.rows, &mut work)?
            && Self::same_terms(&self.predicates, &other.predicates, &mut work)?)
    }

    fn same_terms<T, C: DecisionControl>(
        left: &BTreeMap<u64, Arc<T>>,
        right: &BTreeMap<u64, Arc<T>>,
        work: &mut Work<'_, C>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        if left.len() != right.len() {
            return Ok(false);
        }
        for ((left_id, left), (right_id, right)) in left.iter().zip(right) {
            work.charge(DecisionWork::Visit)?;
            if left_id != right_id || !Arc::ptr_eq(left, right) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn reference(&self, ordinal: u64) -> PreparedEffectReference {
        PreparedEffectReference {
            issuer: Arc::clone(&self.issuer),
            ordinal,
        }
    }

    pub(super) fn validate_choice(
        &self,
        choice: &PreparedEffectChoice,
    ) -> Result<(), PreparedEffectInvariant> {
        self.read().validate_choice(choice)
    }

    pub(super) fn empty() -> Self {
        Self {
            issuer: Arc::new(PreparedEffectIssuer),
            choices: BTreeSet::new(),
            functions: BTreeMap::new(),
            obligations: Vec::new(),
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
    }

    pub(super) fn validate(
        &self,
        reference: &PreparedEffectReference,
    ) -> Result<u64, PreparedEffectInvariant> {
        if !Arc::ptr_eq(&self.issuer, &reference.issuer) {
            return Err(PreparedEffectInvariant::ForeignIssuer);
        }
        Ok(reference.ordinal)
    }

    pub(super) fn function(
        &self,
        reference: &FunctionRef,
    ) -> Result<&Function, PreparedEffectInvariant> {
        self.read().function(reference)
    }

    pub(super) fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&Scheme, PreparedEffectInvariant> {
        self.read().scheme(reference)
    }

    pub(super) fn row(
        &self,
        reference: &PreparedEffectRowRef,
    ) -> Result<&RowTerm, PreparedEffectInvariant> {
        self.read().row(reference)
    }

    pub(super) fn predicate(
        &self,
        reference: &PreparedEffectPredicateRef,
    ) -> Result<&PredicateTerm, PreparedEffectInvariant> {
        self.read().predicate(reference)
    }
}
