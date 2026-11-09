//! Prove that a receiver type can retain a borrow through its value storage.

use std::collections::{BTreeMap, BTreeSet, btree_map::Entry};

use arcweft_lang_hir::{
    identity::TypeId,
    symbol::{
        ProjectSymbolTable,
        nominal::{
            ProjectNominalBody, ProjectNominalDeclaration, ProjectNominalDeclarationId,
            ProjectNominalVariantPayload,
        },
    },
};
use arcweft_source::SourceSpan;

use crate::{
    final_analysis::{FinalSemanticAnalysisControl, FinalSemanticAnalysisError},
    nominal::NominalResolutionLimits,
    types::{ArrayLength, GenericParameterOwnerId, TypeKind},
};

/// A declaration either stores a borrow itself or can store the values of
/// certain type parameters. These dependencies are independent of any one
/// instantiation and remain finite even when recursive arguments grow.
#[derive(Debug, Default, Eq, PartialEq)]
struct StoredBorrow {
    direct: bool,
    parameters: BTreeSet<u16>,
}

impl StoredBorrow {
    fn include(&mut self, other: Self) {
        if other.direct {
            self.direct = true;
            self.parameters.clear();
        } else if !self.direct {
            self.parameters.extend(other.parameters);
        }
    }
}

#[derive(Default)]
struct DeclarationBorrow {
    stored: StoredBorrow,
    dependents: BTreeSet<ProjectNominalDeclarationId>,
}

pub(super) struct ReceiverBorrowQuery<'a> {
    symbols: &'a ProjectSymbolTable,
    types: &'a BTreeMap<TypeId, TypeKind>,
    control: FinalSemanticAnalysisControl<'a>,
    source_span: &'a SourceSpan,
    remaining: u64,
    declarations: BTreeMap<ProjectNominalDeclarationId, DeclarationBorrow>,
    pending: BTreeSet<ProjectNominalDeclarationId>,
}

impl<'a> ReceiverBorrowQuery<'a> {
    pub(super) fn new(
        symbols: &'a ProjectSymbolTable,
        types: &'a BTreeMap<TypeId, TypeKind>,
        control: FinalSemanticAnalysisControl<'a>,
        source_span: &'a SourceSpan,
    ) -> Self {
        Self {
            symbols,
            types,
            control,
            source_span,
            remaining: NominalResolutionLimits::PRODUCTION.work_per_reference(),
            declarations: BTreeMap::new(),
            pending: BTreeSet::new(),
        }
    }

    pub(super) fn contains_borrow(
        mut self,
        root: &TypeKind,
    ) -> Result<bool, FinalSemanticAnalysisError> {
        loop {
            if self.inspect(root, None, 1)?.direct {
                return Ok(true);
            }
            if self.pending.is_empty() {
                return Ok(false);
            }
            // Each summary grows toward more retained parameters or a direct
            // borrow. Revisit only declarations that consumed a changed summary.
            // Reevaluating the root can discover newly retained arguments.
            while let Some(id) = self.pending.pop_first() {
                self.update_declaration(&id)?;
            }
        }
    }

    fn update_declaration(
        &mut self,
        id: &ProjectNominalDeclarationId,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.charge(1)?;
        let declaration = self
            .symbols
            .nominal(id)
            .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
        let computed = self.inspect_declaration(declaration)?;
        let row = self
            .declarations
            .get_mut(id)
            .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
        if row.stored != computed {
            row.stored = computed;
            let edges = u64::try_from(row.dependents.len()).map_err(|_| self.limit())?;
            self.charge(edges)?;
            self.pending
                .extend(self.declarations[id].dependents.iter().cloned());
        }
        Ok(())
    }

    fn charge(&mut self, work: u64) -> Result<(), FinalSemanticAnalysisError> {
        self.control.check()?;
        self.remaining = self
            .remaining
            .checked_sub(work)
            .ok_or_else(|| self.limit())?;
        Ok(())
    }

    fn limit(&self) -> FinalSemanticAnalysisError {
        FinalSemanticAnalysisError::LifetimePromotionReceiverLimit {
            source_span: self.source_span.clone(),
        }
    }

