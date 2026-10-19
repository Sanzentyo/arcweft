//! Original whole-program Source. Sections share one cumulative allowance and
//! acquire identities only from the real aggregate construction issuer.

mod decoding;
mod encoding;
mod limits;
#[cfg(test)]
mod tests;

use super::{
    RuntimePlanSourceConstruction, callable_specializations, callable_states, content_declarations,
    content_plans, entries, function_bodies, functions, invocations, line_groups, pure_callables,
    streams,
};
use crate::plan::declaration_sources::RuntimePlanDeclarationSourceError;
use crate::plan::{
    RuntimeFunctionSiteSeedId, RuntimeLineTaskGroupSeedId, RuntimeTaskPlanBuildCoordinate,
    RuntimeTaskPlanSealError,
};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use limits::Bounds;

pub(in crate::plan) use decoding::PreparedRuntimeProgramSource;

const DOMAIN: &[u8] = b"arcweft.runtime.program-source.v1\0";
const SECTION_COUNT: usize = 15;

/// Resource bounds for the complete original program Source grammar.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeProgramSourceLimits {
    pub rows: u32,
    pub nodes: u32,
    pub elements: u32,
    pub depth: u32,
    pub policy_depth: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub opaque_producer_bytes: usize,
}

#[derive(Debug, thiserror::Error)]
pub(in crate::plan) enum RuntimeProgramSourceError {
    #[error("program Source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("program Source has an invalid domain or version")]
    Header,
    #[error("program Source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("program Source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("program Source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("program Source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        actual: usize,
        expected: usize,
    },
    #[error("program Source count arithmetic overflow")]
    Arithmetic,
    #[error("program Source declarations failed: {0}")]
    Declarations(#[from] RuntimePlanDeclarationSourceError),
    #[error("program Source functions failed: {0}")]
    Functions(#[from] functions::RuntimeFunctionSourceError),
    #[error("program Source callable states failed: {0}")]
    States(#[from] callable_states::RuntimeCallableSourceError),
    #[error("program Source specializations failed: {0}")]
    Specializations(#[from] callable_specializations::RuntimeSpecializationSourceError),
    #[error("program Source pure declarations failed: {0}")]
    Pure(#[from] pure_callables::RuntimePureCallableSourceError),
    #[error("program Source content declarations failed: {0}")]
    ContentDeclarations(#[from] content_declarations::RuntimeContentDeclarationSourceError),
    #[error("program Source invocations failed: {0}")]
    Invocations(#[from] invocations::RuntimeInvocationSourceError),
    #[error("program Source content plans failed: {0}")]
    Contents(#[from] content_plans::RuntimeContentPlanSourceError),
    #[error("program Source streams failed: {0}")]
    Streams(#[from] streams::RuntimeStreamSourceError),
    #[error("program Source bodies failed: {0}")]
    Bodies(#[from] function_bodies::RuntimeFunctionBodySourceError),
    #[error("program Source Line groups failed: {0}")]
    Lines(#[from] line_groups::RuntimeLineGroupSourceError),
    #[error("program Source entries failed: {0}")]
    Entries(#[from] entries::RuntimeEntrySourceError),
    #[error("program Source task reservation failed: {0}")]
    Tasks(#[from] RuntimeTaskPlanSealError),
}
type Error = RuntimeProgramSourceError;
impl From<TaskSemanticEncodingError> for Error {
    fn from(error: TaskSemanticEncodingError) -> Self {
        Self::Resource(error)
    }
}
impl Error {
    fn reject(self, meter: &mut TaskSemanticMeter) -> Self {
        let error = match meter.status() {
            Err(cause) if cause != TaskSemanticEncodingError::OwnerRejected => {
                Self::Resource(cause)
            }
            _ => self,
        };
        meter.reject_owner();
        error
    }
}

/// Reference namespaces needed for complete borrowed child admission. They are
/// assertions against actual declarations, never issued handles or proofs.
#[derive(Clone, Copy)]
pub(in crate::plan) struct Namespaces {
    pub types: usize,
    pub locals: usize,
    pub controls: usize,
    pub functions: usize,
    pub states: usize,
    pub specializations: usize,
    pub helpers: usize,
    pub methods: usize,
    pub templates: usize,
    pub attempts: usize,
    pub contents: usize,
    pub defers: usize,
    pub tasks: usize,
    pub groups: usize,
    pub projects: usize,
}
impl Namespaces {
    fn fields(self) -> [usize; 15] {
        [
            self.types,
            self.locals,
            self.controls,
            self.functions,
            self.states,
            self.specializations,
            self.helpers,
            self.methods,
            self.templates,
            self.attempts,
            self.contents,
            self.defers,
            self.tasks,
            self.groups,
            self.projects,
        ]
    }
    fn validate(self, limits: RuntimeProgramSourceLimits) -> Result<usize, Error> {
        self.fields().into_iter().try_fold(0_usize, |total, count| {
            limit("namespace rows", count, limits.rows as usize)?;
            total.checked_add(count).ok_or(Error::Arithmetic)
        })
    }
}

#[derive(Clone, Copy)]
enum Section {
    Types,
    Nominals,
    Locals,
    Controls,
    Functions,
    States,
    Specializations,
    Pure,
    ContentDeclarations,
    Invocations,
    Contents,
    Streams,
    Bodies,
    Lines,
    Entries,
}
impl Section {
    const ALL: [Self; SECTION_COUNT] = [
        Self::Types,
        Self::Nominals,
        Self::Locals,
        Self::Controls,
        Self::Functions,
        Self::States,
        Self::Specializations,
        Self::Pure,
        Self::ContentDeclarations,
        Self::Invocations,
        Self::Contents,
        Self::Streams,
        Self::Bodies,
        Self::Lines,
        Self::Entries,
    ];
    const fn index(self) -> usize {
        self as usize
    }
}
struct Frames<'a> {
    namespace: Namespaces,
    sections: [&'a [u8]; SECTION_COUNT],
}
impl<'a> Frames<'a> {
    fn read(bytes: &'a [u8], limits: RuntimeProgramSourceLimits) -> Result<Self, Error> {
        limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
        let mut cursor = Cursor { bytes, offset: 0 };
        if cursor.raw(DOMAIN.len())? != DOMAIN || cursor.raw(1)? != [1] {
            return Err(Error::Header);
        }
        let namespace = Namespaces {
            types: cursor.count()?,
            locals: cursor.count()?,
            controls: cursor.count()?,
            functions: cursor.count()?,
            states: cursor.count()?,
            specializations: cursor.count()?,
            helpers: cursor.count()?,
            methods: cursor.count()?,
            templates: cursor.count()?,
            attempts: cursor.count()?,
            contents: cursor.count()?,
            defers: cursor.count()?,
            tasks: cursor.count()?,
            groups: cursor.count()?,
            projects: cursor.count()?,
        };
        namespace.validate(limits)?;
        let mut sections = [&[][..]; SECTION_COUNT];
        for section in Section::ALL {
            let length = cursor.count()?;
            sections[section.index()] = cursor.raw(length)?;
        }
        if cursor.offset != bytes.len() {
            return Err(Error::Trailing {
                offset: cursor.offset,
            });
        }
        Ok(Self {
            namespace,
            sections,
        })
    }
    fn get(&self, section: Section) -> &'a [u8] {
        self.sections[section.index()]
    }
}
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn raw(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn count(&mut self) -> Result<usize, Error> {
        Ok(u32::from_le_bytes(self.raw(4)?.try_into().expect("Source u32 width")) as usize)
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::plan) struct Footprint {
    pub nodes: usize,
    pub elements: usize,
    pub storage: usize,
}
pub(in crate::plan) struct OriginalRuntimeProgramSource {
    pub source: RuntimePlanSourceConstruction,
    pub functions: Box<[RuntimeFunctionSiteSeedId]>,
    pub groups: Box<[RuntimeLineTaskGroupSeedId]>,
    pub tasks: Box<[RuntimeTaskPlanBuildCoordinate]>,
    pub footprint: Footprint,
}
fn namespace(kind: &'static str, actual: usize, expected: usize) -> Result<(), Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::Namespace {
            kind,
            actual,
            expected,
        })
    }
}
fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), Error> {
    if actual <= maximum {
        Ok(())
    } else {
        Err(Error::Limit {
            kind,
            actual,
            maximum,
        })
    }
}
