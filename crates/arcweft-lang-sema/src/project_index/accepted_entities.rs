//! Generation-bound target entity publication. Public index builders are
//! metadata tools; only the exact final project producer can issue this owner.

use super::{EntitySymbol, ProjectEntityId, ProjectSemanticIndex, SemanticHash};
use crate::final_analysis::{FinalSemanticAnalysis, FinalSemanticAnalysisError};
use crate::types::{EntityKind, EntityType, TypeKind};
use arcweft_core::{plan::FlowRuntimeId, value::RuntimeImportedProjectEntityReference};
use arcweft_id::{ProjectEntityReferenceFamily, PublicId};
use arcweft_lang_hir::{
    project::{AcceptedHirProjectGeneration, HirAnalysisProjectView},
    symbol::{ProjectSymbolTable, ProjectSymbolWorldId},
};
use arcweft_source::{SourceAnchor, SourceDocument, SourceDocumentId, SourceSpan};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
enum AcceptedEntityCatalogOwner {
    Source {
        index: Arc<ProjectSemanticIndex>,
        generation: Arc<AcceptedHirProjectGeneration>,
    },
    HostSignals {
        world: ProjectSymbolWorldId,
        entities: BTreeMap<ProjectEntityId, EntitySymbol>,
    },
}

/// Immutable source-index lease or explicit validated host-signal input.
/// Source rows are borrowed from their original index, never copied into an
/// independently editable target registry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedProjectEntityCatalog {
    owner: AcceptedEntityCatalogOwner,
    documents: BTreeMap<SourceDocumentId, Arc<SourceDocument>>,
    generation_digest: [u8; 32],
}

/// One entity from the catalog's exact immutable owner. There is no public
/// constructor from `EntitySymbol`, a public ID, or a semantic hash.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceptedProjectEntity {
    catalog: Arc<AcceptedProjectEntityCatalog>,
    identity: ProjectEntityId,
}

/// A typed explicit host input. This is not source-project proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostSignalPublicationInput {
    public_id: PublicId,
    value_type: TypeKind,
    source: SourceSpan,
    document: Arc<SourceDocument>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectEntityPublicationError {
    #[error("target project semantic generation is not accepted: {0}")]
    Generation(Box<FinalSemanticAnalysisError>),
    #[error("target project index differs from the exact accepted source projection")]
    IndexMismatch,
    #[error("target project entity {identity:?} has an invalid family or owner")]
    EntityOwner { identity: ProjectEntityId },
    #[error("target project entity {identity:?} does not belong to its original source document")]
    Source { identity: ProjectEntityId },
    #[error("target project entity {identity:?} has an invalid closed value type")]
    ValueType { identity: ProjectEntityId },
    #[error("target project entity {public_id} is ambiguous across original structural owners")]
    Ambiguous { public_id: PublicId },
    #[error("explicit host signal {public_id} has duplicate publication inputs")]
    DuplicateSignal { public_id: PublicId },
    #[error("explicit host signal {public_id} has an invalid family, closed type or source")]
    HostSignal { public_id: PublicId },
}

