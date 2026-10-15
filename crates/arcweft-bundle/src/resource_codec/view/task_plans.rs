//! Exact original-coordinate bindings for the sole Core View sealing protocol.

use std::collections::{BTreeMap, BTreeSet};

use arcweft_core::{
    pattern::{
        CanonicalMatchSource, MatchSourceEncodingError, MatchSourceMaterializationBudget,
        MatchSourceSink, MatchSourceWriter,
    },
    plan::{RuntimeTaskPlanBuildCoordinate, RuntimeTaskSemanticBindingKind},
    task::{
        NeedProducerFamily, TaskPlanSemanticDigest, ViewTaskPlanAuthority,
        ViewTaskPlanDigestRequest, ViewTaskPlanValidationError as Error,
    },
};
use arcweft_source::SourceSetRevision;
use arcweft_view::{
    AcceptedViewProgramRevision, CheckedViewMatchAdmissionDigest, ViewMatchSiteId, ViewProgramId,
};

use super::{
    ValidatedViewProduct, ValidatedViewProgramResource, ViewProductValidationLimits,
    ViewProgramInstruction,
};

/// Original lowering data. This record does not grant sealing authority.
#[derive(Clone, Debug)]
pub struct ViewTaskPlanBindingSource {
    program: ViewProgramId,
    revision: AcceptedViewProgramRevision,
    site: ViewMatchSiteId,
    admission: CheckedViewMatchAdmissionDigest,
    coordinate: RuntimeTaskPlanBuildCoordinate,
}

impl ViewTaskPlanBindingSource {
    #[must_use]
    pub fn new(
        program: ViewProgramId,
        revision: AcceptedViewProgramRevision,
        site: ViewMatchSiteId,
        admission: CheckedViewMatchAdmissionDigest,
        coordinate: RuntimeTaskPlanBuildCoordinate,
    ) -> Self {
        Self {
            program,
            revision,
            site,
            admission,
            coordinate,
        }
    }
}

/// The same source record after the complete original/current-product join.
#[derive(Clone, Debug)]
pub struct ValidatedViewTaskPlanBinding(ViewTaskPlanBindingSource);

impl ValidatedViewTaskPlanBinding {
    pub const fn program_id(&self) -> &ViewProgramId {
        &self.0.program
    }
    pub const fn accepted_revision(&self) -> AcceptedViewProgramRevision {
        self.0.revision
    }
    pub const fn site(&self) -> ViewMatchSiteId {
        self.0.site
    }
    pub const fn admission(&self) -> CheckedViewMatchAdmissionDigest {
        self.0.admission
    }
    pub const fn coordinate(&self) -> &RuntimeTaskPlanBuildCoordinate {
        &self.0.coordinate
    }
}

#[derive(Clone, Debug)]
pub(super) struct ValidatedViewTaskPlanBindings {
    program: ViewProgramId,
    revision: AcceptedViewProgramRevision,
    source_set: SourceSetRevision,
    rows: BTreeMap<RuntimeTaskPlanBuildCoordinate, ValidatedViewTaskPlanBinding>,
}

impl ValidatedViewProduct {
    /// Joins the exact original coordinates and source rows before exposing an authority.
    pub fn bind_task_plans(
        &self,
        expected: &[RuntimeTaskPlanBuildCoordinate],
        sources: Vec<ViewTaskPlanBindingSource>,
        limits: ViewProductValidationLimits,
    ) -> Result<(), Error> {
        let current = self.program().ok_or(Error::ProgramMismatch)?;
        let bindings = ValidatedViewTaskPlanBindings::new(
            current,
            self.source_map().source_set_revision(),
            expected,
            sources,
            limits,
        )?;
        self.install_task_bindings(bindings)
    }
}

impl ValidatedViewProgramResource {
    pub fn task_plan_binding(
        &self,
        coordinate: &RuntimeTaskPlanBuildCoordinate,
    ) -> Option<&ValidatedViewTaskPlanBinding> {
        self.task_bindings.get()?.rows.get(coordinate)
    }

    pub fn task_plan_bindings(&self) -> impl Iterator<Item = &ValidatedViewTaskPlanBinding> {
        self.task_bindings
            .get()
            .into_iter()
            .flat_map(|bindings| bindings.rows.values())
    }
}

