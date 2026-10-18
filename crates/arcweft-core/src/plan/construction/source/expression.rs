//! Original expression source, decoded only into the owning construction API.
//! Prefix framing is iterative. Referenced declarations stay in their original
//! namespaces; source bytes never deserialize an admitted expression or proof.

mod cursor;
mod decoding;
mod encoding;

#[cfg(test)]
pub(in crate::plan::construction::source) mod tests;

pub(in crate::plan) use decoding::RuntimeExpressionSourceHandles;

use super::pattern::{RuntimePatternSourceError, RuntimePatternSourceLimits};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{RuntimeExprSeed, RuntimePlanBuildError};
use crate::task::semantic::{TaskSemanticEncodingError, TaskSemanticMeter};
use crate::value::{RuntimeExpr, RuntimeLiteralSourceError, RuntimeLiteralSourceLimits};

const DOMAIN: &[u8] = b"arcweft.runtime.expression-source.v1\0";

#[derive(Clone, Copy)]
pub(in crate::plan) struct RuntimeExpressionSourceLimits {
    pub nodes: u32,
    pub depth: u32,
    pub elements: u32,
    pub encoded_bytes: usize,
    pub decoded_bytes: usize,
    pub type_count: usize,
    pub local_count: usize,
    pub state_count: usize,
    pub specialization_count: usize,
    pub helper_count: usize,
    pub method_count: usize,
    pub template_count: usize,
    pub attempt_count: usize,
}
impl RuntimeExpressionSourceLimits {
    fn literal(self) -> RuntimeLiteralSourceLimits {
        RuntimeLiteralSourceLimits {
            encoded_bytes: self.encoded_bytes,
            decoded_bytes: self.decoded_bytes,
            elements: self.elements,
        }
    }
    fn pattern(self) -> RuntimePatternSourceLimits {
        RuntimePatternSourceLimits {
            nodes: self.nodes,
            elements: self.elements,
            encoded_bytes: self.encoded_bytes,
            decoded_bytes: self.decoded_bytes,
            type_count: self.type_count,
            local_count: self.local_count,
        }
    }
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub(in crate::plan) enum RuntimeExpressionSourceError {
    #[error("expression source resource admission failed: {0:?}")]
    Resource(TaskSemanticEncodingError),
    #[error("invalid expression source domain")]
    Domain,
    #[error("unsupported expression source version {actual}")]
    Version { actual: u8 },
    #[error("expression source {kind} {actual} exceeds {maximum}")]
    Limit {
        kind: &'static str,
        actual: usize,
        maximum: usize,
    },
    #[error("expression source arithmetic overflow")]
    Arithmetic,
    #[error("expression source is truncated at {offset}")]
    Truncated { offset: usize },
    #[error("expression source has trailing bytes at {offset}")]
    Trailing { offset: usize },
    #[error("expression source has invalid {kind} at {offset}")]
    Invalid { kind: &'static str, offset: usize },
    #[error("expression source {kind} reference {actual} is outside 1..={maximum}")]
    Reference {
        kind: &'static str,
        actual: u32,
        maximum: usize,
    },
    #[error("expression source {kind} namespace has {actual} rows instead of {expected}")]
    Namespace {
        kind: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("expression source needs one complete root")]
    Root,
    #[error("expression source contains a foreign or reordered {kind} handle")]
    Handle { kind: &'static str },
    #[error("expression source literal is invalid: {0}")]
    Literal(#[from] RuntimeLiteralSourceError),
    #[error("expression source pattern is invalid: {0}")]
    Pattern(#[from] RuntimePatternSourceError),
    #[error("expression source construction failed: {0}")]
    Construction(#[from] RuntimePlanBuildError),
    #[error("expression source identity is invalid: {0}")]
    Identity(#[from] crate::entry::RuntimeIdentityError),
    #[error("expression source scope name is invalid: {0}")]
    Name(#[from] arcweft_id::DeclarationNameError),
    #[error("expression source custom dialogue field is invalid: {0}")]
    DialogueField(#[from] arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldIdError),
    #[error("expression source needs {required} output bytes, only {remaining} are reserved")]
    OutputCapacity { required: usize, remaining: usize },
}
type Error = RuntimeExpressionSourceError;
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Footprint {
    storage: usize,
    elements: usize,
}
struct Counts {
    limits: RuntimeExpressionSourceLimits,
    footprint: Footprint,
}
impl Counts {
    fn new(limits: RuntimeExpressionSourceLimits, nodes: usize) -> Result<Self, Error> {
        limit("nodes", nodes, limits.nodes as usize)?;
        if nodes == 0 {
            return Err(Error::Root);
        }
        let mut result = Self {
            limits,
            footprint: Footprint::default(),
        };
        // Sum actual retained and scratch phases conservatively. The seed
        // arena, moved recursive seeds, final Arc nodes and prefix/index walk
        // can therefore never exceed the admitted source storage allowance.
        result.array(
            nodes,
            2 * size_of::<RuntimeExprSeed>()
                + RuntimeExpr::source_node_storage_bytes()
                + size_of::<crate::value::RuntimeExpressionChildren<'_>>()
                + size_of::<usize>()
                + size_of::<&RuntimeExpr>()
                + size_of::<PrefixScope>()
                + size_of::<cursor::Row<'_>>(),
        )?;
        Ok(result)
    }
    fn storage(&mut self, bytes: usize) -> Result<(), Error> {
        self.footprint.storage = self
            .footprint
            .storage
            .checked_add(bytes)
            .ok_or(Error::Arithmetic)?;
        limit(
            "decoded bytes",
            self.footprint.storage,
            self.limits.decoded_bytes,
        )
    }
    fn array(&mut self, count: usize, width: usize) -> Result<(), Error> {
        self.footprint.elements = self
            .footprint
            .elements
            .checked_add(count)
            .ok_or(Error::Arithmetic)?;
        limit(
            "elements",
            self.footprint.elements,
            self.limits.elements as usize,
        )?;
        self.storage(count.checked_mul(width).ok_or(Error::Arithmetic)?)
    }
    fn nested(&mut self, storage: usize, elements: usize) -> Result<(), Error> {
        self.array(elements, 0)?;
        self.storage(storage)
    }
    fn literal_limits(&self) -> RuntimeLiteralSourceLimits {
        RuntimeLiteralSourceLimits {
            decoded_bytes: (self.limits.decoded_bytes - self.footprint.storage) / 2,
            elements: (self.limits.elements as usize - self.footprint.elements) as u32,
            ..self.limits.literal()
        }
    }
    fn pattern_limits(&self) -> RuntimePatternSourceLimits {
        RuntimePatternSourceLimits {
            decoded_bytes: self.limits.decoded_bytes - self.footprint.storage,
            elements: (self.limits.elements as usize - self.footprint.elements) as u32,
            ..self.limits.pattern()
        }
    }
}
struct PrefixScope {
    remaining: usize,
    depth: usize,
}

#[derive(Clone, Copy)]
enum Array {
    Guards,
    Fields,
    Displacement,
    Children,
    Arguments,
    Effects,
    Format,
    Dialogue,
    Record,
    Arms,
}
impl Array {
    fn width(self) -> usize {
        use crate::plan::*;
        use crate::value::*;
        match self {
            Self::Guards => {
                3 * size_of::<RuntimeLocalSeedId>()
                    + size_of::<crate::runtime_id::RuntimeLocalDeclarationId>()
            }
            Self::Fields => {
                size_of::<RuntimeRecordFieldSeedId>() + size_of::<RuntimeRecordFieldId>()
            }
            Self::Displacement => {
                size_of::<RuntimeDisplacedField<RuntimeRecordFieldSeedId>>()
                    + size_of::<RuntimeDisplacedField<RuntimeRecordFieldId>>()
            }
            Self::Children => size_of::<RuntimeExprSeed>() + size_of::<RuntimeExpr>(),
            Self::Arguments => {
                size_of::<RuntimeCallArgumentSeed>() + size_of::<RuntimeCallArgument>()
            }
            Self::Effects => {
                size_of::<RuntimeDialogueEffectSiteSeed>()
                    + size_of::<RuntimeDialogueContentEffectBindingExpr>()
            }
            Self::Format => {
                size_of::<RuntimeFormatContentOperandSeed>()
                    + size_of::<RuntimeFormatContentOperand>()
            }
            Self::Dialogue => {
                size_of::<
                    arcweft_interaction_model::dialogue::CharacterDialoguePatchField<
                        RuntimeExprSeed,
                    >,
                >() + size_of::<
                    arcweft_interaction_model::dialogue::CharacterDialoguePatchField<RuntimeExpr>,
                >()
            }
            Self::Record => {
                size_of::<RuntimeNominalRecordFieldSeed>()
                    + size_of::<RuntimeNominalRecordFieldExpr>()
            }
            Self::Arms => size_of::<RuntimeExprMatchArmSeed>() + size_of::<RuntimeExprMatchArm>(),
        }
    }
}

pub(in crate::plan) struct PreparedRuntimeExpressionSource<'a> {
    bytes: &'a [u8],
    limits: RuntimeExpressionSourceLimits,
    nodes: usize,
    footprint: Footprint,
}
impl PreparedRuntimeExpressionSource<'_> {
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.nodes
    }
}
impl RuntimeExpr {
    pub(in crate::plan) fn preflight_sources<'a>(
        bytes: &'a [u8],
        limits: RuntimeExpressionSourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeExpressionSource<'a>, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge(meter, bytes.len())?;
            let mut cursor = cursor::Cursor::new(bytes, limits);
            let nodes = cursor.header()?;
            let mut counts = Counts::new(limits, nodes)?;
            let mut scopes = Vec::with_capacity(nodes);
            scopes.push(PrefixScope {
                remaining: 1,
                depth: 0,
            });
            let mut remaining_roles = 1usize;
            for index in 0..nodes {
                while scopes.last().is_some_and(|scope| scope.remaining == 0) {
                    scopes.pop();
                }
                let Some(scope) = scopes.last_mut() else {
                    return Err(Error::Root);
                };
                scope.remaining -= 1;
                let depth = scope.depth;
                limit("depth", depth, limits.depth as usize)?;
                let row = cursor.row(&mut counts, resources, meter)?;
                remaining_roles = remaining_roles
                    .checked_sub(1)
                    .and_then(|n| n.checked_add(row.children))
                    .ok_or(Error::Arithmetic)?;
                // Reject impossible prefixes before growing the scope stack
                // beyond the complete header's admitted node capacity.
                if remaining_roles > nodes - index - 1 {
                    return Err(Error::Root);
                }
                if row.children != 0 {
                    scopes.push(PrefixScope {
                        remaining: row.children,
                        depth: depth.checked_add(1).ok_or(Error::Arithmetic)?,
                    });
                }
            }
            if scopes.iter().any(|scope| scope.remaining != 0) {
                return Err(Error::Root);
            }
            cursor.finish()?;
            Ok(PreparedRuntimeExpressionSource {
                bytes,
                limits,
                nodes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
fn limit(kind: &'static str, actual: usize, maximum: usize) -> Result<(), Error> {
    if actual > maximum {
        return Err(Error::Limit {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn reference(kind: &'static str, actual: u32, maximum: usize) -> Result<(), Error> {
    if actual == 0 || actual as usize > maximum {
        return Err(Error::Reference {
            kind,
            actual,
            maximum,
        });
    }
    Ok(())
}
fn charge(meter: &mut TaskSemanticMeter, bytes: usize) -> Result<(), Error> {
    meter.charge_work(
        u64::try_from(bytes)
            .map_err(|_| Error::Arithmetic)?
            .checked_mul(3)
            .ok_or(Error::Arithmetic)?,
    )?;
    Ok(())
}