impl AcceptedProjectEntityCatalog {
    /// Issues a source catalog only after rejoining the complete exact final
    /// producer. An arbitrary index or `with_entity` mutation cannot qualify.
    pub fn try_from_final_project(
        index: Arc<ProjectSemanticIndex>,
        project: HirAnalysisProjectView<'_>,
        symbols: &ProjectSymbolTable,
        analysis: &FinalSemanticAnalysis,
    ) -> Result<Self, ProjectEntityPublicationError> {
        analysis
            .validate_generation(project, symbols)
            .map_err(|error| ProjectEntityPublicationError::Generation(Box::new(error)))?;
        let expected = ProjectSemanticIndex::try_from_final_project(
            index.program_hash().clone(),
            project,
            symbols,
            analysis,
        )
        .map_err(|_| ProjectEntityPublicationError::IndexMismatch)?;
        if index.as_ref() != &expected {
            return Err(ProjectEntityPublicationError::IndexMismatch);
        }
        let generation = Arc::clone(analysis.hir_generation());
        let documents = project
            .modules()
            .map(|(_, module)| {
                let document = Arc::clone(module.provenance().document());
                (document.identity().id().clone(), document)
            })
            .collect::<BTreeMap<_, _>>();
        for entity in index.entities().values() {
            validate_entity(entity, &generation, &documents)?;
        }
        let mut identities_by_public = BTreeMap::new();
        for entity in index.entities().values() {
            if identities_by_public
                .insert(entity.public_id(), entity.identity())
                .is_some()
            {
                return Err(ProjectEntityPublicationError::Ambiguous {
                    public_id: entity.public_id().clone(),
                });
            }
        }
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.target-project-entities.v1\0");
        hash_string(&mut hash, generation.symbol_world().package().as_str());
        hash_string(
            &mut hash,
            generation.symbol_world().root_document().as_str(),
        );
        hash_string(&mut hash, generation.symbol_world().profile());
        hash.update(generation.symbol_revision().as_source_set().as_bytes());
        hash_string(&mut hash, index.program_hash().as_str());
        Ok(Self {
            owner: AcceptedEntityCatalogOwner::Source { index, generation },
            documents,
            generation_digest: *hash.finalize().as_bytes(),
        })
    }

    /// Validates explicitly supplied host signals in their caller-selected
    /// world. This path cannot publish Flow, Character or other source owners.
    pub fn try_from_host_signals(
        world: ProjectSymbolWorldId,
        inputs: &[HostSignalPublicationInput],
    ) -> Result<Self, ProjectEntityPublicationError> {
        let mut entities = BTreeMap::new();
        let mut documents = BTreeMap::new();
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.target-host-signals.v1\0");
        hash_string(&mut hash, world.package().as_str());
        hash_string(&mut hash, world.root_document().as_str());
        hash_string(&mut hash, world.profile());
        for input in inputs {
            let mut entity_hash = blake3::Hasher::new();
            entity_hash.update(b"arcweft.host-signal-publication.v1\0");
            hash_string(&mut entity_hash, input.public_id.as_str());
            entity_hash.update(
                input
                    .value_type
                    .semantic_identity_digest()
                    .map_err(|_| ProjectEntityPublicationError::HostSignal {
                        public_id: input.public_id.clone(),
                    })?
                    .as_bytes(),
            );
            let symbol = EntitySymbol::new(
                ProjectEntityId::public(input.public_id.clone()),
                EntityType::new(EntityKind::Signal, Some(input.value_type.clone())),
                SourceAnchor::from_span(input.source.clone()),
                SemanticHash::new(entity_hash.finalize().to_hex().to_string()),
            );
            if entities.insert(symbol.identity().clone(), symbol).is_some() {
                return Err(ProjectEntityPublicationError::DuplicateSignal {
                    public_id: input.public_id.clone(),
                });
            }
            match documents.insert(
                input.document.identity().id().clone(),
                Arc::clone(&input.document),
            ) {
                Some(previous) if previous.identity() != input.document.identity() => {
                    return Err(ProjectEntityPublicationError::HostSignal {
                        public_id: input.public_id.clone(),
                    });
                }
                _ => {}
            }
        }
        for entity in entities.values() {
            hash_string(&mut hash, entity.public_id().as_str());
            hash_string(&mut hash, entity.semantic_hash().as_str());
        }
        for document in documents.values() {
            hash_string(&mut hash, document.identity().id().as_str());
            hash.update(document.identity().revision().as_bytes());
        }
        Ok(Self {
            owner: AcceptedEntityCatalogOwner::HostSignals { world, entities },
            documents,
            generation_digest: *hash.finalize().as_bytes(),
        })
    }

