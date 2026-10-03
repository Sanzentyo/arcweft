//! Typed local-input projection and collection for checked executable roots.

use std::collections::BTreeSet;

use arcweft_lang_hir::{
    identity::{ExprId, LocalId, StmtId},
    scope::CaptureAccess,
};

use crate::{
    semantic_coordinate::{
        CheckedSemanticPath, SemanticCoordinateIndex, StableCheckedBindingCoordinate,
    },
    types::{SemanticTypeDigest, TypeKind},
};

use super::{
    CheckedExecutableCapture, CheckedExpression, CheckedExpressionResolution,
    CheckedRecordValueSource, FinalSemanticAnalysisError, PreparedExpressionFact,
    match_edges::CheckedStructuralEdgeDraft,
};

/// One executable use, including HIR shorthand/capture positions without an
/// independent expression node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CheckedLocalUseSite {
    Expression(ExprId),
    Place(ExprId),
    RecordField { owner: ExprId, source_ordinal: u32 },
    Capture { owner: ExprId, local: LocalId },
    StatementCapture { owner: StmtId, local: LocalId },
}

impl CheckedLocalUseSite {
    pub const fn owner(self) -> arcweft_lang_hir::body_edges::HirBodyChild {
        match self {
            Self::Expression(owner)
            | Self::Place(owner)
            | Self::RecordField { owner, .. }
            | Self::Capture { owner, .. } => {
                arcweft_lang_hir::body_edges::HirBodyChild::Expression(owner)
            }
            Self::StatementCapture { owner, .. } => {
                arcweft_lang_hir::body_edges::HirBodyChild::Statement(owner)
            }
        }
    }

    pub(crate) const fn expression_owner(self) -> Option<ExprId> {
        match self.owner() {
            arcweft_lang_hir::body_edges::HirBodyChild::Expression(owner) => Some(owner),
            arcweft_lang_hir::body_edges::HirBodyChild::Statement(_) => None,
        }
    }
}

impl From<ExprId> for CheckedLocalUseSite {
    fn from(owner: ExprId) -> Self {
        Self::Expression(owner)
    }
}

/// One expression's intrinsic input sources, projected from its owning checked
/// facts. These are transaction-local values, never a second stored read index.
pub(super) struct CheckedCaptureExpression {
    sources: Vec<CheckedLocalInputSource>,
    record_sources: Option<Vec<super::prepared::PreparedRecordValueSource>>,
    descends: bool,
}

#[derive(Clone)]
pub(super) struct CheckedLocalInputSource {
    site: CheckedLocalUseSite,
    local: LocalId,
    ty: SemanticTypeDigest,
    origin: Option<StableCheckedBindingCoordinate>,
    access: CaptureAccess,
}

impl CheckedLocalInputSource {
    pub(super) const fn site(&self) -> CheckedLocalUseSite {
        self.site
    }
    pub(super) const fn local(&self) -> LocalId {
        self.local
    }
    pub(super) const fn access(&self) -> CaptureAccess {
        self.access
    }
    pub(super) const fn value_type(&self) -> SemanticTypeDigest {
        self.ty
    }

