use super::{
    BodyOwner, Counts, DOMAIN, Error, Footprint, Namespaces, RuntimeFunctionBodySourceLimits,
    charge, limit, reading,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::plan::{
    RuntimeExecutableBody, RuntimeFunctionDefinitionIdentity, RuntimeFunctionSiteBody, RuntimePlan,
};
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeExpr;

// Borrowed projections of the actual owning tables; no copied body catalog.
enum Body<'a> {
    Expression(&'a RuntimeExpr),
    Executable(&'a RuntimeExecutableBody),
}
fn bodies(
    plan: &RuntimePlan,
) -> impl Iterator<Item = (BodyOwner, RuntimeFunctionDefinitionIdentity, Body<'_>)> {
    plan.function_sites()
        .iter()
        .enumerate()
        .map(|(index, function)| {
            let body = match function.body() {
                RuntimeFunctionSiteBody::Expression(expr) => Body::Expression(expr),
                RuntimeFunctionSiteBody::Executable(body) => Body::Executable(body),
            };
            (BodyOwner::Function(index), function.definition(), body)
        })
        .chain(
            plan.pure_helpers()
                .iter()
                .enumerate()
                .map(|(index, helper)| {
                    (
                        BodyOwner::Helper(index),
                        helper.definition,
                        Body::Expression(&helper.expr),
                    )
                }),
        )
        .chain(
            plan.trait_methods()
                .iter()
                .enumerate()
                .map(|(index, method)| {
                    (
                        BodyOwner::Method(index),
                        method.definition,
                        Body::Expression(&method.body),
                    )
                }),
        )
}

pub(in crate::plan) struct EncodedRuntimeFunctionBodySources {
    bytes: Vec<u8>,
    footprint: Footprint,
}
impl EncodedRuntimeFunctionBodySources {
    pub(in crate::plan) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(in crate::plan) const fn node_count(&self) -> usize {
        self.footprint.nodes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.footprint.elements
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.footprint.storage
    }
    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.bytes.len() {
            return Err(Error::OutputCapacity {
                required: self.bytes.len(),
                remaining,
            });
        }
        output.extend_from_slice(&self.bytes);
        Ok(())
    }
}
impl RuntimePlan {
    pub(in crate::plan) fn encode_function_body_sources(
        &self,
        limits: RuntimeFunctionBodySourceLimits,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<EncodedRuntimeFunctionBodySources, Error> {
        meter.status()?;
        let result = (|| {
            let namespaces = Namespaces {
                functions: self.function_sites().len(),
                helpers: self.pure_helpers().len(),
                methods: self.trait_methods().len(),
            };
            let mut counts = Counts::new(limits, namespaces)?;
            let mut length = DOMAIN.len().checked_add(13).ok_or(Error::Arithmetic)?;
            limit("encoded bytes", length, limits.encoded_bytes)?;
            for (owner, definition, body) in bodies(self) {
                let packet = encode_row(
                    self,
                    owner,
                    definition,
                    body,
                    &counts,
                    limits.encoded_bytes - length,
                    resources,
                    meter,
                )?;
                let mut cursor = reading::Cursor::new(&packet);
                cursor.row(owner, &mut counts, resources, meter)?;
                cursor.finish()?;
                length = length.checked_add(packet.len()).ok_or(Error::Arithmetic)?;
                limit("encoded bytes", length, limits.encoded_bytes)?;
            }
            charge(meter, length)?;
            let mut output = Writer {
                bytes: Vec::with_capacity(length),
                maximum: length,
            };
            output.raw(DOMAIN)?;
            output.byte(1)?;
            output.count(namespaces.functions)?;
            output.count(namespaces.helpers)?;
            output.count(namespaces.methods)?;
            let mut emitted = Counts::new(limits, namespaces)?;
            for (owner, definition, body) in bodies(self) {
                let packet = encode_row(
                    self,
                    owner,
                    definition,
                    body,
                    &emitted,
                    limits.encoded_bytes - output.bytes.len(),
                    resources,
                    meter,
                )?;
                let mut cursor = reading::Cursor::new(&packet);
                cursor.row(owner, &mut emitted, resources, meter)?;
                cursor.finish()?;
                output.raw(&packet)?;
            }
            debug_assert_eq!(output.bytes.len(), length);
            debug_assert_eq!(emitted.footprint, counts.footprint);
            Ok(EncodedRuntimeFunctionBodySources {
                bytes: output.bytes,
                footprint: counts.footprint,
            })
        })();
        result.map_err(|error: Error| error.reject(meter))
    }
}
fn encode_row(
    plan: &RuntimePlan,
    owner: BodyOwner,
    definition: RuntimeFunctionDefinitionIdentity,
    body: Body<'_>,
    counts: &Counts,
    maximum: usize,
    resources: &ValueResourceAdmission,
    meter: &mut TaskSemanticMeter,
) -> Result<Vec<u8>, Error> {
    let mut counts = *counts;
    if let Body::Executable(body) = &body {
        let bytes = body.effects().iter().try_fold(0_usize, |total, effect| {
            total
                .checked_add(effect.as_str().len())
                .ok_or(Error::Arithmetic)
        })?;
        counts.effects(body.effects().len(), bytes)?;
    }
    let mut output = Writer {
        bytes: Vec::new(),
        maximum,
    };
    output.count(owner.index().checked_add(1).ok_or(Error::Arithmetic)?)?;
    output.raw(definition.as_bytes())?;
    match body {
        Body::Expression(expr) => {
            output.byte(0)?;
            output.count(0)?;
            let child = expr.encode_sources(
                counts.expression(output.frame_capacity()?),
                resources,
                meter,
            )?;
            output.frame(child.bytes())?;
        }
        Body::Executable(body) => {
            output.byte(1)?;
            output.count(body.effects().len())?;
            for effect in body.effects().iter() {
                output.frame(effect.as_str().as_bytes())?;
            }
            let child = plan.encode_flow_source(
                body.ops(),
                counts.flow(output.frame_capacity()?),
                resources,
                meter,
            )?;
            output.frame(child.bytes())?;
        }
    }
    Ok(output.bytes)
}
struct Writer {
    bytes: Vec<u8>,
    maximum: usize,
}
impl Writer {
    fn frame_capacity(&self) -> Result<usize, Error> {
        self.maximum
            .checked_sub(self.bytes.len())
            .and_then(|remaining| remaining.checked_sub(4))
            .ok_or(Error::Limit {
                kind: "encoded bytes",
                actual: self.bytes.len().checked_add(4).ok_or(Error::Arithmetic)?,
                maximum: self.maximum,
            })
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(Error::Arithmetic)?;
        limit("encoded bytes", length, self.maximum)?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), Error> {
        self.raw(&[byte])
    }
    fn count(&mut self, count: usize) -> Result<(), Error> {
        self.raw(
            &u32::try_from(count)
                .map_err(|_| Error::Arithmetic)?
                .to_le_bytes(),
        )
    }
    fn frame(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.count(bytes.len())?;
        self.raw(bytes)
    }
}
