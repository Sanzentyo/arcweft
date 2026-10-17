//! The original nominal declaration tables, using the same type references,
//! policy grammar and shared resource bank as their owning type source frame.

use super::{
    Reader, RuntimeTypeSourceError as Error, RuntimeTypeSourceLimits, Writer, charge_source, limit,
};
use crate::entry::RuntimeNominalRecordShape;
use crate::entry::schema::{RuntimeCodecUse, RuntimeNominalCodecUses};
use crate::plan::{
    RuntimeNominalRecordDomain, RuntimeNominalRecordDomainField, RuntimeNominalRecordDomainTable,
    RuntimePlanInventory, RuntimePlanTypeProjection, RuntimePlanTypeTable, RuntimeVariantCase,
    RuntimeVariantDomain, RuntimeVariantDomainTable,
};
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::RuntimeRecordFieldId;

const DOMAIN: &[u8] = b"arcweft.runtime.nominal-sources.v1\0";

pub(in crate::plan) struct PreparedRuntimeNominalEncoding<'a> {
    inventory: &'a RuntimePlanInventory,
    limits: RuntimeTypeSourceLimits,
    encoded_bytes: usize,
    storage_bytes: usize,
    elements: usize,
}

impl PreparedRuntimeNominalEncoding<'_> {
    pub(in crate::plan) const fn encoded_bytes(&self) -> usize {
        self.encoded_bytes
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }

    pub(in crate::plan) fn append_to(self, output: &mut Vec<u8>) -> Result<(), Error> {
        let remaining = output.capacity() - output.len();
        if remaining < self.encoded_bytes {
            return Err(Error::OutputCapacity {
                required: self.encoded_bytes,
                remaining,
            });
        }
        let mut writer = Writer::new(
            Some(output),
            None,
            self.limits,
            self.inventory.type_table.len(),
        );
        writer.nominal_tables(self.inventory)?;
        debug_assert_eq!(writer.length, self.encoded_bytes);
        debug_assert_eq!(writer.counts.storage, self.storage_bytes);
        debug_assert_eq!(writer.counts.elements, self.elements);
        Ok(())
    }
}

/// All rows and policies have been preflighted, but no domain or proof exists.
pub(in crate::plan) struct PreparedRuntimeNominalSources<'a> {
    bytes: &'a [u8],
    limits: RuntimeTypeSourceLimits,
    type_count: usize,
    record_count: usize,
    variant_count: usize,
    storage_bytes: usize,
    elements: usize,
}