    pub fn entities(&self) -> &BTreeMap<ProjectEntityId, EntitySymbol> {
        match &self.owner {
            AcceptedEntityCatalogOwner::Source { index, .. } => index.entities(),
            AcceptedEntityCatalogOwner::HostSignals { entities, .. } => entities,
        }
    }

    pub fn entity(self: &Arc<Self>, identity: &ProjectEntityId) -> Option<AcceptedProjectEntity> {
        self.entities()
            .contains_key(identity)
            .then(|| AcceptedProjectEntity {
                catalog: Arc::clone(self),
                identity: identity.clone(),
            })
    }

    pub fn documents(&self) -> impl ExactSizeIterator<Item = &Arc<SourceDocument>> {
        self.documents.values()
    }

    pub const fn generation_digest(&self) -> &[u8; 32] {
        &self.generation_digest
    }

    pub fn world(&self) -> &ProjectSymbolWorldId {
        match &self.owner {
            AcceptedEntityCatalogOwner::Source { generation, .. } => generation.symbol_world(),
            AcceptedEntityCatalogOwner::HostSignals { world, .. } => world,
        }
    }

    pub fn source_index(&self) -> Option<&Arc<ProjectSemanticIndex>> {
        match &self.owner {
            AcceptedEntityCatalogOwner::Source { index, .. } => Some(index),
            AcceptedEntityCatalogOwner::HostSignals { .. } => None,
        }
    }

    pub fn validate_generation(
        &self,
        generation: &AcceptedHirProjectGeneration,
    ) -> Result<(), ProjectEntityPublicationError> {
        match &self.owner {
            AcceptedEntityCatalogOwner::Source {
                generation: expected,
                ..
            } if expected.same_generation(generation) => Ok(()),
            AcceptedEntityCatalogOwner::Source { .. }
            | AcceptedEntityCatalogOwner::HostSignals { .. } => {
                Err(ProjectEntityPublicationError::Generation(Box::new(
                    FinalSemanticAnalysisError::GenerationMismatch,
                )))
            }
        }
    }
}

impl HostSignalPublicationInput {
    pub fn try_new(
        nominal_catalog: &crate::env::nominal::AcceptedNominalCatalog,
        public_id: PublicId,
        value_type: TypeKind,
        document: Arc<SourceDocument>,
        source: SourceSpan,
    ) -> Result<Self, ProjectEntityPublicationError> {
        if ProjectEntityReferenceFamily::Signal
            .validate_public_id(&public_id)
            .is_err()
            || source.validate_for(&document).is_err()
            || value_type.contains_nominal_poison()
            || crate::types::contains_generic_parameter(&value_type)
            || value_type.semantic_identity_digest().is_err()
        {
            return Err(ProjectEntityPublicationError::HostSignal { public_id });
        }
        let value_type = crate::env::nominal::standard_watch_record(nominal_catalog)
            .ok_or_else(|| ProjectEntityPublicationError::HostSignal {
                public_id: public_id.clone(),
            })?
            .try_instantiate([value_type])
            .map_err(|_| ProjectEntityPublicationError::HostSignal {
                public_id: public_id.clone(),
            })?;
        Ok(Self {
            public_id,
            value_type,
            source,
            document,
        })
    }
    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }
    pub const fn value_type(&self) -> &TypeKind {
        &self.value_type
    }
}

