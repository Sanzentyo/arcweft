//! Allocation-free framing of the original, distinct callable declarations.

use super::{DOMAIN, Error, RuntimePureCallableSourceLimits, limit, reference};
use crate::plan::{
    RuntimeFunctionDefinitionIdentity, RuntimeFunctionParameterIdentity,
    RuntimeFunctionParameterPassing, RuntimePureHelperOrigin, RuntimePureInputType,
    RuntimePureOutputType, RuntimeReceiverMode, RuntimeTraitMethodIdentity,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use arcweft_id::runtime_program::RuntimePureProgramId;

pub(super) const INPUT_BYTES: usize = 32 + 4 + 1 + 1;
pub(super) struct Input {
    pub(super) identity: RuntimeFunctionParameterIdentity,
    pub(super) local: RuntimeLocalDeclarationId,
    pub(super) passing: RuntimeFunctionParameterPassing,
    pub(super) abi: RuntimePureInputType,
}
pub(super) struct Helper<'a> {
    pub(super) definition: RuntimeFunctionDefinitionIdentity,
    pub(super) result: RuntimePlanTypeId,
    pub(super) output: RuntimePureOutputType,
    pub(super) name: &'a str,
    pub(super) scalar: bool,
    pub(super) origin: RuntimePureHelperOrigin,
    pub(super) inputs: &'a [u8],
}
pub(super) struct Method<'a> {
    pub(super) definition: RuntimeFunctionDefinitionIdentity,
    pub(super) result: RuntimePlanTypeId,
    pub(super) output: RuntimePureOutputType,
    pub(super) receiver: RuntimeReceiverMode,
    impl_id: usize,
    trait_id: Option<usize>,
    witness: Option<usize>,
    trait_name: Option<&'a str>,
    self_type: &'a str,
    method_name: &'a str,
    monomorph_label: &'a str,
    pub(super) inputs: &'a [u8],
}
impl Method<'_> {
    pub(super) fn label_bytes(&self) -> Result<usize, Error> {
        [
            self.trait_name.map_or(0, str::len),
            self.self_type.len(),
            self.method_name.len(),
            self.monomorph_label.len(),
        ]
        .into_iter()
        .try_fold(0usize, |total, n| {
            total.checked_add(n).ok_or(Error::Arithmetic)
        })
    }
    pub(super) fn identity(&self) -> RuntimeTraitMethodIdentity {
        RuntimeTraitMethodIdentity {
            impl_id: self.impl_id,
            trait_id: self.trait_id,
            witness: self.witness,
            trait_name: self.trait_name.map(str::to_owned),
            self_type: self.self_type.to_owned(),
            method_name: self.method_name.to_owned(),
            monomorph_label: self.monomorph_label.to_owned(),
        }
    }
}
pub(super) struct Binding {
    pub(super) program: RuntimePureProgramId,
    pub(super) function: u32,
    pub(super) inputs: usize,
}
pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
    limits: RuntimePureCallableSourceLimits,
}
impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: RuntimePureCallableSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    fn slice(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Arithmetic)?;
        let result = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.slice(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.slice(4)?.try_into().expect("source u32 width"),
        ))
    }
    fn index(&mut self) -> Result<usize, Error> {
        usize::try_from(u64::from_le_bytes(
            self.slice(8)?.try_into().expect("source u64 width"),
        ))
        .map_err(|_| Error::Arithmetic)
    }
    fn identity(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.slice(32)?.try_into().expect("source identity width"))
    }
    fn flag(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(Error::Tag { kind: "flag", tag }),
        }
    }
    fn optional_index(&mut self) -> Result<Option<usize>, Error> {
        if self.flag()? {
            Ok(Some(self.index()?))
        } else {
            Ok(None)
        }
    }
    fn string(&mut self) -> Result<&'a str, Error> {
        let n = self.u32()? as usize;
        Ok(std::str::from_utf8(self.slice(n)?)?)
    }
    fn optional_string(&mut self) -> Result<Option<&'a str>, Error> {
        if self.flag()? {
            Ok(Some(self.string()?))
        } else {
            Ok(None)
        }
    }
    fn count(&mut self, kind: &'static str, max: usize) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        limit(kind, n, max)?;
        Ok(n)
    }
    fn ordinal(&mut self, index: usize) -> Result<(), Error> {
        let expected = u32::try_from(index + 1).map_err(|_| Error::Arithmetic)?;
        let actual = self.u32()?;
        if actual != expected {
            return Err(Error::Ordinal { expected, actual });
        }
        Ok(())
    }
    fn result(&mut self) -> Result<RuntimePlanTypeId, Error> {
        let actual = self.u32()?;
        reference("type", actual, self.limits.type_count)?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(
            std::num::NonZeroU32::new(actual).expect("checked type coordinate"),
        ))
    }
    fn output(&mut self) -> Result<RuntimePureOutputType, Error> {
        let tag = self.byte()?;
        RuntimePureOutputType::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "output ABI",
            tag,
        })
    }
    fn inputs(&mut self) -> Result<&'a [u8], Error> {
        let count = self.count("inputs", self.limits.elements as usize)?;
        let start = self.offset;
        for _ in 0..count {
            self.input()?;
        }
        Ok(&self.bytes[start..self.offset])
    }
    pub(super) fn input(&mut self) -> Result<Input, Error> {
        let identity = RuntimeFunctionParameterIdentity::from_accepted_identity(self.identity()?);
        let actual = self.u32()?;
        reference("local", actual, self.limits.local_count)?;
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(
            std::num::NonZeroU32::new(actual).expect("checked local coordinate"),
        );
        let tag = self.byte()?;
        let passing =
            RuntimeFunctionParameterPassing::from_semantic_tag(tag).ok_or(Error::Tag {
                kind: "input passing",
                tag,
            })?;
        let tag = self.byte()?;
        let abi = RuntimePureInputType::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "input ABI",
            tag,
        })?;
        Ok(Input {
            identity,
            local,
            passing,
            abi,
        })
    }
    pub(super) fn header(&mut self) -> Result<usize, Error> {
        if self.slice(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        self.count("helpers", self.limits.helpers as usize)
    }
    pub(super) fn helper(&mut self, index: usize) -> Result<Helper<'a>, Error> {
        self.ordinal(index)?;
        let definition =
            RuntimeFunctionDefinitionIdentity::from_accepted_identity(self.identity()?);
        let result = self.result()?;
        let output = self.output()?;
        let name = self.string()?;
        let scalar = self.flag()?;
        let origin = match self.byte()? {
            0 => RuntimePureHelperOrigin::Annotated,
            1 => RuntimePureHelperOrigin::Inferred,
            tag => {
                return Err(Error::Tag {
                    kind: "helper origin",
                    tag,
                });
            }
        };
        let inputs = self.inputs()?;
        Ok(Helper {
            definition,
            result,
            output,
            name,
            scalar,
            origin,
            inputs,
        })
    }
    pub(super) fn method_count(&mut self) -> Result<usize, Error> {
        self.count("methods", self.limits.methods as usize)
    }
    pub(super) fn method(&mut self, index: usize) -> Result<Method<'a>, Error> {
        self.ordinal(index)?;
        let definition =
            RuntimeFunctionDefinitionIdentity::from_accepted_identity(self.identity()?);
        let result = self.result()?;
        let output = self.output()?;
        let receiver = match self.byte()? {
            0 => RuntimeReceiverMode::Owned,
            1 => RuntimeReceiverMode::SharedRef,
            2 => RuntimeReceiverMode::MutRef,
            tag => {
                return Err(Error::Tag {
                    kind: "receiver",
                    tag,
                });
            }
        };
        let impl_id = self.index()?;
        let trait_id = self.optional_index()?;
        let witness = self.optional_index()?;
        let trait_name = self.optional_string()?;
        let self_type = self.string()?;
        let method_name = self.string()?;
        let monomorph_label = self.string()?;
        let inputs = self.inputs()?;
        Ok(Method {
            definition,
            result,
            output,
            receiver,
            impl_id,
            trait_id,
            witness,
            trait_name,
            self_type,
            method_name,
            monomorph_label,
            inputs,
        })
    }
    pub(super) fn binding_count(&mut self) -> Result<usize, Error> {
        self.count("program bindings", self.limits.bindings as usize)
    }
    pub(super) fn binding(&mut self, index: usize) -> Result<Binding, Error> {
        self.ordinal(index)?;
        let program = RuntimePureProgramId::from_checked_digest(self.identity()?);
        let function = self.u32()?;
        reference("function", function, self.limits.function_count)?;
        let inputs = self.count("program inputs", self.limits.elements as usize)?;
        Ok(Binding {
            program,
            function,
            inputs,
        })
    }
    pub(super) fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing);
        }
        Ok(())
    }
}
