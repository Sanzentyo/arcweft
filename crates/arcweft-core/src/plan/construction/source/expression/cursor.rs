//! Borrowed row metadata views. Factory metadata is admitted before its scratch
//! index or owned keys. Nested literal/pattern owners retain responsibility for
//! their own complete framing and resource counts.

use super::{Array, Counts, DOMAIN, Error, RuntimeExpressionSourceLimits, limit, reference};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::pattern::RuntimePattern;
use crate::runtime_id::RuntimePlanTypeId;
use crate::task::semantic::TaskSemanticMeter;
use crate::value::{
    AwbcRuntimeValueSnapshot, RuntimeAgentConstructor, RuntimeAgentField,
    RuntimeExternalCallOperand,
};

pub(super) struct Row<'a> {
    pub tag: u8,
    pub ty: RuntimePlanTypeId,
    pub guards: &'a [u8],
    pub payload: &'a [u8],
    pub children: usize,
}
pub(super) struct Cursor<'a> {
    pub bytes: &'a [u8],
    pub offset: usize,
    pub limits: RuntimeExpressionSourceLimits,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_interaction_model::presentation::{
        PresentationArgumentRole, PresentationCallableId,
    };

    #[test]
    fn external_source_preflight_rejects_overlapping_borrowed_operand_ranges() {
        fn text(bytes: &mut Vec<u8>, value: &str) {
            bytes.extend(u32::try_from(value.len()).unwrap().to_le_bytes());
            bytes.extend(value.as_bytes());
        }
        let plan = crate::plan::RuntimePlanBuilder::new().finish().unwrap();
        let limits = super::super::tests::limits(&plan);
        for overlap in [false, true] {
            let mut bytes = vec![2];
            text(
                &mut bytes,
                crate::entry::RuntimeCallableId::from_checked_digest([0xa7; 32]).as_str(),
            );
            bytes.extend([
                0,
                crate::value::StandardEnvironmentId::Core.canonical_tag(),
                0,
            ]);
            bytes.extend(1_u32.to_le_bytes());
            text(&mut bytes, "fixture");
            bytes.extend(0_u16.to_le_bytes());
            bytes.push(crate::value::RuntimeExternalCallOperandOrder::Authored.semantic_tag());
            bytes.extend(2_u32.to_le_bytes());
            for position in [0_u32, u32::from(!overlap)] {
                bytes.extend([
                    crate::value::RuntimeExternalCallOperandProjection::Value.semantic_tag(),
                    0,
                ]);
                bytes.extend(position.to_le_bytes());
                bytes.extend(1_u32.to_le_bytes());
            }
            let mut cursor = Cursor::new(&bytes, limits);
            let mut counts = Counts::new(limits, 1).unwrap();
            let result = cursor.call_target(&mut counts);
            if overlap {
                assert!(matches!(
                    result,
                    Err(Error::Invalid {
                        kind: "external operand layout",
                        ..
                    })
                ));
            } else {
                result.unwrap();
                cursor.finish().unwrap();
            }
        }
    }

    #[test]
    fn factory_source_preflight_rejects_duplicate_borrowed_destinations() {
        let plan = crate::plan::RuntimePlanBuilder::new().finish().unwrap();
        let limits = super::super::tests::limits(&plan);
        for duplicate in [false, true] {
            let mut bytes = vec![4, PresentationCallableId::Image.semantic_tag()];
            bytes.extend([1; 32]);
            bytes.extend([2; 32]);
            bytes.extend(2_u32.to_le_bytes());
            for key in ["channel", if duplicate { "channel" } else { "user.role" }] {
                bytes.push(PresentationArgumentRole::Parameter.semantic_tag());
                bytes.extend((key.len() as u32).to_le_bytes());
                bytes.extend(key.as_bytes());
            }
            let mut cursor = Cursor::new(&bytes, limits);
            let mut counts = Counts::new(limits, 1).unwrap();
            let result = cursor.call_target(&mut counts);
            if duplicate {
                assert!(matches!(
                    result,
                    Err(Error::Invalid {
                        kind: "presentation factory ABI",
                        ..
                    })
                ));
            } else {
                result.unwrap();
                cursor.finish().unwrap();
            }
        }
    }
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8], limits: RuntimeExpressionSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    pub fn invalid(&self, kind: &'static str) -> Error {
        Error::Invalid {
            kind,
            offset: self.offset,
        }
    }
    pub fn slice(&mut self, n: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(n).ok_or(Error::Arithmetic)?;
        let value = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(value)
    }
    pub fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.slice(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.slice(4)?.try_into().expect("source u32 width"),
        ))
    }
    pub fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(
            self.slice(2)?.try_into().expect("source u16 width"),
        ))
    }
    pub fn external_operand(&mut self) -> Result<RuntimeExternalCallOperand, Box<Error>> {
        let tag = self.byte()?;
        let projection = crate::value::RuntimeExternalCallOperandProjection::from_semantic_tag(tag)
            .ok_or_else(|| Box::new(self.invalid("external operand projection")))?;
        let parameter = match self.byte()? {
            0 => None,
            1 => Some(crate::plan::RuntimeCallableParameterCoordinate {
                group: self.u32()?,
                parameter: self.u32()?,
            }),
            _ => return Err(Box::new(self.invalid("external operand coordinate"))),
        };
        Ok(RuntimeExternalCallOperand {
            projection,
            parameter,
            abi_position: self.u32()?,
            expansion_width: self.u32()?,
        })
    }
    pub fn boolean(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(self.invalid("flag")),
        }
    }
    pub fn frame(&mut self) -> Result<&'a [u8], Error> {
        let n = self.u32()? as usize;
        self.slice(n)
    }
    pub fn string(&mut self) -> Result<&'a str, Error> {
        let value = self.frame()?;
        std::str::from_utf8(value).map_err(|_| self.invalid("UTF-8"))
    }
    fn text(&mut self, counts: &mut Counts) -> Result<(), Error> {
        let n = self.string()?.len();
        counts.storage(n.checked_mul(2).ok_or(Error::Arithmetic)?)
    }
    pub fn id(&mut self, kind: &'static str, maximum: usize) -> Result<u32, Error> {
        let id = self.u32()?;
        reference(kind, id, maximum)?;
        Ok(id)
    }
    pub fn ty(&mut self) -> Result<RuntimePlanTypeId, Error> {
        let raw = self.id("type", self.limits.type_count)?;
        Ok(RuntimePlanTypeId::from_accepted_ordinal(
            std::num::NonZeroU32::new(raw).expect("checked type"),
        ))
    }
    pub fn count(&mut self) -> Result<usize, Error> {
        let n = self.u32()? as usize;
        limit("elements", n, self.limits.elements as usize)?;
        Ok(n)
    }
    fn array(&mut self, counts: &mut Counts, array: Array) -> Result<usize, Error> {
        let n = self.count()?;
        counts.array(n, array.width())?;
        Ok(n)
    }
    pub fn fields(&mut self) -> Result<&'a [u8], Error> {
        let n = self.count()?;
        let bytes = self.slice(n.checked_mul(4).ok_or(Error::Arithmetic)?)?;
        for bytes in bytes.chunks_exact(4) {
            reference(
                "field",
                u32::from_le_bytes(bytes.try_into().expect("field width")),
                u32::MAX as usize,
            )?;
        }
        Ok(bytes)
    }
    fn field_array(&mut self, counts: &mut Counts) -> Result<(), Error> {
        let fields = self.fields()?;
        counts.array(fields.len() / 4, Array::Fields.width())
    }
    fn value_target(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        match self.byte()? {
            0 => Ok(1),
            1 | 2 => {
                self.place(counts)?;
                Ok(0)
            }
            _ => Err(self.invalid("value target")),
        }
    }

    fn place(&mut self, counts: &mut Counts) -> Result<(), Error> {
        self.id("local", self.limits.local_count)?;
        self.field_array(counts)
    }
    fn arguments(&mut self, counts: &mut Counts) -> Result<usize, Error> {
        let n = self.array(counts, Array::Arguments)?;
        for _ in 0..n {
            self.boolean()?;
            self.u32()?;
        }
        Ok(n)
    }
    fn optional_ref(&mut self, kind: &'static str, maximum: usize) -> Result<(), Error> {
        if self.boolean()? {
            self.id(kind, maximum)?;
        }
        Ok(())
    }
    fn literal(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let literal = AwbcRuntimeValueSnapshot::preflight_literal_source(
            self.frame()?,
            counts.literal_limits(),
            resources,
            meter,
        )?;
        counts.nested(
            literal
                .storage_bytes()
                .checked_mul(2)
                .ok_or(Error::Arithmetic)?,
            literal.elements(),
        )
    }
    fn pattern(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<(), Error> {
        let pattern = RuntimePattern::preflight_sources(
            self.frame()?,
            counts.pattern_limits(),
            resources,
            meter,
        )?;
        counts.nested(pattern.storage_bytes(), pattern.elements())
    }
    pub fn header(&mut self) -> Result<usize, Error> {
        if self.slice(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        let n = self.u32()? as usize;
        limit("nodes", n, self.limits.nodes as usize)?;
        if n == 0 {
            return Err(Error::Root);
        }
        Ok(n)
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
    #[expect(
        clippy::too_many_lines,
        reason = "one exhaustive source row grammar, with borrowed metadata and owning nested preflights"
    )]
    pub fn row(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Row<'a>, Error> {
        let tag = self.byte()?;
        let ty = self.ty()?;
        let guards_count = self.array(counts, Array::Guards)?;
        let start = self.offset;
        for _ in 0..guards_count {
            self.id("local", self.limits.local_count)?;
        }
        let guards = &self.bytes[start..self.offset];
        let start = self.offset;
        let children = match tag {
            0 | 6 => {
                self.literal(counts, resources, meter)?;
                0
            }
            1 => {
                let constructor = RuntimeAgentConstructor::from_semantic_tag(self.byte()?)
                    .ok_or_else(|| self.invalid("Agent constructor"))?;
                if constructor == RuntimeAgentConstructor::ChoiceAction {
                    self.text(counts)?;
                }
                let n = self.array(counts, Array::Children)?;
                if !constructor.accepts_operand_count(
                    n + usize::from(constructor == RuntimeAgentConstructor::ChoiceAction),
                ) {
                    return Err(self.invalid("Agent operand count"));
                }
                n
            }
            2 => {
                self.id("local", self.limits.local_count)?;
                self.boolean()?;
                self.field_array(counts)?;
                0
            }
            38 => {
                use arcweft_id::LifetimeScopeKind;

                self.boolean()?;
                let scope_start = self.offset;
                let scope = self.string()?;
                if scope != LifetimeScopeKind::Flow.as_str()
                    && scope != LifetimeScopeKind::Line.as_str()
                {
                    return Err(Error::Invalid {
                        kind: "registry read scope",
                        offset: scope_start,
                    });
                }
                counts.storage(scope.len().checked_mul(2).ok_or(Error::Arithmetic)?)?;
                let count = self.array(counts, Array::RegistryKey)?;
                let start = self.offset;
                for _ in 0..count {
                    self.text(counts)?;
                }
                let mut segments = Self::new(&self.bytes[start..self.offset], self.limits);
                for _ in 0..count {
                    arcweft_id::DeclarationName::validate_source(segments.string()?)?;
                }
                0
            }
            3 | 5 => {
                self.place(counts)?;
                0
            }
            4 => {
                self.place(counts)?;
                1
            }
            7 => {
                self.id("local", self.limits.local_count)?;
                2
            }
            8 => {
                if self.boolean()? {
                    self.text(counts)?;
                }
                1
            }
            9 | 13 => self.array(counts, Array::Children)?,
            10 => {
                self.id("template", self.limits.template_count)?;
                let mut n = self.array(counts, Array::Children)?;
                let effects = self.array(counts, Array::Effects)?;
                for _ in 0..effects {
                    self.id("effect site", u32::MAX as usize)?;
                    self.id("state", self.limits.state_count)?;
                    n = n
                        .checked_add(self.array(counts, Array::Children)?)
                        .ok_or(Error::Arithmetic)?;
                }
                n
            }
            11 => {
                self.id("template", self.limits.template_count)?;
                self.optional_ref("attempt", self.limits.attempt_count)?;
                self.optional_ref("method", self.limits.method_count)?;
                self.boolean()?;
                let n = self.array(counts, Array::Format)?;
                for _ in 0..n {
                    self.slice(32)?;
                    if self.byte()? > 8 {
                        return Err(self.invalid("fmt parameter"));
                    }
                }
                n
            }
            12 => {
                self.boolean()?;
                let fields = self.array(counts, Array::Dialogue)?;
                let mut n = 1usize;
                for _ in 0..fields {
                    match self.byte()? {
                        0..=11 => {}
                        12 => self.text(counts)?,
                        _ => return Err(self.invalid("dialogue field")),
                    }
                    if self.boolean()? {
                        n = n.checked_add(1).ok_or(Error::Arithmetic)?;
                    }
                }
                n
            }
            14 => {
                self.u32()?;
                1
            }
            15 => {
                let start = self.boolean()?;
                let end = self.boolean()?;
                self.boolean()?;
                usize::from(start) + usize::from(end)
            }
            16 => {
                let n = self.array(counts, Array::Record)?;
                for _ in 0..n {
                    self.id("field", u32::MAX as usize)?;
                }
                n
            }
            17 => {
                self.u32()?;
                usize::from(self.boolean()?)
            }
            37 => {
                if arcweft_interaction_model::presentation::PresentationHandleMethodId::from_semantic_tag(self.byte()?).is_none() {
                    return Err(self.invalid("presentation handle method"));
                }
                self.value_target(counts)?
            }
            18 => {
                let children = self.value_target(counts)?;
                match self.byte()? {
                    0 => {
                        self.ty()?;
                        self.id("field", u32::MAX as usize)?;
                    }
                    1 => {
                        self.ty()?;
                        self.text(counts)?;
                        self.id("field", u32::MAX as usize)?;
                    }
                    2 => {
                        let tag = self.u32()?;
                        if !RuntimeAgentField::ALL
                            .iter()
                            .any(|field| u32::from(field.semantic_tag()) == tag)
                        {
                            return Err(self.invalid("Agent field"));
                        }
                    }
                    3 => {
                        if self.byte()? > 2 {
                            return Err(self.invalid("entity field"));
                        }
                    }
                    4 => {
                        if self.byte()? > 1 {
                            return Err(self.invalid("progress field"));
                        }
                    }
                    _ => return Err(self.invalid("field projection")),
                }
                children
            }
            19 | 20 => {
                self.u32()?;
                1
            }
            21 => {
                self.place(counts)?;
                if self.boolean()? {
                    self.initialization()?;
                    let n = self.array(counts, Array::Displacement)?;
                    for _ in 0..n {
                        self.field_array(counts)?;
                        self.initialization()?;
                    }
                }
                2
            }
            22 => {
                self.call_target(counts)?;
                self.arguments(counts)?
            }
            23 => {
                self.id("state", self.limits.state_count)?;
                self.array(counts, Array::Children)?
            }
            24 => {
                self.id("specialization", self.limits.specialization_count)?;
                1
            }
            25 => self
                .arguments(counts)?
                .checked_add(1)
                .ok_or(Error::Arithmetic)?,
            26 => {
                self.id("method", self.limits.method_count)?;
                if self.byte()? > 2 {
                    return Err(self.invalid("receiver mode"));
                }
                self.arguments(counts)?
                    .checked_add(1)
                    .ok_or(Error::Arithmetic)?
            }
            27 => {
                self.id("helper", self.limits.helper_count)?;
                self.arguments(counts)?
            }
            29 | 35 | 36 => 1,
            30 => {
                if self.byte()? > 1 {
                    return Err(self.invalid("unary operator"));
                }
                1
            }
            31 => {
                if self.byte()? > 11 {
                    return Err(self.invalid("binary operator"));
                }
                2
            }
            32 => 3,
            33 => {
                self.pattern(counts, resources, meter)?;
                3 + usize::from(self.boolean()?)
            }
            34 => {
                let arms = self.array(counts, Array::Arms)?;
                let mut n = 1usize;
                for _ in 0..arms {
                    self.pattern(counts, resources, meter)?;
                    n = n
                        .checked_add(1 + usize::from(self.boolean()?))
                        .ok_or(Error::Arithmetic)?;
                }
                n
            }
            _ => return Err(self.invalid("expression tag")),
        };
        Ok(Row {
            tag,
            ty,
            guards,
            payload: &self.bytes[start..self.offset],
            children,
        })
    }
    pub fn initialization(&mut self) -> Result<u8, Error> {
        let tag = self.byte()?;
        if tag > 2 {
            return Err(self.invalid("place initialization"));
        }
        Ok(tag)
    }
    fn call_target(&mut self, counts: &mut Counts) -> Result<(), Error> {
        match self.byte()? {
            0 => {
                if crate::value::RuntimeIntrinsic::from_source_tag(self.byte()?).is_none() {
                    return Err(self.invalid("intrinsic"));
                }
            }
            1 => self.text(counts)?,
            5 => {
                let view = self.string()?;
                counts.storage(view.len().checked_mul(2).ok_or(Error::Arithmetic)?)?;
                let definition =
                    crate::presentation::RuntimePresentationViewIdentity::from_checked_declaration(
                        self.slice(32)?
                            .try_into()
                            .expect("definition identity width"),
                    )
                    .map_err(|_| self.invalid("View definition"))?;
                let parameters = self.u32()?;
                let resource = crate::pattern::RuntimeSemanticTypeId::from_bytes(
                    self.slice(32)?.try_into().expect("semantic identity width"),
                );
                let body = crate::pattern::RuntimeSemanticTypeId::from_bytes(
                    self.slice(32)?.try_into().expect("semantic identity width"),
                );
                let count = self.count()?;
                counts.array(count, size_of::<u32>())?;
                let bytes = self.slice(count.checked_mul(4).ok_or(Error::Arithmetic)?)?;
                let supplied = bytes
                    .chunks_exact(4)
                    .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("parameter width")));
                crate::presentation::RuntimePresentationViewCall::preflight(
                    view,
                    definition,
                    parameters,
                    count,
                    supplied,
                    resource,
                    body,
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .map_err(|_| self.invalid("presentation View ABI"))?;
            }
            4 => {
                use arcweft_interaction_model::presentation::{
                    PresentationArgumentKey, PresentationArgumentKeyRef, PresentationArgumentRole,
                    PresentationCallableId, PresentationCallableResult,
                };
                let callable = PresentationCallableId::from_semantic_tag(self.byte()?)
                    .ok_or_else(|| self.invalid("presentation factory"))?;
                if !matches!(callable.result(), PresentationCallableResult::Handle(_)) {
                    return Err(self.invalid("presentation factory result"));
                }
                let resource_type = crate::pattern::RuntimeSemanticTypeId::from_bytes(
                    self.slice(32)?.try_into().expect("semantic identity width"),
                );
                let body_type = crate::pattern::RuntimeSemanticTypeId::from_bytes(
                    self.slice(32)?.try_into().expect("semantic identity width"),
                );
                let n = self.count()?;
                counts.array(n, std::mem::size_of::<PresentationArgumentKey>())?;
                let start = self.offset;
                let mut fixed = 0_u64;
                for _ in 0..n {
                    let role = PresentationArgumentRole::from_semantic_tag(self.byte()?)
                        .ok_or_else(|| self.invalid("presentation argument role"))?;
                    if !callable.accepts_argument_role(role) {
                        return Err(self.invalid("presentation argument destination"));
                    }
                    if role.requires_parameter() {
                        let text = self.string()?;
                        counts.storage(text.len().checked_mul(2).ok_or(Error::Arithmetic)?)?;
                        PresentationArgumentKey::validate_parameter(text)
                            .map_err(|_| self.invalid("presentation metadata key"))?;
                    } else {
                        let bit = 1_u64 << role.semantic_tag();
                        if fixed & bit != 0 {
                            return Err(self.invalid("duplicate presentation argument"));
                        }
                        fixed |= bit;
                    }
                }
                let bytes = self.bytes;
                let limits = self.limits;
                let inputs = (0..n).scan(start, move |offset, _| {
                    let mut cursor = Cursor::new(bytes, limits);
                    cursor.offset = *offset;
                    let role = PresentationArgumentRole::from_semantic_tag(
                        cursor.byte().expect("preflighted role"),
                    )
                    .expect("preflighted role");
                    let parameter = if role.requires_parameter() {
                        Some(cursor.string().expect("preflighted key text"))
                    } else {
                        None
                    };
                    *offset = cursor.offset;
                    Some(
                        PresentationArgumentKeyRef::try_new(role, parameter)
                            .expect("preflighted destination"),
                    )
                });
                crate::presentation::RuntimePresentationFactoryCall::preflight_inputs(
                    callable,
                    inputs,
                    resource_type,
                    body_type,
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .map_err(|_| self.invalid("presentation factory ABI"))?;
            }
            3 => {
                let operation =
                    crate::presentation::RuntimePresentationSlotOperation::from_semantic_tag(
                        self.byte()?,
                    )
                    .ok_or_else(|| self.invalid("slot operation"))?;
                let resource = arcweft_interaction_model::presentation::PresentationResourceKind::from_semantic_tag(self.byte()?)
                    .ok_or_else(|| self.invalid("slot reference resource"))?;
                let count = self.count()?;
                if count > 4 {
                    return Err(self.invalid("slot reference input count"));
                }
                counts.array(count, 0)?;
                let mut roles = [crate::presentation::RuntimePresentationSlotInput::Target; 4];
                for role in &mut roles[..count] {
                    *role = crate::presentation::RuntimePresentationSlotInput::from_semantic_tag(
                        self.byte()?,
                    )
                    .ok_or_else(|| self.invalid("slot reference input role"))?;
                }
                crate::presentation::RuntimePresentationSlotCall::try_new(
                    operation,
                    resource,
                    roles[..count].iter().copied(),
                )
                .map_err(|_| self.invalid("slot reference input ABI"))?;
            }
            2 => {
                self.text(counts)?;
                match self.byte()? {
                    0 => {
                        if crate::value::StandardEnvironmentId::from_canonical_tag(self.byte()?)
                            .is_none()
                        {
                            return Err(self.invalid("standard environment"));
                        }
                    }
                    1 => self.text(counts)?,
                    _ => return Err(self.invalid("external provider")),
                }
                match self.byte()? {
                    0 | 2 => {
                        let n = self.count()?;
                        counts.array(n, 2 * size_of::<String>())?;
                        for _ in 0..n {
                            self.text(counts)?;
                        }
                    }
                    1 => {
                        self.slice(32)?;
                        self.text(counts)?;
                    }
                    _ => return Err(self.invalid("external export")),
                }
                self.u16()?;
                let order =
                    crate::value::RuntimeExternalCallOperandOrder::from_semantic_tag(self.byte()?)
                        .ok_or_else(|| self.invalid("external operand order"))?;
                let n = self.count()?;
                counts.array(
                    n,
                    size_of::<RuntimeExternalCallOperand>()
                        + RuntimeExternalCallOperand::validation_scratch_element_bytes(),
                )?;
                let start = self.offset;
                for position in 0..n {
                    let row = self.external_operand().map_err(|error| *error)?;
                    if order == crate::value::RuntimeExternalCallOperandOrder::InvocationInputs
                        && (row.parameter.is_none()
                            || row.expansion_width != 1
                            || row.abi_position as usize != position)
                    {
                        return Err(self.invalid("external invocation-input layout"));
                    }
                }
                let mut rows = Cursor::new(self.bytes, self.limits);
                rows.offset = start;
                RuntimeExternalCallOperand::preflight_layout((0..n).map(|_| {
                    rows.external_operand()
                        .expect("complete operand table was scanned")
                }))
                .map_err(|_| self.invalid("external operand layout"))?;
            }
            _ => return Err(self.invalid("call target")),
        }
        Ok(())
    }
}