impl AcceptedProjectEntity {
    /// The catalog constructor guarantees this immutable identity exists.
    ///
    /// # Panics
    /// Panics only if the private issuance invariant is violated: the entity
    /// identity must remain in the immutable catalog that issued it.
    pub fn symbol(&self) -> &EntitySymbol {
        self.catalog
            .entities()
            .get(&self.identity)
            .expect("issued target entity remains in its immutable original catalog")
    }
    pub const fn identity(&self) -> &ProjectEntityId {
        &self.identity
    }
    pub fn catalog(&self) -> &Arc<AcceptedProjectEntityCatalog> {
        &self.catalog
    }
    pub fn ty(&self) -> TypeKind {
        TypeKind::Ref(self.symbol().ty().clone())
    }
    /// # Panics
    /// Panics only if a catalog issued an entity outside its closed family domain.
    pub fn family(&self) -> ProjectEntityReferenceFamily {
        entity_family(self.symbol().ty().kind())
            .expect("target catalog admits only closed entity families")
    }
    /// # Panics
    /// Panics only if an issued entity's closed type loses its semantic identity.
    pub fn semantic_identity(&self) -> [u8; 32] {
        let mut hash = blake3::Hasher::new();
        hash.update(b"arcweft.accepted-target-project-entity.v1\0");
        hash.update(self.catalog.generation_digest());
        hash.update(&[self.family().semantic_tag()]);
        match self.identity() {
            ProjectEntityId::Public(public_id) => {
                hash.update(&[0]);
                hash_string(&mut hash, public_id.as_str());
            }
            ProjectEntityId::StructuralFlow(flow) => {
                hash.update(&[1]);
                hash.update(flow.semantic_digest().as_bytes());
            }
        }
        hash_string(&mut hash, self.symbol().semantic_hash().as_str());
        hash.update(
            self.ty()
                .semantic_identity_digest()
                .expect("admitted target type is closed")
                .as_bytes(),
        );
        let span = self.symbol().source().span();
        hash_string(&mut hash, span.source().id().as_str());
        hash.update(span.source().revision().as_bytes());
        hash.update(&(span.range().start() as u64).to_le_bytes());
        hash.update(&(span.range().end() as u64).to_le_bytes());
        *hash.finalize().as_bytes()
    }
    /// # Panics
    /// Panics only if private catalog invariants no longer project the admitted
    /// family, type and exact structural Flow identity.
    pub fn runtime_reference(&self) -> RuntimeImportedProjectEntityReference {
        let flow = match self.identity() {
            ProjectEntityId::StructuralFlow(flow) => Some(
                FlowRuntimeId::from_checked_declaration_digest(
                    flow.semantic_digest().into_bytes(),
                    flow.public_id().as_str(),
                )
                .expect("admitted Flow declaration has its exact checked runtime identity"),
            ),
            ProjectEntityId::Public(_) => None,
        };
        RuntimeImportedProjectEntityReference::try_new(
            self.family(),
            self.symbol().public_id().clone(),
            *self.catalog.generation_digest(),
            self.semantic_identity(),
            *self
                .ty()
                .semantic_identity_digest()
                .expect("admitted target type is closed")
                .as_bytes(),
            flow,
        )
        .expect("accepted target entity projects the same closed family, identity and type")
    }
}

fn validate_entity(
    entity: &EntitySymbol,
    generation: &AcceptedHirProjectGeneration,
    documents: &BTreeMap<SourceDocumentId, Arc<SourceDocument>>,
) -> Result<(), ProjectEntityPublicationError> {
    let identity = entity.identity().clone();
    let family = entity_family(entity.ty().kind()).ok_or_else(|| {
        ProjectEntityPublicationError::EntityOwner {
            identity: identity.clone(),
        }
    })?;
    if family.validate_public_id(entity.public_id()).is_err()
        || matches!(entity.identity(), ProjectEntityId::Public(_))
            && family == ProjectEntityReferenceFamily::Flow
        || matches!(entity.identity(), ProjectEntityId::StructuralFlow(_))
            && family != ProjectEntityReferenceFamily::Flow
    {
        return Err(ProjectEntityPublicationError::EntityOwner { identity });
    }
    if let ProjectEntityId::StructuralFlow(flow) = entity.identity() {
        if flow.package() != generation.package() || generation.module(flow.module()).is_none() {
            return Err(ProjectEntityPublicationError::EntityOwner { identity });
        }
    }
    let span = entity.source().span();
    if !documents
        .get(span.source().id())
        .is_some_and(|document| span.validate_for(document).is_ok())
    {
        return Err(ProjectEntityPublicationError::Source { identity });
    }
    if TypeKind::Ref(entity.ty().clone())
        .semantic_identity_digest()
        .is_err()
        || entity
            .ty()
            .value()
            .is_some_and(TypeKind::contains_nominal_poison)
        || entity
            .ty()
            .value()
            .is_some_and(crate::types::contains_generic_parameter)
        || matches!(entity.ty().kind(), EntityKind::Signal | EntityKind::Metric)
            != entity.ty().value().is_some()
        || matches!(entity.ty().kind(), EntityKind::Signal | EntityKind::Metric)
            && entity.ty().observable_payload().is_none()
    {
        return Err(ProjectEntityPublicationError::ValueType { identity });
    }
    Ok(())
}