impl ValidatedViewTaskPlanBindings {
    fn new(
        current: &ValidatedViewProgramResource,
        source_set: SourceSetRevision,
        expected: &[RuntimeTaskPlanBuildCoordinate],
        sources: Vec<ViewTaskPlanBindingSource>,
        limits: ViewProductValidationLimits,
    ) -> Result<Self, Error> {
        if current.source_set_revision() != source_set {
            return Err(Error::StaleAuthority);
        }
        let work = expected
            .len()
            .checked_add(sources.len())
            .and_then(|count| count.checked_add(current.resource().instructions.len()))
            .and_then(|count| u64::try_from(count).ok())
            .ok_or(Error::ArithmeticOverflow)?;
        if work > limits.validation_work {
            return Err(Error::WorkLimitExceeded);
        }
        #[expect(
            clippy::mutable_key_type,
            reason = "Coordinate equality/order use only immutable original issuer identity and ordinal; sealed publication never changes either."
        )]
        let expected_set: BTreeSet<_> = expected.iter().collect();
        if expected_set.len() != expected.len() || sources.len() != expected.len() {
            return Err(Error::BindingSetMismatch);
        }
        if let Some(first) = expected.first()
            && expected
                .iter()
                .any(|coordinate| !coordinate.same_candidate(first))
        {
            return Err(Error::CoordinateOwnerMismatch);
        }
        let mut budget = MatchSourceMaterializationBudget::new(limits.match_source_bytes);
        for instruction in &current.resource().instructions {
            if let ViewProgramInstruction::Match { program, .. } = instruction {
                program
                    .source
                    .preflight_source(&mut budget)
                    .map_err(source_storage_error)?;
            }
        }
        preflight_hash_work(current, work, limits.validation_work)?;
        let mut sites = BTreeMap::new();
        for instruction in &current.resource().instructions {
            if let ViewProgramInstruction::Match { program, .. } = instruction {
                let site = ViewMatchSiteId::for_source_coordinate(program.source.coordinate())
                    .map_err(|_| Error::ArithmeticOverflow)?;
                let mut writer = MatchSourceWriter::new(MatchHash(blake3::Hasher::new()));
                let encoded = program.source.write_semantic(&mut writer);
                let hash = writer
                    .finish(encoded)
                    .map_err(|_| Error::ArithmeticOverflow)?
                    .0
                    .finalize();
                let admission =
                    CheckedViewMatchAdmissionDigest::for_match_semantic_transcript(hash.as_bytes());
                if sites.insert(site, admission).is_some() {
                    return Err(Error::DuplicateBinding);
                }
            }
        }
        #[expect(
            clippy::mutable_key_type,
            reason = "Coordinate equality/order use only immutable original issuer identity and ordinal; sealed publication never changes either."
        )]
        let mut rows = BTreeMap::new();
        let mut covered_sites = BTreeSet::new();
        for source in sources {
            if &source.program != current.program_id() {
                return Err(Error::ProgramMismatch);
            }
            if source.revision != current.accepted_revision() {
                return Err(Error::StaleAuthority);
            }
            if !expected_set.contains(&source.coordinate) {
                return Err(Error::BindingSetMismatch);
            }
            let admission = sites.get(&source.site).ok_or(Error::SiteMismatch)?;
            if *admission != source.admission {
                return Err(Error::AdmissionMismatch);
            }
            covered_sites.insert(source.site);
            let coordinate = source.coordinate.clone();
            if rows
                .insert(coordinate, ValidatedViewTaskPlanBinding(source))
                .is_some()
            {
                return Err(Error::DuplicateBinding);
            }
        }
        if rows.len() != expected.len() || covered_sites.len() != sites.len() {
            return Err(Error::BindingSetMismatch);
        }
        Ok(Self {
            program: current.program_id().clone(),
            revision: current.accepted_revision(),
            source_set,
            rows,
        })
    }
}