    fn inspect_declaration(
        &mut self,
        declaration: &ProjectNominalDeclaration,
    ) -> Result<StoredBorrow, FinalSemanticAnalysisError> {
        let owner = declaration.id();
        let mut stored = StoredBorrow::default();
        match declaration.body() {
            ProjectNominalBody::Struct { fields } => {
                for field in fields {
                    self.include_member(&mut stored, owner, field.ty())?;
                }
            }
            ProjectNominalBody::Enum { variants } => {
                for variant in variants {
                    self.charge(1)?;
                    match variant.payload() {
                        ProjectNominalVariantPayload::Unit => {}
                        ProjectNominalVariantPayload::Tuple(ty) => {
                            self.include_member(&mut stored, owner, *ty)?;
                        }
                        ProjectNominalVariantPayload::Record(fields) => {
                            for field in fields {
                                self.include_member(&mut stored, owner, field.ty())?;
                            }
                        }
                    }
                }
            }
            ProjectNominalBody::TypeAlias { .. } => {
                return Err(FinalSemanticAnalysisError::InvalidNominalOwner);
            }
        }
        Ok(stored)
    }

    fn include_member(
        &mut self,
        stored: &mut StoredBorrow,
        owner: &ProjectNominalDeclarationId,
        ty: TypeId,
    ) -> Result<(), FinalSemanticAnalysisError> {
        self.charge(1)?;
        if !stored.direct {
            let ty = self
                .types
                .get(&ty)
                .ok_or(FinalSemanticAnalysisError::TypeResolutionFailed { owner: ty })?;
            stored.include(self.inspect(ty, Some(owner), 1)?);
        }
        Ok(())
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one exhaustive type storage projection"
    )]
    fn inspect(
        &mut self,
        ty: &TypeKind,
        owner: Option<&ProjectNominalDeclarationId>,
        depth: u16,
    ) -> Result<StoredBorrow, FinalSemanticAnalysisError> {
        self.charge(1)?;
        if depth > NominalResolutionLimits::PRODUCTION.recursive_type_depth() {
            return Err(self.limit());
        }
        let child = depth + 1;
        let mut stored = StoredBorrow::default();
        match ty {
            TypeKind::BorrowRef { .. } | TypeKind::Slice(_) => stored.direct = true,
            TypeKind::GenericParam(reference) => {
                if let Some(parameter) = reference.free_parameter()
                    && matches!(parameter.owner(), GenericParameterOwnerId::Nominal(id) if Some(id) == owner)
                {
                    stored.parameters.insert(parameter.ordinal());
                }
            }
            TypeKind::ProjectNominal(nominal) => {
                let id = nominal.declaration();
                let row = match self.declarations.entry(id.clone()) {
                    Entry::Occupied(entry) => entry.into_mut(),
                    Entry::Vacant(entry) => {
                        self.pending.insert(id.clone());
                        entry.insert(DeclarationBorrow::default())
                    }
                };
                if let Some(owner) = owner {
                    row.dependents.insert(owner.clone());
                }
                if row.stored.direct {
                    stored.direct = true;
                } else {
                    let parameters = row.stored.parameters.iter().copied().collect::<Vec<_>>();
                    for ordinal in parameters {
                        let argument = nominal
                            .arguments()
                            .get(usize::from(ordinal))
                            .ok_or(FinalSemanticAnalysisError::InvalidNominalOwner)?;
                        stored.include(self.inspect(argument, owner, child)?);
                        if stored.direct {
                            break;
                        }
                    }
                }
            }
            TypeKind::Array { item, len } => {
                if !matches!(len, ArrayLength::Const(0)) {
                    stored = self.inspect(item, owner, child)?;
                }
            }
            TypeKind::Option(item)
            | TypeKind::Range(item)
            | TypeKind::Shared(item)
            | TypeKind::Traversal { source: item, .. }
            | TypeKind::Vec(item) => stored = self.inspect(item, owner, child)?,
            TypeKind::FixedVector(vector) => {
                stored = self.inspect(vector.component(), owner, child)?;
            }
            TypeKind::Map {
                key: first,
                value: second,
                ..
            }
            | TypeKind::Result {
                ok: first,
                error: second,
            } => {
                stored = self.inspect(first, owner, child)?;
                if !stored.direct {
                    stored.include(self.inspect(second, owner, child)?);
                }
            }
            TypeKind::Choice(items) | TypeKind::Tuple(items) => {
                for item in items {
                    stored.include(self.inspect(item, owner, child)?);
                    if stored.direct {
                        break;
                    }
                }
            }
            TypeKind::VariantPayload(payload) => {
                for field in payload.stored_fields() {
                    stored.include(self.inspect(field, owner, child)?);
                    if stored.direct {
                        break;
                    }
                }
            }
            // Result/signature types, handles and opaque types do not describe
            // their stored captures or resources. Their owner must supply that
            // evidence before an executable promotion can be admitted.
            TypeKind::Function { .. }
            | TypeKind::Seq(_)
            | TypeKind::Stream { .. }
            | TypeKind::Parser { .. }
            | TypeKind::Need(_)
            | TypeKind::ThreadHandle(_)
            | TypeKind::Probe(_)
            | TypeKind::Handle { .. }
            | TypeKind::IteratorState { .. }
            | TypeKind::AcceptedNominal(_)
            | TypeKind::OpenNominal(_)
            | TypeKind::Named(_)
            | TypeKind::Projection { .. }
            | TypeKind::DialogueLine(_)
            | TypeKind::SlotRef(_)
            | TypeKind::Ref(_)
            | TypeKind::MetaType(_)
            | TypeKind::DataShape(_)
            | TypeKind::Error(_)
            | TypeKind::Bool
            | TypeKind::I8
            | TypeKind::I16
            | TypeKind::I32
            | TypeKind::I64
            | TypeKind::I128
            | TypeKind::ISize
            | TypeKind::U8
            | TypeKind::U16
            | TypeKind::U32
            | TypeKind::U64
            | TypeKind::U128
            | TypeKind::USize
            | TypeKind::F32
            | TypeKind::F64
            | TypeKind::String
            | TypeKind::Char
            | TypeKind::Bytes
            | TypeKind::TextCluster
            | TypeKind::Duration
            | TypeKind::Progress
            | TypeKind::DenseNumeric(_)
            | TypeKind::StageApi(_)
            | TypeKind::LineContext
            | TypeKind::StageActorHandle(_)
            | TypeKind::CueHandle
            | TypeKind::VoiceHandle
            | TypeKind::StatementIngress(_)
            | TypeKind::DisplayContext
            | TypeKind::DisplayError
            | TypeKind::DebugStatePath
            | TypeKind::ObservationFieldPath
            | TypeKind::Predicate
            | TypeKind::Observation
            | TypeKind::ObservedObject
            | TypeKind::AgentBBox
            | TypeKind::ActionName
            | TypeKind::ActionTarget
            | TypeKind::ActionResult
            | TypeKind::AgentValue
            | TypeKind::DataFormat
            | TypeKind::DataValue
            | TypeKind::DataError
            | TypeKind::DataErrorKind
            | TypeKind::DataPath
            | TypeKind::DataPathSegment
            | TypeKind::DataMapKind
            | TypeKind::AgentEntityMetadata
            | TypeKind::AgentSourceAnchor
            | TypeKind::AgentProjectGraphNeighborhood
            | TypeKind::AgentProjectGraphSymbol
            | TypeKind::AgentProjectGraphEdge
            | TypeKind::CaptureTarget
            | TypeKind::CaptureRef
            | TypeKind::AgentResource
            | TypeKind::AgentResourceBody
            | TypeKind::RagContextPack
            | TypeKind::AgentBuiltin(_)
            | TypeKind::PresentationResource(_)
            | TypeKind::CompileTimeCallable(_)
            | TypeKind::CompileTimeScalar(_)
            | TypeKind::CompileTimeStyle(_)
            | TypeKind::CompileTimeEnum(_)
            | TypeKind::CompileTimeFx(_)
            | TypeKind::CharacterPatch(_)
            | TypeKind::FocusPatch
            | TypeKind::CharacterDialogue(_)
            | TypeKind::ViewValue
            | TypeKind::CharacterNominal(_)
            | TypeKind::Unit
            | TypeKind::Never => {}
        }
        Ok(stored)
    }
}