pub(crate) fn entity_family(kind: &EntityKind) -> Option<ProjectEntityReferenceFamily> {
    use ProjectEntityReferenceFamily as Family;
    Some(match kind {
        EntityKind::Agent => Family::Agent,
        EntityKind::Entry => Family::Entry,
        EntityKind::Flow => Family::Flow,
        EntityKind::Choice => Family::Choice,
        EntityKind::ChoiceOption => Family::ChoiceOption,
        EntityKind::Character => Family::Character,
        EntityKind::View => Family::View,
        EntityKind::Action => Family::Action,
        EntityKind::Activity => Family::Activity,
        EntityKind::DialogueLine => Family::DialogueLine,
        EntityKind::Text => Family::Text,
        EntityKind::Content => Family::Content,
        EntityKind::Input => Family::Input,
        EntityKind::Button => Family::Button,
        EntityKind::Style => Family::Style,
        EntityKind::Asset => Family::Asset,
        EntityKind::Image => Family::Image,
        EntityKind::Animation => Family::Animation,
        EntityKind::Capture => Family::Capture,
        EntityKind::Hook => Family::Hook,
        EntityKind::Signal => Family::Signal,
        EntityKind::Metric => Family::Metric,
        EntityKind::Scene => Family::Scene,
        EntityKind::Test => Family::Test,
        EntityKind::Bench => Family::Bench,
        EntityKind::Layer => Family::Layer,
        EntityKind::Voice => Family::Voice,
        EntityKind::Se => Family::Se,
        EntityKind::Bgm => Family::Bgm,
        EntityKind::AudioBus => Family::AudioBus,
        EntityKind::MixerSnapshot => Family::MixerSnapshot,
        EntityKind::Ducking => Family::Ducking,
        EntityKind::Motion => Family::Motion,
        EntityKind::Rig => Family::Rig,
        EntityKind::Slot => Family::Slot,
        EntityKind::Target => Family::Target,
        EntityKind::Other(_) => return None,
    })
}