impl PreparedRuntimeNominalSources<'_> {
    pub(in crate::plan) const fn record_count(&self) -> usize {
        self.record_count
    }
    pub(in crate::plan) const fn variant_count(&self) -> usize {
        self.variant_count
    }
    pub(in crate::plan) const fn storage_bytes(&self) -> usize {
        self.storage_bytes
    }
    pub(in crate::plan) const fn elements(&self) -> usize {
        self.elements
    }

    pub(in crate::plan) fn materialize(
        self,
        types: &RuntimePlanTypeTable,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(RuntimeNominalRecordDomainTable, RuntimeVariantDomainTable), Error> {
        meter.status()?;
        let result = (|| {
            if types.len() != self.type_count {
                return Err(Error::TypeTableCount {
                    expected: self.type_count,
                    actual: types.len(),
                });
            }
            // Validate every original owner before any field/case is retained.
            let mut validate = Reader::new(self.bytes, self.limits, meter);
            validate.type_count = self.type_count;
            validate.nominal_tables(false, Some(types))?;
            let mut reader = Reader::new(self.bytes, self.limits, meter);
            reader.type_count = self.type_count;
            let tables = reader
                .nominal_tables(true, Some(types))?
                .tables
                .expect("retaining a complete preflighted nominal source frame");
            debug_assert_eq!(reader.counts.storage, self.storage_bytes);
            debug_assert_eq!(reader.counts.elements, self.elements);
            Ok(tables)
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl RuntimePlanInventory {
    pub(in crate::plan) fn prepare_nominal_sources_encoding(
        &self,
        limits: RuntimeTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeNominalEncoding<'_>, Error> {
        meter.status()?;
        let result = (|| {
            let mut writer = Writer::new(None, Some(&mut *meter), limits, self.type_table.len());
            writer.nominal_tables(self)?;
            Ok(PreparedRuntimeNominalEncoding {
                inventory: self,
                limits,
                encoded_bytes: writer.length,
                storage_bytes: writer.counts.storage,
                elements: writer.counts.elements,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }

    pub(in crate::plan) fn preflight_nominal_sources<'a>(
        bytes: &'a [u8],
        type_count: usize,
        limits: RuntimeTypeSourceLimits,
        meter: &mut TaskSemanticMeter,
    ) -> Result<PreparedRuntimeNominalSources<'a>, Error> {
        meter.status()?;
        let result = (|| {
            limit("encoded bytes", bytes.len(), limits.encoded_bytes)?;
            charge_source(meter, bytes.len())?;
            let mut reader = Reader::new(bytes, limits, meter);
            reader.type_count = type_count;
            let counts = reader.nominal_tables(false, None)?;
            Ok(PreparedRuntimeNominalSources {
                bytes,
                limits,
                type_count,
                record_count: counts.records,
                variant_count: counts.variants,
                storage_bytes: reader.counts.storage,
                elements: reader.counts.elements,
            })
        })();
        if result.is_err() {
            meter.reject_owner();
        }
        result
    }
}

impl super::Counts {
    fn nominal_rows<T>(&mut self, count: usize) -> Result<(), Error> {
        limit("nominal rows", count, self.limits.rows as usize)?;
        self.array::<(RuntimePlanTypeId, T)>(count)?;
        // Private row staging and final map entries can coexist. Nested payloads
        // move once; both logical outer buffers are included in the allowance.
        self.storage(
            count
                .checked_mul(size_of::<(RuntimePlanTypeId, T)>())
                .ok_or(Error::Arithmetic)?,
        )
    }
}

fn nominal_owner(
    types: &RuntimePlanTypeTable,
    owner: RuntimePlanTypeId,
) -> Result<
    (
        &crate::entry::RuntimeNominalTypeId,
        crate::entry::TypeLayoutHash,
    ),
    Error,
> {
    let row = types.get(owner).ok_or(Error::NominalOwner { owner })?;
    match (row.projection(), row.nominal_declaration()) {
        (
            RuntimePlanTypeProjection::Nominal {
                nominal, layout, ..
            },
            Some(_),
        ) => Ok((nominal, *layout)),
        _ => Err(Error::NominalOwner { owner }),
    }
}

impl Writer<'_> {
    fn nominal_policy(&mut self, value: Option<&RuntimeNominalCodecUses>) -> Result<(), Error> {
        self.tag(u8::from(value.is_some()))?;
        if let Some(value) = value {
            self.policy(&value.body, 0)?;
            self.counts
                .array::<RuntimeCodecUse>(value.arguments.len())?;
            self.count(value.arguments.len())?;
            for argument in &value.arguments {
                self.policy(argument, 0)?;
            }
        }
        Ok(())
    }

    fn nominal_tables(&mut self, inventory: &RuntimePlanInventory) -> Result<(), Error> {
        self.bytes(DOMAIN)?;
        self.tag(1)?;
        let records = inventory.nominal_record_domains();
        let variants = inventory.variant_domains();
        let rows = records
            .len()
            .checked_add(variants.len())
            .ok_or(Error::Arithmetic)?;
        limit("nominal rows", rows, self.counts.limits.rows as usize)?;
        self.counts
            .nominal_rows::<RuntimeNominalRecordDomain>(records.len())?;
        self.count(records.len())?;
        for row in records.domains() {
            nominal_owner(inventory.type_table(), row.owner())?;
            self.reference(row.owner())?;
            self.tag(row.shape().semantic_tag())?;
            self.counts
                .array::<RuntimeNominalRecordDomainField>(row.fields().len())?;
            self.count(row.fields().len())?;
            for field in row.fields() {
                self.count(field.field().zero_based() as usize)?;
                self.tag(u8::from(field.name().is_some()))?;
                if let Some(name) = field.name() {
                    self.string(name)?;
                }
                self.reference(field.ty())?;
            }
            self.nominal_policy(row.data_codec())?;
        }
        self.counts
            .nominal_rows::<RuntimeVariantDomain>(variants.len())?;
        self.count(variants.len())?;
        for row in variants.domains() {
            if records.get(row.owner()).is_some() {
                return Err(Error::NominalKind { owner: row.owner() });
            }
            let (nominal, layout) = nominal_owner(inventory.type_table(), row.owner())?;
            if row.nominal() != nominal || row.layout() != layout {
                return Err(Error::NominalOwner { owner: row.owner() });
            }
            self.reference(row.owner())?;
            self.string(row.nominal().as_str())?;
            self.bytes(row.layout().as_bytes())?;
            self.counts.array::<RuntimeVariantCase>(row.cases().len())?;
            self.count(row.cases().len())?;
            for (ordinal, case) in row.cases().iter().enumerate() {
                self.count(ordinal)?;
                self.string(case.name())?;
                self.tag(u8::from(case.payload().is_some()))?;
                if let Some(payload) = case.payload() {
                    self.reference(payload)?;
                }
            }
            self.nominal_policy(row.data_codec())?;
        }
        Ok(())
    }
}

struct DecodedNominalTables {
    records: usize,
    variants: usize,
    tables: Option<(RuntimeNominalRecordDomainTable, RuntimeVariantDomainTable)>,
}

impl Reader<'_, '_> {
    fn nominal_policy(
        &mut self,
        materialize: bool,
    ) -> Result<Option<RuntimeNominalCodecUses>, Error> {
        if !self.boolean()? {
            return Ok(None);
        }
        let body = self.policy(materialize, 0)?;
        let count = self.count()?;
        self.counts.array::<RuntimeCodecUse>(count)?;
        let mut arguments = materialize.then(|| Vec::with_capacity(count));
        for _ in 0..count {
            let value = self.policy(materialize, 0)?;
            if let (Some(arguments), Some(value)) = (&mut arguments, value) {
                arguments.push(value);
            }
        }
        Ok(body.map(|body| RuntimeNominalCodecUses {
            body,
            arguments: arguments
                .expect("retaining source policy arguments")
                .into_boxed_slice(),
        }))
    }

    fn domain_owner(
        &mut self,
        previous: &mut Option<RuntimePlanTypeId>,
        types: Option<&RuntimePlanTypeTable>,
    ) -> Result<RuntimePlanTypeId, Error> {
        let owner = self.reference()?;
        if previous.is_some_and(|previous| owner <= previous) {
            return Err(Error::NominalOrder);
        }
        *previous = Some(owner);
        if let Some(types) = types {
            nominal_owner(types, owner)?;
        }
        Ok(owner)
    }

    fn source_ordinal(&mut self, ordinal: usize) -> Result<(), Error> {
        let expected = u32::try_from(ordinal).map_err(|_| Error::Arithmetic)?;
        let actual = self.u32()?;
        if actual != expected {
            return Err(Error::Ordinal { expected, actual });
        }
        Ok(())
    }

    fn nominal_tables(
        &mut self,
        materialize: bool,
        types: Option<&RuntimePlanTypeTable>,
    ) -> Result<DecodedNominalTables, Error> {
        if self.take(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let version = self.tag()?;
        if version != 1 {
            return Err(Error::Version { actual: version });
        }
        let records = self.count()?;
        self.counts
            .nominal_rows::<RuntimeNominalRecordDomain>(records)?;
        let mut record_rows = materialize.then(|| Vec::with_capacity(records));
        let mut previous = None;
        for _ in 0..records {
            let owner = self.domain_owner(&mut previous, types)?;
            let tag = self.tag()?;
            let shape = RuntimeNominalRecordShape::from_tag(tag).ok_or(Error::Tag {
                kind: "nominal record shape",
                tag,
            })?;
            let count = self.count()?;
            self.counts
                .array::<RuntimeNominalRecordDomainField>(count)?;
            let mut fields = materialize.then(|| Vec::with_capacity(count));
            for ordinal in 0..count {
                self.source_ordinal(ordinal)?;
                let name = if self.boolean()? {
                    Some(self.string()?)
                } else {
                    None
                };
                let ty = self.reference()?;
                if let Some(fields) = &mut fields {
                    fields.push(RuntimeNominalRecordDomainField::from_source_parts(
                        RuntimeRecordFieldId::try_from_zero_based_ordinal(ordinal)
                            .expect("bounded source field ordinal"),
                        name.map(str::to_owned),
                        ty,
                    ));
                }
            }
            let codec = self.nominal_policy(materialize)?;
            if let (Some(rows), Some(fields)) = (&mut record_rows, fields) {
                rows.push(
                    RuntimeNominalRecordDomain::from_source_fields(
                        owner,
                        shape,
                        fields.into_boxed_slice(),
                    )
                    .with_data_codec(codec),
                );
            }
        }
        let variants = self.count()?;
        let rows = records.checked_add(variants).ok_or(Error::Arithmetic)?;
        limit("nominal rows", rows, self.counts.limits.rows as usize)?;
        self.counts.nominal_rows::<RuntimeVariantDomain>(variants)?;
        let mut variant_rows = materialize.then(|| Vec::with_capacity(variants));
        previous = None;
        for _ in 0..variants {
            let owner = self.domain_owner(&mut previous, types)?;
            let nominal = self.string()?;
            crate::entry::RuntimeNominalTypeId::validate_source(nominal)?;
            let layout = crate::entry::TypeLayoutHash::from_bytes(self.digest()?);
            if let Some(types) = types {
                let (expected, expected_layout) = nominal_owner(types, owner)?;
                if nominal != expected.as_str() || layout != expected_layout {
                    return Err(Error::NominalOwner { owner });
                }
            }
            let count = self.count()?;
            self.counts.array::<RuntimeVariantCase>(count)?;
            let mut cases = materialize.then(|| Vec::with_capacity(count));
            for ordinal in 0..count {
                self.source_ordinal(ordinal)?;
                let name = self.string()?;
                let payload = if self.boolean()? {
                    Some(self.reference()?)
                } else {
                    None
                };
                if let Some(cases) = &mut cases {
                    cases.push(RuntimeVariantCase::from_source_parts(
                        name.to_owned(),
                        payload,
                    ));
                }
            }
            let codec = self.nominal_policy(materialize)?;
            if let (Some(rows), Some(cases)) = (&mut variant_rows, cases) {
                rows.push(
                    RuntimeVariantDomain::from_source_cases(
                        owner,
                        crate::entry::RuntimeNominalTypeId::try_new(nominal)?,
                        layout,
                        cases.into_boxed_slice(),
                    )
                    .with_data_codec(codec),
                );
            }
        }
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing);
        }
        let tables = match (record_rows, variant_rows) {
            (Some(records), Some(variants)) => {
                let records = RuntimeNominalRecordDomainTable::from_source_domains(records)?;
                let variants = RuntimeVariantDomainTable::from_source_domains(variants)?;
                for row in variants.domains() {
                    if records.get(row.owner()).is_some() {
                        return Err(Error::NominalKind { owner: row.owner() });
                    }
                }
                Some((records, variants))
            }
            _ => None,
        };
        Ok(DecodedNominalTables {
            records,
            variants,
            tables,
        })
    }
}

#[cfg(test)]
pub(in crate::plan) mod tests;
