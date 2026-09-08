//! Iterative encoding, including the independent hashes owned by payload cases.

use std::slice;

use super::{Encoder, GenericScope, GenericScopeError, SemanticTypeDigest, TypeKind};
use crate::effect_row::EffectRow;
use crate::types::{
    ArrayLength, TypeProjectionNodeKind, VariantPayloadType, VariantPayloadTypeChildren,
};

pub(super) enum EncodingTask<'ty> {
    Type(&'ty TypeKind, usize),
    Types(slice::Iter<'ty, TypeKind>, usize),
    Length(&'ty ArrayLength, usize),
    FunctionEnd {
        effects: &'ty EffectRow,
        enclosing: GenericScope,
        depth: usize,
    },
    ProjectionTail {
        trait_name: &'ty Option<String>,
        assoc: &'ty str,
    },
    PayloadChild(Box<PayloadEncoding<'ty>>),
}

/// Only payloads need an independent hash context. Ordinary type children
/// append to their parent's stream, and lists retain a cursor instead of
/// scheduling or copying every unvisited child in advance.
pub(super) struct PayloadEncoding<'ty> {
    parent: Encoder,
    payload: &'ty VariantPayloadType,
    children: VariantPayloadTypeChildren<'ty>,
    digests: Vec<SemanticTypeDigest>,
    depth: usize,
}

impl Encoder {
    pub(super) fn encode<C, E: From<GenericScopeError>>(
        ty: &TypeKind,
        scope: &GenericScope,
        control: &mut C,
        node: &impl Fn(&mut C, TypeProjectionNodeKind, u64) -> Result<(), E>,
        binding: &impl Fn(&mut C) -> Result<(), E>,
    ) -> Result<SemanticTypeDigest, E> {
        let mut encoder = Self::scoped(scope, control, binding)?;
        let mut tasks = vec![EncodingTask::Type(ty, 1)];
        while let Some(task) = tasks.pop() {
            match task {
                EncodingTask::Type(ty, depth) => {
                    node(control, TypeProjectionNodeKind::Type, depth_u64(depth))?;
                    if let Some(payload) = encoder.ty(ty, depth, &mut tasks, control, binding)? {
                        let nested = Self::scoped(&encoder.scope, control, binding)?;
                        let parent = std::mem::replace(&mut encoder, nested);
                        let mut children = payload.children();
                        let owner = children.next().expect("payload has one owner type");
                        let depth = depth.checked_add(1).expect("owned type nesting fits usize");
                        tasks.push(EncodingTask::PayloadChild(Box::new(PayloadEncoding {
                            parent,
                            payload,
                            children,
                            digests: Vec::new(),
                            depth,
                        })));
                        tasks.push(EncodingTask::Type(owner, depth));
                    }
                }
                EncodingTask::Types(mut children, depth) => {
                    if let Some(child) = children.next() {
                        tasks.push(EncodingTask::Types(children, depth));
                        tasks.push(EncodingTask::Type(child, depth));
                    }
                }
                EncodingTask::Length(length, depth) => {
                    node(control, TypeProjectionNodeKind::Const, depth_u64(depth))?;
                    encoder.array_length(length);
                }
                EncodingTask::FunctionEnd {
                    effects,
                    enclosing,
                    depth,
                } => {
                    encoder.effect_row(effects, depth, control, node)?;
                    encoder.scope = enclosing;
                }
                EncodingTask::ProjectionTail { trait_name, assoc } => {
                    encoder.option(trait_name.as_ref(), |encoder, value| encoder.string(value));
                    encoder.string(assoc);
                }
                EncodingTask::PayloadChild(mut payload) => {
                    payload
                        .digests
                        .push(SemanticTypeDigest(*encoder.finish()?.as_bytes()));
                    if let Some(child) = payload.children.next() {
                        encoder = Self::scoped(&payload.parent.scope, control, binding)?;
                        let depth = payload.depth;
                        tasks.push(EncodingTask::PayloadChild(payload));
                        tasks.push(EncodingTask::Type(child, depth));
                    } else {
                        let identity = payload
                            .payload
                            .semantic_case_from_type_digests(payload.digests);
                        encoder = payload.parent;
                        identity.write_payload_type_identity(&mut encoder.bytes);
                    }
                }
            }
            if let Some(error) = encoder.error.take() {
                return Err(error.into());
            }
        }
        Ok(SemanticTypeDigest(*encoder.finish()?.as_bytes()))
    }

    fn scoped<C, E>(
        scope: &GenericScope,
        control: &mut C,
        binding: &impl Fn(&mut C) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut encoder = Self::new(GenericScope::default());
        if !scope.binders().is_empty() {
            encoder.tag(96);
            encoder.len(scope.binders().len());
            for binder in scope.binders() {
                binding(control)?;
                encoder.u16(binder.types());
                encoder.u16(binder.const_lengths());
                encoder.u32(binder.effects());
            }
        }
        encoder.scope = scope.clone();
        Ok(encoder)
    }
}

pub(super) fn depth_u64(depth: usize) -> u64 {
    // Depth is derived only from a finite, owned TypeKind tree, never a wire
    // integer or a caller-supplied counter. Supported host indices fit u64.
    u64::try_from(depth).expect("owned type nesting fits u64")
}