impl ViewTaskPlanAuthority for ValidatedViewProgramResource {
    fn task_plan_semantic_digest(
        &self,
        request: ViewTaskPlanDigestRequest<'_>,
    ) -> Result<TaskPlanSemanticDigest, Error> {
        let bindings = self.task_bindings.get().ok_or(Error::StaleAuthority)?;
        if &bindings.program != self.program_id()
            || bindings.revision != self.accepted_revision()
            || bindings.source_set != self.source_set_revision()
        {
            return Err(Error::StaleAuthority);
        }
        let base = request.base();
        let binding =
            bindings
                .rows
                .get(base.coordinate())
                .ok_or_else(|| Error::MissingBinding {
                    coordinate: base.coordinate().clone(),
                })?;
        if base.binding_marker() != RuntimeTaskSemanticBindingKind::View {
            return Err(Error::ExpectedViewMarker);
        }
        if base.family() != NeedProducerFamily::ViewMatchSubscription {
            return Err(Error::ExpectedViewFamily);
        }
        let mut hash = blake3::Hasher::new();
        hash.update(TaskPlanSemanticDigest::DOMAIN);
        hash.update(&[base.plan_owner_tag()]);
        hash.update(base.executable_semantic_digest().as_bytes());
        hash.update(base.producer_function_semantic_digest().as_bytes());
        hash.update(&[
            base.family().semantic_tag(),
            base.task_class().semantic_tag(),
        ]);
        hash.update(base.request_template_digest().as_bytes());
        hash.update(base.control_effect_contract_digest().as_bytes());
        hash.update(&[1]);
        let program = binding.program_id().as_str().as_bytes();
        let length = u32::try_from(program.len()).map_err(|_| Error::ArithmeticOverflow)?;
        hash.update(&length.to_le_bytes());
        hash.update(program);
        hash.update(binding.site().as_bytes());
        hash.update(binding.admission().as_bytes());
        Ok(request.finish_authority_transcript(hash))
    }
}

struct MatchHash(blake3::Hasher);

fn source_storage_error(error: arcweft_core::pattern::MatchSourceMaterializationError) -> Error {
    use arcweft_core::pattern::MatchSourceMaterializationError;
    match error {
        MatchSourceMaterializationError::LimitExceeded { .. } => Error::WorkLimitExceeded,
        MatchSourceMaterializationError::ArithmeticOverflow
        | MatchSourceMaterializationError::Encoding(_) => Error::ArithmeticOverflow,
        MatchSourceMaterializationError::ChangedCoordinateEncoding => Error::AdmissionMismatch,
    }
}

fn preflight_hash_work(
    current: &ValidatedViewProgramResource,
    used: u64,
    limit: u64,
) -> Result<(), Error> {
    let mut writer = MatchSourceWriter::new(BindingWork { used, limit });
    let encoded = (|| {
        for instruction in &current.resource().instructions {
            if !writer.is_active() {
                break;
            }
            if let ViewProgramInstruction::Match { program, .. } = instruction {
                writer.extend_from_slice(ViewMatchSiteId::DOMAIN);
                program.source.coordinate().write_source(&mut writer)?;
                program.source.write_semantic(&mut writer)?;
                writer.extend_from_slice(CheckedViewMatchAdmissionDigest::DOMAIN);
                writer.extend_from_slice(&[0; 32]);
            }
        }
        Ok(())
    })();
    writer.finish(encoded).map(|_| ()).map_err(|error| error.0)
}

struct BindingWork {
    used: u64,
    limit: u64,
}
struct BindingWorkError(Error);
impl From<MatchSourceEncodingError> for BindingWorkError {
    fn from(_: MatchSourceEncodingError) -> Self {
        Self(Error::ArithmeticOverflow)
    }
}
impl MatchSourceSink for BindingWork {
    type Error = BindingWorkError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let count =
            u64::try_from(bytes.len()).map_err(|_| BindingWorkError(Error::ArithmeticOverflow))?;
        let attempted = self
            .used
            .checked_add(count)
            .ok_or(BindingWorkError(Error::ArithmeticOverflow))?;
        if attempted > self.limit {
            return Err(BindingWorkError(Error::WorkLimitExceeded));
        }
        self.used = attempted;
        Ok(())
    }
}
impl MatchSourceSink for MatchHash {
    type Error = MatchSourceEncodingError;
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.update(bytes);
        Ok(())
    }
}