    pub(super) fn source_order(
        &self,
        topology: &arcweft_lang_hir::project::HirProjectEvaluationTopology,
        record: impl Fn(ExprId) -> Option<Vec<super::prepared::PreparedRecordValueSource>>,
    ) -> Result<(u32, u8, u32), FinalSemanticAnalysisError> {
        let owner = self
            .site
            .expression_owner()
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        let index = topology
            .module(owner.module())
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?
            .expression_uses();
        let row = index
            .row(owner)
            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
        match self.site {
            CheckedLocalUseSite::RecordField { source_ordinal, .. } => {
                let fields = record(owner).ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let ordinal = usize::try_from(source_ordinal)
                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if fields.get(ordinal)
                    != Some(&super::prepared::PreparedRecordValueSource::Local(
                        self.local,
                    ))
                {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
                let position = fields[..ordinal]
                    .iter()
                    .rev()
                    .find_map(|field| match field {
                        super::prepared::PreparedRecordValueSource::Expression(child) => {
                            Some(*child)
                        }
                        super::prepared::PreparedRecordValueSource::Local(_) => None,
                    })
                    .map(|child| {
                        index
                            .row(child)
                            .map(|row| row.subtree_end_ordinal())
                            .ok_or(FinalSemanticAnalysisError::InvalidOwner)
                    })
                    .transpose()?
                    .unwrap_or(
                        row.source_ordinal()
                            .checked_add(1)
                            .ok_or(FinalSemanticAnalysisError::InvalidOwner)?,
                    );
                Ok((position, 0, source_ordinal))
            }
            _ => Ok((row.source_ordinal(), 1, 0)),
        }
    }
}

/// Projects the exact HIR-authenticated implicit region's selected facts.
/// HIR supplies membership and traversal positions, never local resolution.
pub(super) fn implicit_region_inputs(
    topology: &arcweft_lang_hir::project::HirProjectEvaluationTopology,
    owner: ExprId,
    expression: impl Fn(
        ExprId,
        CaptureAccess,
    ) -> Result<CheckedCaptureExpression, FinalSemanticAnalysisError>,
    record: impl Fn(ExprId) -> Option<Vec<super::prepared::PreparedRecordValueSource>>,
) -> Result<Vec<CheckedLocalInputSource>, FinalSemanticAnalysisError> {
    let module = topology
        .module(owner.module())
        .ok_or(FinalSemanticAnalysisError::InvalidOwner)?;
    let region = module
        .expression_uses()
        .implicit_callable_region(owner)
        .map_err(|_| FinalSemanticAnalysisError::InvalidOwner)?;
    let mut ordered = Vec::new();
    for row in module
        .expression_uses()
        .rows()
        .iter()
        .filter(|row| region.contains_expression(row.expression()))
    {
        let inputs = expression(row.expression(), row.capture_access())?;
        for input in inputs.sources {
            // A finalized root's creation packet belongs to its enclosing
            // execution, not to this body's occurrences.
            if matches!(input.site, CheckedLocalUseSite::Capture { owner: capture_owner, .. } if capture_owner == owner)
            {
                continue;
            }
            let binding = module.local_origins().binding(input.local).ok_or(
                FinalSemanticAnalysisError::CaptureAuthority {
                    violation: super::CheckedCaptureAuthorityViolation::MissingLocalBinding {
                        local: input.local,
                    },
                },
            )?;
            if !region.contains_binding(binding) {
                ordered.push((input.source_order(topology, &record)?, input));
            }
        }
    }
    ordered.sort_by_key(|(order, _)| *order);
    Ok(ordered.into_iter().map(|(_, input)| input).collect())
}

impl CheckedCaptureExpression {
    /// Closes the already authenticated sources without reconstructing their
    /// selection or origin. This temporary map serves only the requested root.
    pub(super) fn close_input_types(
        mut self,
        context: &super::CheckedClosedExecutionContext<'_>,
        types: &mut std::collections::BTreeMap<LocalId, TypeKind>,
    ) -> Result<Self, super::CheckedExecutionContextError> {
        for source in &mut self.sources {
            let original = context
                .analysis()
                .local(source.local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable {
                    owner: source.local,
                })?
                .ty();
            if original
                .semantic_identity_digest()
                .map_err(FinalSemanticAnalysisError::from)?
                != source.ty
            {
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily.into());
            }
            let closed = match types.entry(source.local) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(context.instantiate_type(original)?)
                }
            };
            source.ty = context
                .environment()
                .semantic_type_identity(closed)
                .map_err(FinalSemanticAnalysisError::from)?;
        }
        Ok(self)
    }

    pub(super) fn from_place(
        owner: ExprId,
        place: &super::CheckedMutablePlace,
        ty: &TypeKind,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        Ok(Self {
            sources: vec![CheckedLocalInputSource {
                site: CheckedLocalUseSite::Place(owner),
                local: place.local_id(),
                ty: ty.semantic_identity_digest()?,
                origin: None,
                access: CaptureAccess::Reassign,
            }],
            record_sources: None,
            descends: false,
        })
    }
    pub(super) fn from_checked(
        owner: ExprId,
        checked: &CheckedExpression,
        record_fields: &[super::CheckedExpressionRecordField],
        local_type: impl Fn(LocalId) -> Option<TypeKind>,
        source: impl Fn(ExprId) -> Option<super::CheckedFieldReceiver>,
        access: CaptureAccess,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut value = Self {
            sources: Vec::new(),
            record_sources: matches!(
                checked.resolution(),
                CheckedExpressionResolution::Nominal(_)
            )
            .then(|| {
                record_fields
                    .iter()
                    .map(|field| field.slot().source())
                    .collect()
            }),
            descends: !matches!(
                checked.resolution(),
                CheckedExpressionResolution::Closure(_)
                    | CheckedExpressionResolution::ImplicitCallable(_)
            ),
        };
        if let Some(local) = checked.execution_local_use() {
            value.push_typed(
                CheckedLocalUseSite::Expression(owner),
                local,
                checked.source_value_type(),
                access,
            )?;
        }
        if let Some(local) = checked.field_root(&source) {
            let ty = local_type(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
            value.push_typed(
                CheckedLocalUseSite::Expression(owner),
                local,
                Some(&ty),
                access,
            )?;
            value.descends = false;
        }
        for field in record_fields {
            if let CheckedRecordValueSource::Binding(source) = field.source() {
                value.sources.push(CheckedLocalInputSource {
                    site: CheckedLocalUseSite::RecordField {
                        owner,
                        source_ordinal: field.source_ordinal(),
                    },
                    local: source.raw(),
                    ty: field.field_type(),
                    origin: Some(source.coordinate().clone()),
                    access: CaptureAccess::Read,
                });
            }
        }
        value.include_resolution(owner, checked.resolution(), local_type)?;
        Ok(value)
    }

    pub(super) fn from_prepared(
        owner: ExprId,
        checked: &PreparedExpressionFact,
        local_type: impl Fn(LocalId) -> Option<TypeKind>,
        source: impl Fn(ExprId) -> Option<super::CheckedFieldReceiver>,
        access: CaptureAccess,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        if let PreparedExpressionFact::Complete(checked) = checked {
            if matches!(
                checked.resolution(),
                CheckedExpressionResolution::Nominal(_)
            ) {
                // Records remain ProjectRecord until their C1 slot plan is
                // joined to C2. A completed record requires that final plan.
                return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
            }
            return Self::from_checked(owner, checked, &[], local_type, source, access);
        }
        let mut value = Self {
            sources: Vec::new(),
            record_sources: None,
            descends: true,
        };
        if let Some(local) = checked.execution_local_use() {
            value.push_typed(
                CheckedLocalUseSite::Expression(owner),
                local,
                checked.value_type(),
                access,
            )?;
        }
        if let PreparedExpressionFact::ProjectRecord(record) = checked {
            value.record_sources =
                Some(record.fields().iter().map(|field| field.source()).collect());
            for field in record.fields() {
                if let super::prepared::PreparedRecordValueSource::Local(local) = field.source() {
                    value.push_typed(
                        CheckedLocalUseSite::RecordField {
                            owner,
                            source_ordinal: field.source_ordinal(),
                        },
                        local,
                        Some(field.field_type()),
                        CaptureAccess::Read,
                    )?;
                }
            }
        }
        if let Some(local) = checked.field_root(source) {
            let ty = local_type(local)
                .ok_or(FinalSemanticAnalysisError::LocalTypeUnavailable { owner: local })?;
            value.push_typed(
                CheckedLocalUseSite::Expression(owner),
                local,
                Some(&ty),
                access,
            )?;
            value.descends = false;
        }
        if let Some(resolution) = checked.checked_resolution() {
            value.include_resolution(owner, resolution, local_type)?;
        }
        Ok(value)
    }

    pub(super) fn from_statement(
        owner: StmtId,
        checked: &super::CheckedStatement,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let mut value = Self {
            sources: Vec::new(),
            record_sources: None,
            descends: false,
        };
        if let super::CheckedStatementPayload::Defer(defer) = checked.payload() {
            for capture in defer.captures() {
                value.sources.push(CheckedLocalInputSource {
                    site: CheckedLocalUseSite::StatementCapture {
                        owner,
                        local: capture.local(),
                    },
                    local: capture.local(),
                    ty: capture.ty().semantic_identity_digest()?,
                    origin: Some(capture.origin().clone()),
                    access: CaptureAccess::Read,
                });
            }
        }
        Ok(value)
    }

    fn push_typed(
        &mut self,
        site: CheckedLocalUseSite,
        local: LocalId,
        ty: Option<&TypeKind>,
        access: CaptureAccess,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let ty = ty.ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
        self.sources.push(CheckedLocalInputSource {
            site,
            local,
            ty: ty.semantic_identity_digest()?,
            origin: None,
            access,
        });
        Ok(())
    }

    fn include_resolution(
        &mut self,
        owner: ExprId,
        resolution: &CheckedExpressionResolution,
        local_type: impl Fn(LocalId) -> Option<TypeKind>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        match resolution {
            CheckedExpressionResolution::Closure(closure) => {
                for capture in closure.captures() {
                    let ty = local_type(capture.local()).ok_or(
                        FinalSemanticAnalysisError::LocalTypeUnavailable {
                            owner: capture.local(),
                        },
                    )?;
                    self.push_typed(
                        CheckedLocalUseSite::Capture {
                            owner,
                            local: capture.local(),
                        },
                        capture.local(),
                        Some(&ty),
                        capture.mode(),
                    )?;
                }
            }
            CheckedExpressionResolution::ImplicitCallable(callable) => {
                for capture in callable.captures() {
                    self.sources.push(CheckedLocalInputSource {
                        site: CheckedLocalUseSite::Capture {
                            owner,
                            local: capture.lookup_local(),
                        },
                        local: capture.lookup_local(),
                        ty: capture.value_type(),
                        origin: Some(capture.origin().clone()),
                        access: capture.mode(),
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn sources(&self) -> impl Iterator<Item = &CheckedLocalInputSource> + '_ {
        self.sources.iter()
    }

    pub(super) fn record_source_at(&self, source_ordinal: u32) -> Option<&CheckedLocalInputSource> {
        self.sources
            .binary_search_by_key(&source_ordinal, |source| match source.site {
                CheckedLocalUseSite::RecordField { source_ordinal, .. } => source_ordinal,
                _ => u32::MAX,
            })
            .ok()
            .map(|index| &self.sources[index])
    }
}

enum CheckedLocalInputStep {
    Expression(ExprId),
    Input(CheckedLocalInputSource),
}

/// Walks the selected expression authority, interleaving record values and
/// shorthand inputs in authored field order. Creation captures stop latent
/// bodies; the root's caller determines whether an implicit body is executed.
pub(super) fn ordered_checked_local_inputs(
    root: ExprId,
    expression: impl Fn(ExprId) -> Result<CheckedCaptureExpression, FinalSemanticAnalysisError>,
    children: impl Fn(ExprId) -> Result<Vec<ExprId>, FinalSemanticAnalysisError>,
    admitted: impl Fn(ExprId) -> bool,
) -> Result<Vec<CheckedLocalInputSource>, FinalSemanticAnalysisError> {
    let mut pending = vec![CheckedLocalInputStep::Expression(root)];
    let mut visited = BTreeSet::new();
    let mut inputs = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            CheckedLocalInputStep::Input(input) => inputs.push(input),
            CheckedLocalInputStep::Expression(owner) => {
                if !admitted(owner) || !visited.insert(owner) {
                    continue;
                }
                let projected = expression(owner)?;
                if let Some(fields) = &projected.record_sources {
                    let mut ordered = Vec::with_capacity(fields.len());
                    let mut binding_count = 0;
                    for (ordinal, field) in fields.iter().enumerate() {
                        ordered.push(match *field {
                            super::prepared::PreparedRecordValueSource::Expression(child) => {
                                CheckedLocalInputStep::Expression(child)
                            }
                            super::prepared::PreparedRecordValueSource::Local(local) => {
                                let ordinal = u32::try_from(ordinal)
                                    .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
                                let source = projected
                                    .record_source_at(ordinal)
                                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                                if source.local != local {
                                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                                }
                                binding_count += 1;
                                CheckedLocalInputStep::Input(source.clone())
                            }
                        });
                    }
                    if binding_count != projected.sources.len() {
                        return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                    }
                    pending.extend(ordered.into_iter().rev());
                } else {
                    if projected.descends {
                        pending.extend(
                            children(owner)?
                                .into_iter()
                                .rev()
                                .map(CheckedLocalInputStep::Expression),
                        );
                    }
                    pending.extend(
                        projected
                            .sources
                            .into_iter()
                            .rev()
                            .map(CheckedLocalInputStep::Input),
                    );
                }
            }
        }
    }
    Ok(inputs)
}

/// Issues free-input evidence from one exact root and its selected inventory.
/// The same collector consumes a callback traversal or an eager execution fold.
pub(super) struct CheckedFreeLocalCollector<'a, 'coordinate, F> {
    root: CheckedSemanticPath,
    type_scope: crate::types::GenericScope,
    coordinates: &'a SemanticCoordinateIndex<'coordinate, 'coordinate>,
    local_type: F,
    captured: BTreeSet<LocalId>,
    captures: Vec<CheckedExecutableCapture>,
}

impl<'a, 'coordinate, F: Fn(LocalId) -> Option<TypeKind>>
    CheckedFreeLocalCollector<'a, 'coordinate, F>
{
    pub(super) fn new(
        root: ExprId,
        coordinates: &'a SemanticCoordinateIndex<'coordinate, 'coordinate>,
        local_type: F,
    ) -> Result<Self, FinalSemanticAnalysisError> {
        let root = coordinates
            .expression_evidence(root)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?
            .into_coordinate();
        Ok(Self::at_path(
            root,
            coordinates,
            local_type,
            crate::types::GenericScope::default(),
        ))
    }

    pub(super) fn at_path(
        root: CheckedSemanticPath,
        coordinates: &'a SemanticCoordinateIndex<'coordinate, 'coordinate>,
        local_type: F,
        type_scope: crate::types::GenericScope,
    ) -> Self {
        Self {
            root,
            type_scope,
            coordinates,
            local_type,
            captured: BTreeSet::new(),
            captures: Vec::new(),
        }
    }

    pub(super) fn include(
        &mut self,
        inputs: CheckedCaptureExpression,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.include_sources(inputs.sources)
    }

    /// Projects the same authenticated free sources for an executable input
    /// ABI. Consumers retain occurrences; the collector alone decides which
    /// bindings are external to the root and authenticates their type/origin.
    pub(super) fn include_with_free_sources(
        &mut self,
        inputs: CheckedCaptureExpression,
        mut free: impl FnMut(CheckedLocalInputSource),
    ) -> Result<(), FinalSemanticAnalysisError> {
        for source in inputs.sources {
            if self.include_source(&source)? {
                free(source);
            }
        }
        Ok(())
    }

    fn include_sources(
        &mut self,
        sources: Vec<CheckedLocalInputSource>,
    ) -> Result<(), FinalSemanticAnalysisError> {
        for source in sources {
            self.include_source(&source)?;
        }
        Ok(())
    }

    fn include_source(
        &mut self,
        source: &CheckedLocalInputSource,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        let accepted = (self.local_type)(source.local).ok_or(
            FinalSemanticAnalysisError::LocalTypeUnavailable {
                owner: source.local,
            },
        )?;
        let origin = self
            .coordinates
            .binding(source.local)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?;
        if accepted
            .semantic_identity_digest()
            .or_else(|_| accepted.semantic_identity_digest_in_scope(&self.type_scope))?
            != source.ty
            || source
                .origin
                .as_ref()
                .is_some_and(|expected| expected != &origin)
        {
            return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
        }
        if origin.path().is_at_or_below(&self.root) {
            return Ok(false);
        }
        if self.captured.insert(source.local) {
            self.captures.push(CheckedExecutableCapture::new(
                source.local,
                origin,
                accepted,
            ));
        }
        Ok(true)
    }

    pub(super) fn finish(self) -> Box<[CheckedExecutableCapture]> {
        self.captures.into_boxed_slice()
    }
}

/// Collects a callback body's inputs through the one selected edge authority.
pub(super) fn collect_checked_free_locals(
    root: ExprId,
    coordinates: &SemanticCoordinateIndex<'_, '_>,
    structural_edges: &CheckedStructuralEdgeDraft,
    expression: impl Fn(ExprId) -> Result<CheckedCaptureExpression, FinalSemanticAnalysisError>,
    local_type: impl Fn(LocalId) -> Option<TypeKind>,
) -> Result<Box<[CheckedExecutableCapture]>, FinalSemanticAnalysisError> {
    let mut collector = CheckedFreeLocalCollector::new(root, coordinates, local_type)?;
    let sources = ordered_checked_local_inputs(
        root,
        expression,
        |owner| {
            structural_edges
                .free_capture_children(owner)
                .map(|children| children.collect())
                .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)
        },
        |_| true,
    )?;
    collector.include_sources(sources)?;
    Ok(collector.finish())
}

impl super::FinalSemanticAnalysis {
    pub(super) fn checked_capture_inputs(
        &self,
        owner: ExprId,
    ) -> Result<CheckedCaptureExpression, FinalSemanticAnalysisError> {
        let checked = self
            .expression(owner)
            .ok_or(FinalSemanticAnalysisError::ExpressionTypeUnavailable { owner })?;
        if let CheckedExpressionResolution::Closure(closure) = checked.resolution() {
            for capture in closure.captures() {
                let capture_type = self
                    .capture(capture.capture())
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                let binding = self
                    .local(capture.local())
                    .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?;
                if capture_type.ty() != binding.ty() {
                    return Err(FinalSemanticAnalysisError::WrongPayloadFamily);
                }
            }
        }
        let fields = self
            .checked_expression_edge_fact(owner)
            .map_err(|_| FinalSemanticAnalysisError::WrongPayloadFamily)?
            .record_fields();
        let access = self
            .hir_topology()
            .module(owner.module())
            .and_then(|module| module.expression_uses().row(owner))
            .ok_or(FinalSemanticAnalysisError::WrongPayloadFamily)?
            .capture_access();
        CheckedCaptureExpression::from_checked(
            owner,
            checked,
            fields,
            |local| self.local(local).map(|binding| binding.ty().clone()),
            |child| {
                self.expression(child)
                    .and_then(CheckedExpression::local_place_source)
            },
            access,
        )
    }
}
