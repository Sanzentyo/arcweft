//! Borrowed finite checked-type storage admission for standalone task outcomes.

use super::{RuntimeCheckedType, RuntimeVariantIdentity};
use crate::entry::RuntimeSchemaError;
use crate::plan::RuntimeAgentTypeProjection;
use crate::task::descriptor::TaskDescriptorVisitor;

impl RuntimeCheckedType {
    pub(crate) fn visit_standalone_descriptor(
        &self,
        visitor: &mut impl TaskDescriptorVisitor,
        depth: usize,
    ) -> Result<(), RuntimeSchemaError> {
        let mut pending = vec![(self, depth)];
        while let Some((ty, depth)) = pending.pop() {
            // Type kinds occupy one byte in retained descriptor counting.
            // This is a counting sink; semantic identity retains its own encoder.
            visitor.fixed(&[0], depth)?;
            let next = depth + 1;
            match ty {
                Self::ExecutableRef(_)
                | Self::Nominal { .. }
                | Self::Variant {
                    owner: RuntimeVariantIdentity::Nominal { .. },
                    ..
                }
                | Self::Agent(RuntimeAgentTypeProjection::DataShape(_)) => {
                    return Err(RuntimeSchemaError::Encoding {
                        message: "standalone task outcome requires a finite type without program-owned rows".into(),
                    });
                }
                Self::Array { item, length } => {
                    visitor.fixed(&length.to_le_bytes(), next)?;
                    pending.push((item, next));
                }
                Self::Agent(agent @ RuntimeAgentTypeProjection::Probe(child)) => {
                    visitor.fixed(&[agent.operational_type().semantic_tag()], next)?;
                    pending.push((child, next));
                }
                Self::Sequence(child) | Self::Option(child) => {
                    pending.push((child, next));
                }
                Self::Map { key, value, .. } => {
                    visitor.fixed(&[0], next)?;
                    pending.push((value, next));
                    pending.push((key, next));
                }
                Self::Result { ok, error } => {
                    pending.push((error, next));
                    pending.push((ok, next));
                }
                Self::Tuple(items) | Self::Choice(items) => {
                    visitor.collection(items.len(), next)?;
                    pending.extend(items.iter().rev().map(|child| (child, next)));
                }
                Self::Record(fields) => {
                    visitor.collection(fields.len(), next)?;
                    for field in fields.iter().rev() {
                        visitor.fixed(&field.field().zero_based().to_le_bytes(), next)?;
                        visitor.string(field.diagnostic_name(), next)?;
                        pending.push((field.ty(), next));
                    }
                }
                Self::Variant {
                    owner: RuntimeVariantIdentity::Builtin(owner),
                    arguments,
                    cases,
                    ..
                } => {
                    visitor.fixed(&[1, owner.semantic_tag()], next)?;
                    visitor.collection(arguments.len(), next)?;
                    visitor.collection(cases.len(), next)?;
                    for case in cases.iter().rev() {
                        visitor.string(&case.name, next)?;
                        visitor.fixed(&[u8::from(case.payload.is_some())], next)?;
                        if let Some(payload) = &case.payload {
                            pending.push((payload, next));
                        }
                    }
                    pending.extend(arguments.iter().rev().map(|child| (child, next)));
                }
                Self::Opaque { owner } => {
                    visitor.string(owner.producer().as_str(), next)?;
                    visitor.fixed(owner.semantic_identity().as_bytes(), next)?;
                    visitor.fixed(
                        &[
                            owner.value_class().semantic_tag(),
                            owner.persistence().semantic_tag(),
                            owner.admission().encoded(),
                        ],
                        next,
                    )?;
                }
                Self::Signed(_) | Self::Unsigned(_) => visitor.fixed(&[0], next)?,
                Self::DenseNumeric(dense) => visitor.fixed(&dense.semantic_parts(), next)?,
                Self::Agent(agent) => {
                    visitor.fixed(&[agent.operational_type().semantic_tag()], next)?;
                }
                Self::Never
                | Self::Unit
                | Self::Bool
                | Self::F32
                | Self::F64
                | Self::String
                | Self::Color
                | Self::Char
                | Self::Duration
                | Self::Progress
                | Self::EntityReference
                | Self::AgentValue
                | Self::Bytes => {}
            }
        }
        Ok(())
    }
}
