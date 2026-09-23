//! Bounded source traversal performed before the atomic catalog join.

use super::{
    NominalAggregationLimitKind, NominalAggregationLimits, NominalResolutionLimitKind,
    NominalResolutionLimits, RustMetadataJoinError, RustMetadataJoinErrorKind,
    RustTypeMetadataPublicationInput,
};
use crate::{
    env::rust_metadata::{
        RustStructMetadataInput, RustTypeMetadataPublicationKind, RustVariantPayloadInput,
    },
    registration::{EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode},
};

pub(super) fn validate(
    inputs: &[RustTypeMetadataPublicationInput],
    nominal: NominalResolutionLimits,
    aggregate: NominalAggregationLimits,
) -> Result<(), RustMetadataJoinError> {
    let mut work = 0_u64;
    for input in inputs {
        let mut admission = Admission {
            input,
            nominal,
            aggregate,
            work: &mut work,
            row_work: 0,
        };
        admission.charge(1)?;
        admission.name(&input.data_policy().name)?;
        match &input.data_policy().tag {
            arcweft_rust_abi::ArcweftRustEnumTagStyle::External => {}
            arcweft_rust_abi::ArcweftRustEnumTagStyle::Internal { tag } => admission.name(tag)?,
            arcweft_rust_abi::ArcweftRustEnumTagStyle::Adjacent { tag, content } => {
                admission.name(tag)?;
                admission.name(content)?;
            }
        }
        admission.reference_limit(
            NominalResolutionLimitKind::GenericArgumentsPerApplication,
            input.parameters().len() as u64,
            u64::from(nominal.generic_arguments_per_application()),
        )?;
        for parameter in input.parameters() {
            admission.name(parameter.name())?;
        }
        match input.kind() {
            RustTypeMetadataPublicationKind::Struct { shape } => match shape {
                RustStructMetadataInput::Unit => {}
                RustStructMetadataInput::Tuple(fields) => {
                    for field in fields {
                        admission.field(None, field)?;
                    }
                }
                RustStructMetadataInput::Record(fields) => {
                    for field in fields {
                        admission.field(Some(field.name()), field.ty())?;
                        admission.name(field.wire_name())?;
                        if let Some(super::super::RustFieldDefault::Function(path)) =
                            field.data_default()
                        {
                            admission.name(path.as_str())?;
                        }
                    }
                }
            },
            RustTypeMetadataPublicationKind::Newtype { inner } => admission.field(None, inner)?,
            RustTypeMetadataPublicationKind::Enum { variants } => {
                for variant in variants {
                    admission.name(variant.name())?;
                    admission.name(variant.wire_name())?;
                    match variant.payload() {
                        RustVariantPayloadInput::Unit => {}
                        RustVariantPayloadInput::Tuple(fields) => {
                            for field in fields {
                                admission.field(None, field)?;
                            }
                        }
                        RustVariantPayloadInput::Record(fields) => {
                            for field in fields {
                                admission.field(Some(field.name()), field.ty())?;
                                admission.name(field.wire_name())?;
                                if let Some(super::super::RustFieldDefault::Function(path)) =
                                    field.data_default()
                                {
                                    admission.name(path.as_str())?;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

struct Admission<'a, 'work> {
    input: &'a RustTypeMetadataPublicationInput,
    nominal: NominalResolutionLimits,
    aggregate: NominalAggregationLimits,
    work: &'work mut u64,
    row_work: u64,
}

impl Admission<'_, '_> {
    fn error(&self, kind: RustMetadataJoinErrorKind) -> RustMetadataJoinError {
        RustMetadataJoinError {
            declaration: self.input.id().clone(),
            source_span: self.input.source().clone(),
            kind,
        }
    }

    fn reference_limit(
        &self,
        kind: NominalResolutionLimitKind,
        observed: u64,
        maximum: u64,
    ) -> Result<(), RustMetadataJoinError> {
        if observed > maximum {
            Err(self.error(RustMetadataJoinErrorKind::ReferenceLimit {
                kind,
                observed,
                maximum,
            }))
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, amount: u64) -> Result<(), RustMetadataJoinError> {
        self.row_work = self.row_work.saturating_add(amount);
        *self.work = self.work.saturating_add(amount);
        self.reference_limit(
            NominalResolutionLimitKind::WorkPerReference,
            self.row_work,
            self.nominal.work_per_reference(),
        )?;
        if *self.work > self.aggregate.work_per_project() {
            return Err(self.error(RustMetadataJoinErrorKind::AggregateLimit {
                kind: NominalAggregationLimitKind::WorkPerProject,
                observed: *self.work,
                maximum: self.aggregate.work_per_project(),
            }));
        }
        Ok(())
    }

    fn name(&mut self, name: &str) -> Result<(), RustMetadataJoinError> {
        self.charge(1_u64.saturating_add(name.len() as u64))
    }

    fn field(
        &mut self,
        name: Option<&str>,
        field: &EnvironmentTypeProjectionNode,
    ) -> Result<(), RustMetadataJoinError> {
        match name {
            Some(name) => self.name(name)?,
            None => self.charge(1)?,
        }
        let mut frames = vec![(std::slice::from_ref(field).iter(), 1_u64)];
        let mut nodes = 0_u64;
        while let Some((siblings, depth)) = frames.last_mut() {
            let depth = *depth;
            let Some(node) = siblings.next() else {
                frames.pop();
                continue;
            };
            self.charge(1)?;
            nodes = nodes.saturating_add(1);
            self.reference_limit(
                NominalResolutionLimitKind::TypeNodesPerReference,
                nodes,
                self.nominal.type_nodes_per_reference(),
            )?;
            self.reference_limit(
                NominalResolutionLimitKind::RecursiveTypeDepth,
                depth,
                u64::from(self.nominal.recursive_type_depth()),
            )?;
            use EnvironmentTypeProjectionKind as Kind;
            match node.kind() {
                Kind::Vec(child) | Kind::Seq(child) | Kind::Option(child) | Kind::Need(child) => {
                    frames.push((std::slice::from_ref(child.as_ref()).iter(), depth + 1))
                }
                Kind::Result { ok, error } => {
                    frames.push((std::slice::from_ref(error.as_ref()).iter(), depth + 1));
                    frames.push((std::slice::from_ref(ok.as_ref()).iter(), depth + 1));
                }
                Kind::Tuple(items) => frames.push((items.iter(), depth + 1)),
                Kind::AcceptedNominal { arguments, .. } => {
                    self.reference_limit(
                        NominalResolutionLimitKind::GenericArgumentsPerApplication,
                        arguments.len() as u64,
                        u64::from(self.nominal.generic_arguments_per_application()),
                    )?;
                    frames.push((arguments.iter(), depth + 1));
                }
                Kind::Unit
                | Kind::Bool
                | Kind::I8
                | Kind::I16
                | Kind::I32
                | Kind::I64
                | Kind::I128
                | Kind::ISize
                | Kind::U8
                | Kind::U16
                | Kind::U32
                | Kind::U64
                | Kind::U128
                | Kind::USize
                | Kind::F32
                | Kind::F64
                | Kind::String
                | Kind::Char
                | Kind::Bytes
                | Kind::CharacterNominal(_)
                | Kind::TypeParameter { .. } => {}
            }
        }
        Ok(())
    }
}
