//! Borrowed request metadata and expression frames. Inspection allocates no
//! request strings, type arrays or argument seeds.

use super::{Counts, DOMAIN, Error, RuntimePlanSourceConstruction};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::{
    AssetLoadKind, CancelScopeId, HostCapabilityId, HostRestartPolicy, NeedProducerContractDigest,
    NeedProducerPlanError, NeedProducerRequestProjection, NeedProducerSiteDigest,
    NeedProducerTaskPlan, RuntimeHostArgumentTemplate, RuntimeRequestRoleIdentity, TaskClass,
    TaskPolicy, TaskPriority,
};

pub(super) enum Request<'a> {
    Asset {
        kind: AssetLoadKind,
        name: &'a str,
    },
    External {
        capability: &'a str,
        operation: &'a str,
        contract: [u8; 32],
        names: &'a [u8],
        count: usize,
    },
}
pub(super) struct Schema<'a> {
    contract: [u8; 32],
    site: [u8; 32],
    policy: TaskPolicy,
    restart: HostRestartPolicy,
    class: TaskClass,
    priority: i32,
    cancel_scope: &'a str,
    request: Request<'a>,
    types: &'a [u8],
    payload: u32,
}
impl Schema<'_> {
    pub fn bindings(&self) -> Bindings<'_> {
        match &self.request {
            Request::Asset { name, .. } => Bindings::Asset(Some(name)),
            Request::External { names, .. } => Bindings::External(Cursor::new(names)),
        }
    }
    pub fn materialize(
        &self,
        source: &RuntimePlanSourceConstruction,
    ) -> Result<NeedProducerTaskPlan, Error> {
        let request = match &self.request {
            Request::Asset { kind, name } => NeedProducerRequestProjection::AssetLoad {
                kind: *kind,
                argument_name: (*name).to_owned(),
            },
            Request::External {
                capability,
                operation,
                contract,
                names,
                count,
            } => {
                let mut cursor = Cursor::new(names);
                let mut argument_names = Vec::with_capacity(*count);
                for _ in 0..*count {
                    argument_names.push(cursor.optional_text()?.map(str::to_owned));
                }
                cursor.finish()?;
                NeedProducerRequestProjection::ExternCapability {
                    capability: HostCapabilityId((*capability).to_owned()),
                    operation: (*operation).to_owned(),
                    contract: crate::step::HostCallContractDigest::from_bytes(*contract),
                    argument_names: argument_names.into_boxed_slice(),
                }
            }
        };
        let semantic = |id| {
            let id = RuntimePlanTypeId::from_accepted_ordinal(
                std::num::NonZeroU32::new(id).expect("preflighted positive type coordinate"),
            );
            source
                .builder
                .types
                .get(id)
                .expect("authenticated original type namespace")
                .semantic_identity()
        };
        let types = self
            .types
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| semantic(u32::from_le_bytes(*bytes)))
            .collect();
        Ok(NeedProducerTaskPlan::try_new(
            NeedProducerContractDigest::from_bytes(self.contract),
            NeedProducerSiteDigest::from_bytes(self.site),
            request,
            types,
            semantic(self.payload),
            self.policy,
            self.restart,
            self.class.clone(),
            TaskPriority(self.priority),
            CancelScopeId(self.cancel_scope.to_owned()),
        )?)
    }
}
pub(super) enum Bindings<'a> {
    Asset(Option<&'a str>),
    External(Cursor<'a>),
}
impl Bindings<'_> {
    pub fn check_next(&mut self, actual: Option<&str>) -> Result<(), Error> {
        let valid = match self {
            Self::Asset(name) => name.take().is_some_and(|expected| {
                crate::task::NeedProducerArgumentBinding::NamedOrPositional(expected)
                    .accepts_source_name(actual)
            }),
            Self::External(cursor) => match cursor.optional_text()? {
                Some(name) => crate::task::NeedProducerArgumentBinding::Named(name),
                None => crate::task::NeedProducerArgumentBinding::Positional,
            }
            .accepts_source_name(actual),
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Invalid {
                kind: "argument binding",
                offset: 0,
            })
        }
    }
}
pub(super) struct Row<'a> {
    pub task: u32,
    pub schema: Schema<'a>,
    pub binding: &'a [u8],
    pub arguments: &'a [u8],
    pub argument_count: usize,
}
pub(super) struct Argument<'a> {
    pub identity: RuntimeRequestRoleIdentity,
    pub name: Option<&'a str>,
    pub value: &'a [u8],
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn invalid(&self, kind: &'static str) -> Error {
        Error::Invalid {
            kind,
            offset: self.offset,
        }
    }
    fn raw(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn digest(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.raw(32)?.try_into().expect("source identity width"))
    }
    fn frame(&mut self) -> Result<&'a [u8], Error> {
        let n = self.u32()? as usize;
        self.raw(n)
    }
    fn text(&mut self) -> Result<&'a str, Error> {
        Ok(std::str::from_utf8(self.frame()?)?)
    }
    fn nonempty_text(&mut self) -> Result<&'a str, Error> {
        let text = self.text()?;
        if text.is_empty() {
            return Err(self.invalid("empty request identity"));
        }
        Ok(text)
    }
    fn optional_text(&mut self) -> Result<Option<&'a str>, Error> {
        match self.byte()? {
            0 => Ok(None),
            1 => Ok(Some(self.nonempty_text()?)),
            _ => Err(self.invalid("optional name")),
        }
    }
    fn ty(&mut self, count: usize) -> Result<u32, Error> {
        let actual = self.u32()?;
        if actual == 0 || actual as usize > count {
            return Err(Error::Reference {
                kind: "type",
                actual,
                maximum: count,
            });
        }
        Ok(actual)
    }
    fn request(&mut self, counts: &mut Counts, contract: [u8; 32]) -> Result<Request<'a>, Error> {
        Ok(match self.byte()? {
            tag @ (0 | 1) => {
                let name = self.nonempty_text()?;
                counts.text(name.len())?;
                Request::Asset {
                    kind: if tag == 0 {
                        AssetLoadKind::Image
                    } else {
                        AssetLoadKind::Voice
                    },
                    name,
                }
            }
            2 => {
                let capability = self.nonempty_text()?;
                let operation = self.nonempty_text()?;
                counts.text(capability.len())?;
                counts.text(operation.len())?;
                let request_contract = self.digest()?;
                if request_contract != contract {
                    return Err(NeedProducerPlanError::ExternalContractMismatch.into());
                }
                let count = self.u32()? as usize;
                counts.array(count, 2 * size_of::<Option<String>>())?;
                let start = self.offset;
                for _ in 0..count {
                    if let Some(name) = self.optional_text()? {
                        counts.text(name.len())?;
                    }
                }
                Request::External {
                    capability,
                    operation,
                    contract: request_contract,
                    names: &self.bytes[start..self.offset],
                    count,
                }
            }
            _ => return Err(self.invalid("request family")),
        })
    }
    pub fn row(&mut self, counts: &mut Counts) -> Result<Row<'a>, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let task = self.u32()?;
        if task as usize >= counts.limits.task_count {
            return Err(Error::Reference {
                kind: "task",
                actual: task,
                maximum: counts.limits.task_count,
            });
        }
        let contract = self.digest()?;
        let site = self.digest()?;
        let policy =
            TaskPolicy::from_semantic_tag(self.byte()?).map_err(|_| self.invalid("task policy"))?;
        let restart = match self.byte()? {
            0 => HostRestartPolicy::MustBeQuiescent,
            1 => HostRestartPolicy::Restartable,
            _ => return Err(self.invalid("restart policy")),
        };
        let class =
            TaskClass::from_semantic_tag(self.byte()?).ok_or_else(|| self.invalid("task class"))?;
        let priority = self.u32()?.cast_signed();
        let cancel_scope = self.text()?;
        counts.text(cancel_scope.len())?;
        let request = self.request(counts, contract)?;
        let payload = self.ty(counts.limits.expression.type_count)?;
        let count = self.u32()? as usize;
        counts.array(
            count,
            2 * size_of::<crate::pattern::RuntimeSemanticTypeId>(),
        )?;
        let expected = match &request {
            Request::Asset { .. } => 1,
            Request::External { count, .. } => *count,
        };
        if count != expected {
            return Err(NeedProducerPlanError::ArgumentTypeCountMismatch.into());
        }
        let start = self.offset;
        for _ in 0..count {
            self.ty(counts.limits.expression.type_count)?;
        }
        let types = &self.bytes[start..self.offset];
        let binding = self.frame()?;
        let argument_count = self.u32()? as usize;
        if argument_count != count {
            return Err(NeedProducerPlanError::ArgumentTypeCountMismatch.into());
        }
        counts.array(
            argument_count,
            size_of::<crate::plan::RuntimeHostArgumentSeed>()
                + size_of::<RuntimeHostArgumentTemplate>(),
        )?;
        let start = self.offset;
        for _ in 0..argument_count {
            let argument = self.argument()?;
            if let Some(name) = argument.name {
                counts.text(name.len())?;
            }
        }
        Ok(Row {
            task,
            schema: Schema {
                contract,
                site,
                policy,
                restart,
                class,
                priority,
                cancel_scope,
                request,
                types,
                payload,
            },
            binding,
            arguments: &self.bytes[start..self.offset],
            argument_count,
        })
    }
    pub fn argument(&mut self) -> Result<Argument<'a>, Error> {
        let identity = RuntimeRequestRoleIdentity::from_accepted_identity(self.digest()?);
        let name = self.optional_text()?;
        let value = self.frame()?;
        Ok(Argument {
            identity,
            name,
            value,
        })
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(Error::Trailing {
                offset: self.offset,
            })
        }
    }
}