fn hash_string(hash: &mut blake3::Hasher, value: &str) {
    hash.update(&(value.len() as u64).to_le_bytes());
    hash.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::final_analysis::tests::{analyze, fixture};
    use crate::project_index::ProgramHash;
    use arcweft_lang_hir::symbol::CallablePackageId;
    use arcweft_source::{SourceName, SourceRange};

    const SOURCE: &str = "pub signal @signal.level Level: Watch<i64>\nflow @flow.opening opening() -> String { return \"opening\" }\n";

    #[test]
    fn native_entity_catalog_rejects_public_index_forgery_and_foreign_generation() {
        let native = fixture(SOURCE, None);
        let analysis = analyze(&native).expect("native source accepted");
        let index = Arc::new(
            ProjectSemanticIndex::try_from_final_project(
                ProgramHash::new("native-catalog"),
                native.project.analysis_view().expect("HIR"),
                &native.symbols,
                &analysis,
            )
            .expect("source index"),
        );
        let catalog = AcceptedProjectEntityCatalog::try_from_final_project(
            Arc::clone(&index),
            native.project.analysis_view().expect("HIR"),
            &native.symbols,
            &analysis,
        )
        .expect("issued source owner");
        assert!(Arc::ptr_eq(
            catalog.source_index().expect("original index"),
            &index
        ));
        assert_eq!(catalog.entities(), index.entities());
        assert!(
            catalog
                .validate_generation(analysis.hir_generation())
                .is_ok()
        );
        let original = index
            .entities()
            .values()
            .find(|row| row.ty().kind() == &EntityKind::Signal)
            .expect("signal");
        let forged = EntitySymbol::new(
            original.identity().clone(),
            EntityType::new(EntityKind::Signal, Some(TypeKind::String)),
            original.source().clone(),
            original.semantic_hash().clone(),
        );
        let forged_index = Arc::new(index.as_ref().clone().with_entity(forged));
        assert!(matches!(
            AcceptedProjectEntityCatalog::try_from_final_project(
                forged_index,
                native.project.analysis_view().expect("HIR"),
                &native.symbols,
                &analysis
            ),
            Err(ProjectEntityPublicationError::IndexMismatch)
        ));
        let hash_forgery = EntitySymbol::new(
            original.identity().clone(),
            original.ty().clone(),
            original.source().clone(),
            SemanticHash::new("caller-chosen"),
        );
        assert!(matches!(
            AcceptedProjectEntityCatalog::try_from_final_project(
                Arc::new(index.as_ref().clone().with_entity(hash_forgery)),
                native.project.analysis_view().expect("HIR"),
                &native.symbols,
                &analysis
            ),
            Err(ProjectEntityPublicationError::IndexMismatch)
        ));
        let foreign = fixture(SOURCE, None);
        let foreign_analysis = analyze(&foreign).expect("identical source in foreign generation");
        assert!(
            catalog
                .validate_generation(foreign_analysis.hir_generation())
                .is_err()
        );
        assert!(matches!(
            AcceptedProjectEntityCatalog::try_from_final_project(
                index,
                foreign.project.analysis_view().expect("foreign HIR"),
                &foreign.symbols,
                &analysis
            ),
            Err(ProjectEntityPublicationError::Generation(_))
        ));
    }

    #[test]
    fn host_signal_publication_rejects_wrong_family_duplicate_and_foreign_source_revision() {
        let environment = crate::env::TypeCheckEnv::standard();
        let id = PublicId::try_new("signal.level").expect("signal ID");
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcweft-test://signals").expect("document ID"),
                SourceName::path("signals.json"),
                "{\"level\":1}",
            )
            .expect("document"),
        );
        let span = document
            .span(SourceRange::new(0, document.text().len()))
            .expect("span");
        assert!(
            HostSignalPublicationInput::try_new(
                environment.nominal_catalog(),
                PublicId::try_new("flow.level").expect("wrong family"),
                TypeKind::I64,
                Arc::clone(&document),
                span.clone()
            )
            .is_err()
        );
        let revised = Arc::new(
            SourceDocument::try_new(
                document.identity().id().clone(),
                SourceName::path("signals.json"),
                "{\"level\":2}",
            )
            .expect("revised document"),
        );
        assert!(
            HostSignalPublicationInput::try_new(
                environment.nominal_catalog(),
                id.clone(),
                TypeKind::I64,
                revised,
                span.clone()
            )
            .is_err()
        );
        let input = HostSignalPublicationInput::try_new(
            environment.nominal_catalog(),
            id.clone(),
            TypeKind::I64,
            Arc::clone(&document),
            span,
        )
        .expect("typed host input");
        let crate::types::TypeKind::AcceptedNominal(carrier) = input.value_type() else {
            panic!("explicit host publication must retain the accepted Watch carrier")
        };
        assert_eq!(
            carrier.declaration(),
            &crate::env::nominal::standard_nominal_id("Watch")
        );
        assert_eq!(carrier.arguments(), &[TypeKind::I64]);
        let world = ProjectSymbolWorldId::try_new(
            CallablePackageId::try_new("host-signals").expect("package"),
            document.identity().id().clone(),
            "host",
        )
        .expect("world");
        assert!(
            matches!(AcceptedProjectEntityCatalog::try_from_host_signals(world, &[input.clone(), input]),
            Err(ProjectEntityPublicationError::DuplicateSignal { public_id }) if public_id == id)
        );
    }
}

