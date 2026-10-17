//! Original identity headers and effect-scope metadata of inert source values.

mod entity;

use super::{Reader, RuntimeLiteralSourceError as Error, Writer};
use crate::effect_row::EffectSet;
use crate::entry::{
    RuntimeCommandConstructorId, RuntimeCommandTargetId, RuntimeNominalTypeId, TypeLayoutHash,
};
use crate::pattern::{
    RuntimeBuiltinVariantIdentity, RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner,
    RuntimeOpaqueTypeProducerId, RuntimeSemanticTypeId, RuntimeVariantIdentity,
};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeRecordFieldId};
use arcweft_id::EffectId;

impl Writer<'_> {
    pub(super) fn nominal_id(&mut self, value: &RuntimeNominalTypeId) -> Result<(), Error> {
        RuntimeNominalTypeId::validate_source(value.as_str()).map_err(|source| {
            Error::Identity {
                offset: self.length,
                source,
            }
        })?;
        self.string(value.as_str())
    }

    pub(super) fn constructor(&mut self, value: &RuntimeCommandConstructorId) -> Result<(), Error> {
        RuntimeCommandConstructorId::validate_source(value.as_str()).map_err(|source| {
            Error::Identity {
                offset: self.length,
                source,
            }
        })?;
        self.string(value.as_str())
    }

    pub(super) fn target(&mut self, value: &RuntimeCommandTargetId) -> Result<(), Error> {
        RuntimeCommandTargetId::validate_source(value.as_str()).map_err(|source| {
            Error::Identity {
                offset: self.length,
                source,
            }
        })?;
        self.string(value.as_str())
    }

    pub(super) fn field(&mut self, field: RuntimeRecordFieldId) -> Result<(), Error> {
        self.u32(field.get().get())
    }

    pub(super) fn binding(
        &mut self,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
        depth: usize,
    ) -> Result<(), Error> {
        self.boolean(binding.is_some())?;
        let Some(binding) = binding else {
            return Ok(());
        };
        self.node(depth)?;
        self.bytes(binding.context().as_bytes())?;
        self.counts
            .array::<EffectSet>(binding.source_rows().len())?;
        self.count(binding.source_rows().len())?;
        for row in binding.source_rows() {
            self.counts.array::<EffectId>(row.len())?;
            self.count(row.len())?;
            for effect in row {
                self.string(effect.as_str())?;
            }
        }
        Ok(())
    }

    pub(super) fn variant_owner(&mut self, owner: &RuntimeVariantIdentity) -> Result<(), Error> {
        match owner {
            RuntimeVariantIdentity::Nominal {
                nominal,
                semantic_identity,
                layout,
            } => {
                self.tag(0)?;
                self.nominal_id(nominal)?;
                self.bytes(semantic_identity.as_bytes())?;
                self.bytes(layout.as_bytes())
            }
            RuntimeVariantIdentity::Builtin(owner) => {
                self.tag(1)?;
                self.tag(owner.semantic_tag())
            }
        }
    }

    pub(super) fn opaque_header(
        &mut self,
        producer: &RuntimeOpaqueTypeProducerId,
        semantic: RuntimeSemanticTypeId,
        class: RuntimeOpaqueValueClass,
        persistence: RuntimeOpaquePersistence,
    ) -> Result<(), Error> {
        self.nominal_id(producer.nominal())?;
        self.bytes(semantic.as_bytes())?;
        self.tag(class.semantic_tag())?;
        self.tag(persistence.semantic_tag())
    }

    pub(super) fn opaque_owner(&mut self, owner: &RuntimeOpaqueTypeOwner) -> Result<(), Error> {
        self.tag(match owner.admission() {
            RuntimeOpaqueTypeAdmission::ExactIdentity => 0,
            RuntimeOpaqueTypeAdmission::ProducerWide => 1,
        })?;
        self.opaque_header(
            owner.producer(),
            owner.semantic_identity(),
            owner.value_class(),
            owner.persistence(),
        )
    }
}

