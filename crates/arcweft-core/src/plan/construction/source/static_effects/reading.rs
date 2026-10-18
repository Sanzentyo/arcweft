//! One byte reader for inspection and retention of the actual request enum.

use super::{Context, Counts, DOMAIN, Error};
use crate::effect::{
    LineEffectRequest as Effect, RuntimeAssertion, RuntimeAssertionGuardId,
    RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall, RuntimeEvent, RuntimeField,
    RuntimeLog, RuntimeWaitTarget,
};
use crate::line_task::LineOutRequest;
use crate::task::semantic::TaskSemanticMeter;
use crate::time::LogicalDuration;

pub(super) struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
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
    fn count(&mut self) -> Result<usize, Error> {
        Ok(u32::from_le_bytes(self.raw(4)?.try_into().expect("source count width")) as usize)
    }
    fn frame(&mut self) -> Result<&'a [u8], Error> {
        let n = self.count()?;
        self.raw(n)
    }
    fn text(&mut self, counts: &mut Counts) -> Result<&'a str, Error> {
        let text = std::str::from_utf8(self.frame()?)?;
        counts.storage(text.len())?;
        Ok(text)
    }
    fn optional_text(&mut self, counts: &mut Counts) -> Result<Option<&'a str>, Error> {
        match self.byte()? {
            0 => Ok(None),
            1 => self.text(counts).map(Some),
            _ => Err(self.invalid("optional text flag")),
        }
    }
    fn fields(
        &mut self,
        counts: &mut Counts,
        retain: bool,
    ) -> Result<Option<Vec<RuntimeField>>, Error> {
        let n = self.count()?;
        counts.array::<RuntimeField>(n)?;
        let mut fields = retain.then(|| Vec::with_capacity(n));
        for _ in 0..n {
            let name = self.text(counts)?;
            let value = self.text(counts)?;
            if let Some(fields) = &mut fields {
                fields.push(RuntimeField {
                    name: name.into(),
                    value: value.into(),
                });
            }
        }
        Ok(fields)
    }
    fn strings(&mut self, counts: &mut Counts, retain: bool) -> Result<Option<Vec<String>>, Error> {
        let n = self.count()?;
        counts.array::<String>(n)?;
        let mut strings = retain.then(|| Vec::with_capacity(n));
        for _ in 0..n {
            let text = self.text(counts)?;
            if let Some(strings) = &mut strings {
                strings.push(text.into());
            }
        }
        Ok(strings)
    }
    pub fn request(
        &mut self,
        counts: &mut Counts,
        context: &mut Context<'_, '_>,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Option<Effect>, Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(self.invalid("domain"));
        }
        if self.byte()? != 1 {
            return Err(self.invalid("version"));
        }
        let retain = context.retains();
        let tag = self.byte()?;
        let result = match tag {
            0 => {
                let nanos =
                    u64::from_le_bytes(self.raw(8)?.try_into().expect("source duration width"));
                retain.then(|| {
                    Effect::Wait(RuntimeWaitTarget::Duration(LogicalDuration::from_nanos(
                        nanos,
                    )))
                })
            }
            1 => return Err(self.invalid("raw Audio expression carrier")),
            2 => {
                let callee = self.text(counts)?;
                let args = self.strings(counts, retain)?;
                retain.then(|| {
                    Effect::Call(RuntimeCall {
                        callee: callee.into(),
                        args: args.expect("retaining call arguments"),
                    })
                })
            }
            3 => {
                let level = self.text(counts)?;
                let message = self.text(counts)?;
                let fields = self.fields(counts, retain)?;
                retain.then(|| {
                    Effect::Log(RuntimeLog {
                        level: level.into(),
                        message: message.into(),
                        fields: fields.expect("retaining log fields"),
                    })
                })
            }
            4 | 5 => {
                let target = self.text(counts)?;
                let value = context.value(self.frame()?, counts, meter)?;
                if let Some(value) = value {
                    let assignment = RuntimeAssignment::try_new(target.into(), value)?;
                    Some(if tag == 4 {
                        Effect::SignalWrite(assignment)
                    } else {
                        Effect::MetricWrite(assignment)
                    })
                } else {
                    None
                }
            }
            6 => {
                let event = self.text(counts)?;
                let fields = self.fields(counts, retain)?;
                retain.then(|| {
                    Effect::EmitEvent(RuntimeEvent {
                        event: event.into(),
                        fields: fields.expect("retaining event fields"),
                    })
                })
            }
            7 => {
                let label = self.optional_text(counts)?;
                let value = self.text(counts)?;
                retain.then(|| {
                    Effect::Out(LineOutRequest {
                        label: label.map(Into::into),
                        value: value.into(),
                    })
                })
            }
            8..=12 | 15 | 16 => {
                let text = self.text(counts)?;
                retain.then(|| match tag {
                    8 => Effect::Return(text.into()),
                    9 => Effect::Goto(text.into()),
                    10 => Effect::Panic(text.into()),
                    11 => Effect::Fail(text.into()),
                    12 => Effect::Bail(text.into()),
                    15 => Effect::Close(text.into()),
                    16 => Effect::Select(text.into()),
                    _ => unreachable!("closed single-text source tags"),
                })
            }
            13 => {
                let condition = self.text(counts)?;
                let message = self.text(counts)?;
                retain.then(|| Effect::Ensure {
                    condition: condition.into(),
                    message: message.into(),
                })
            }
            14 => {
                let guard = RuntimeAssertionGuardId::try_from_bytes(
                    self.raw(16)?.try_into().expect("source guard width"),
                )?;
                let profile = match self.byte()? {
                    0 => RuntimeAssertionProfile::Always,
                    1 => RuntimeAssertionProfile::DebugOnly,
                    _ => return Err(self.invalid("assertion profile")),
                };
                let condition = self.text(counts)?;
                let message = self.text(counts)?;
                retain.then(|| {
                    Effect::Assert(RuntimeAssertion::new(
                        guard,
                        condition.into(),
                        message.into(),
                        profile,
                    ))
                })
            }
            17 => {
                let label = self.optional_text(counts)?;
                let value = self.optional_text(counts)?;
                retain.then(|| Effect::Break {
                    label: label.map(Into::into),
                    value: value.map(Into::into),
                })
            }
            18 => {
                let label = self.optional_text(counts)?;
                retain.then(|| Effect::Continue {
                    label: label.map(Into::into),
                })
            }
            _ => return Err(self.invalid("effect tag")),
        };
        Ok(result)
    }
    pub fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
}