#[cfg(test)]
mod ambiguous_source_tests {
    use super::*;
    use crate::{
        final_analysis::tests::{analyze, fixture, try_fixture},
        registration::CharacterRegistrationDiagnosticKind,
    };
    use arcweft_lang_hir::symbol::ProjectSymbolLinkError;

    #[test]
    fn source_target_catalog_rejects_two_structural_flow_owners_with_one_public_label() {
        let rejected = try_fixture(
            "mod child\nflow @flow.opening opening() -> String { return \"root\" }\n",
            Some("flow @flow.opening opening() -> String { return \"child\" }\n"),
        )
        .err()
        .expect("project-global Flow identity collision is rejected before catalog issuance");
        let (public_id, first, duplicate) = rejected
            .diagnostics()
            .iter()
            .find_map(|diagnostic| match diagnostic.kind() {
                CharacterRegistrationDiagnosticKind::ProjectSymbol {
                    error:
                        ProjectSymbolLinkError::DuplicatePublicId {
                            public_id,
                            first,
                            duplicate,
                        },
                } => Some((public_id, first, duplicate)),
                _ => None,
            })
            .expect("typed duplicate public Flow identity diagnostic");
        assert_eq!(public_id.as_str(), "flow.opening");
        assert_ne!(first.source().id(), duplicate.source().id());

        let native = fixture(
            "flow @flow.opening opening() -> String { return \"root\" }\n",
            None,
        );
        let foreign = fixture(
            "mod child\n",
            Some("flow @flow.opening opening() -> String { return \"child\" }\n"),
        );
        let analysis = analyze(&native).expect("valid original source Flow");
        let foreign_analysis = analyze(&foreign).expect("valid original Flow in another module");
        let project = native.project.analysis_view().expect("native HIR");
        let index = ProjectSemanticIndex::try_from_final_project(
            super::super::ProgramHash::new("ambiguous-source-target"),
            project,
            &native.symbols,
            &analysis,
        )
        .expect("exact source index");
        AcceptedProjectEntityCatalog::try_from_final_project(
            Arc::new(index.clone()),
            project,
            &native.symbols,
            &analysis,
        )
        .expect("valid exact source catalog");
        let foreign_index = ProjectSemanticIndex::try_from_final_project(
            super::super::ProgramHash::new("ambiguous-source-target"),
            foreign.project.analysis_view().expect("foreign HIR"),
            &foreign.symbols,
            &foreign_analysis,
        )
        .expect("foreign original source index");
        let native_flow = index
            .entities()
            .values()
            .find(|row| row.ty().kind() == &EntityKind::Flow)
            .expect("native Flow row");
        let foreign_flow = foreign_index
            .entities()
            .values()
            .find(|row| row.ty().kind() == &EntityKind::Flow)
            .expect("foreign Flow row");
        assert_eq!(native_flow.ty().kind(), &EntityKind::Flow);
        assert_eq!(foreign_flow.ty().kind(), &EntityKind::Flow);
        assert_ne!(native_flow.identity(), foreign_flow.identity());
        assert_eq!(native_flow.public_id(), foreign_flow.public_id());
        let forged_index = Arc::new(index.with_entity(foreign_flow.clone()));
        assert_eq!(
            forged_index
                .entities()
                .values()
                .filter(|row| row.public_id().as_str() == "flow.opening")
                .count(),
            2,
        );
        assert!(matches!(
            AcceptedProjectEntityCatalog::try_from_final_project(
                forged_index,
                project,
                &native.symbols,
                &analysis,
            ),
            Err(ProjectEntityPublicationError::IndexMismatch)
        ));
    }
}