impl Reader<'_, '_> {
    pub(super) fn opaque_header(
        &mut self,
        materialize: bool,
    ) -> Result<
        (
            Option<RuntimeOpaqueTypeProducerId>,
            RuntimeSemanticTypeId,
            RuntimeOpaqueValueClass,
            RuntimeOpaquePersistence,
        ),
        Error,
    > {
        let producer = self
            .nominal_id(materialize)?
            .map(RuntimeOpaqueTypeProducerId::from_nominal);
        let semantic = RuntimeSemanticTypeId::from_bytes(self.digest()?);
        let offset = self.offset;
        let tag = self.tag()?;
        let class = RuntimeOpaqueValueClass::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "opaque value class",
            tag,
            offset,
        })?;
        let offset = self.offset;
        let tag = self.tag()?;
        let persistence = RuntimeOpaquePersistence::from_semantic_tag(tag).ok_or(Error::Tag {
            kind: "opaque persistence",
            tag,
            offset,
        })?;
        Ok((producer, semantic, class, persistence))
    }

    pub(super) fn opaque_owner(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeOpaqueTypeOwner>, Error> {
        let offset = self.offset;
        let admission = match self.tag()? {
            0 => RuntimeOpaqueTypeAdmission::ExactIdentity,
            1 => RuntimeOpaqueTypeAdmission::ProducerWide,
            tag => {
                return Err(Error::Tag {
                    kind: "opaque owner admission",
                    tag,
                    offset,
                });
            }
        };
        let (producer, semantic, class, persistence) = self.opaque_header(materialize)?;
        Ok(producer.map(|producer| {
            RuntimeOpaqueTypeOwner::with_admission(
                producer,
                semantic,
                admission,
                class,
                persistence,
            )
        }))
    }

    pub(super) fn field(&mut self) -> Result<RuntimeRecordFieldId, Error> {
        let offset = self.offset;
        let actual = self.u32()?;
        let ordinal = actual
            .checked_sub(1)
            .ok_or(Error::Field { offset, actual })?;
        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal as usize)
            .map_err(|_| Error::Field { offset, actual })
    }

    pub(super) fn nominal_id(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeNominalTypeId>, Error> {
        let offset = self.offset;
        let text = self.string()?;
        RuntimeNominalTypeId::validate_source(text)
            .map_err(|source| Error::Identity { offset, source })?;
        if materialize {
            RuntimeNominalTypeId::try_new(text)
                .map(Some)
                .map_err(|source| Error::Identity { offset, source })
        } else {
            Ok(None)
        }
    }

    pub(super) fn constructor(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeCommandConstructorId>, Error> {
        let offset = self.offset;
        let text = self.string()?;
        RuntimeCommandConstructorId::validate_source(text)
            .map_err(|source| Error::Identity { offset, source })?;
        if materialize {
            RuntimeCommandConstructorId::try_new(text)
                .map(Some)
                .map_err(|source| Error::Identity { offset, source })
        } else {
            Ok(None)
        }
    }

    pub(super) fn target(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeCommandTargetId>, Error> {
        let offset = self.offset;
        let text = self.string()?;
        RuntimeCommandTargetId::validate_source(text)
            .map_err(|source| Error::Identity { offset, source })?;
        if materialize {
            RuntimeCommandTargetId::try_new(text)
                .map(Some)
                .map_err(|source| Error::Identity { offset, source })
        } else {
            Ok(None)
        }
    }

    pub(super) fn binding(
        &mut self,
        materialize: bool,
        depth: usize,
    ) -> Result<Option<RuntimeFunctionEffectInstantiation>, Error> {
        if !self.boolean()? {
            return Ok(None);
        }
        self.node(depth)?;
        let context = RuntimeSemanticTypeId::from_bytes(self.digest()?);
        let count = self.count()?;
        self.counts.array::<EffectSet>(count)?;
        let mut rows = materialize.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let count = self.count()?;
            self.counts.array::<EffectId>(count)?;
            let mut row = materialize.then(EffectSet::new);
            let mut previous = None;
            for _ in 0..count {
                let offset = self.offset;
                let label = self.string()?;
                EffectId::validate_source(label)
                    .map_err(|source| Error::Effect { offset, source })?;
                if previous.is_some_and(|previous: &str| previous >= label) {
                    return Err(Error::Invalid {
                        kind: "effect set canonical order",
                        offset,
                    });
                }
                previous = Some(label);
                if let Some(row) = &mut row {
                    row.insert(
                        EffectId::parse(label)
                            .map_err(|source| Error::Effect { offset, source })?,
                    );
                }
            }
            if let Some(rows) = &mut rows {
                rows.push(row.expect("retained effect row"));
            }
        }
        Ok(rows.map(|rows| {
            RuntimeFunctionEffectInstantiation::from_source_rows(context, rows.into_boxed_slice())
        }))
    }

    pub(super) fn variant_owner(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeVariantIdentity>, Error> {
        let offset = self.offset;
        match self.tag()? {
            0 => {
                let nominal = self.nominal_id(materialize)?;
                let semantic_identity = RuntimeSemanticTypeId::from_bytes(self.digest()?);
                let layout = TypeLayoutHash::from_bytes(self.digest()?);
                Ok(nominal.map(|nominal| RuntimeVariantIdentity::Nominal {
                    nominal,
                    semantic_identity,
                    layout,
                }))
            }
            1 => {
                let offset = self.offset;
                let tag = self.tag()?;
                let owner =
                    RuntimeBuiltinVariantIdentity::from_wire_tag(tag).ok_or(Error::Tag {
                        kind: "builtin variant owner",
                        tag,
                        offset,
                    })?;
                Ok(materialize.then_some(RuntimeVariantIdentity::Builtin(owner)))
            }
            tag => Err(Error::Tag {
                kind: "variant owner",
                tag,
                offset,
            }),
        }
    }
}
