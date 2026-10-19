//! Borrowed prefix framing. Operation fields, value roots and owned bodies
//! have independent exact extents; none can consume another row's bytes.

mod fields;

use super::{Counts, DOMAIN, Error};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::task::semantic::TaskSemanticMeter;

pub(super) struct Row<'a> {
    pub tag: u8,
    pub metadata: &'a [u8],
    pub values: &'a [u8],
    pub value_count: usize,
    pub bodies: &'a [u8],
    pub body_count: usize,
    pub children: usize,
    pub prefix: super::Footprint,
}

pub(super) struct Cursor<'a> {
    pub bytes: &'a [u8],
    pub offset: usize,
}
impl<'a> Cursor<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub fn raw(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(count).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    pub fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.raw(1)?[0])
    }
    pub fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.raw(4)?.try_into().expect("source u32 width"),
        ))
    }
    pub fn i32(&mut self) -> Result<i32, Error> {
        Ok(i32::from_le_bytes(
            self.raw(4)?.try_into().expect("source i32 width"),
        ))
    }
    pub fn digest(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.raw(32)?.try_into().expect("source digest width"))
    }
    pub fn frame(&mut self) -> Result<&'a [u8], Error> {
        let count = self.u32()? as usize;
        self.raw(count)
    }
    pub fn text(&mut self) -> Result<&'a str, Error> {
        Ok(std::str::from_utf8(self.frame()?)?)
    }
    pub fn flag(&mut self, kind: &'static str) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(Error::Tag { kind, tag }),
        }
    }
    pub fn optional_text(&mut self) -> Result<Option<&'a str>, Error> {
        if self.flag("optional text")? {
            self.text().map(Some)
        } else {
            Ok(None)
        }
    }
    pub fn positive(&mut self, kind: &'static str, maximum: usize) -> Result<u32, Error> {
        let actual = self.u32()?;
        if actual == 0 || actual as usize > maximum {
            return Err(Error::Reference {
                kind,
                actual,
                maximum,
            });
        }
        Ok(actual)
    }
    pub fn zero_based(&mut self, kind: &'static str, maximum: usize) -> Result<u32, Error> {
        let actual = self.u32()?;
        if actual as usize >= maximum {
            return Err(Error::Reference {
                kind,
                actual,
                maximum,
            });
        }
        Ok(actual)
    }
    pub fn header(&mut self) -> Result<(usize, usize), Error> {
        if self.raw(DOMAIN.len())? != DOMAIN {
            return Err(Error::Domain);
        }
        let actual = self.byte()?;
        if actual != 1 {
            return Err(Error::Version { actual });
        }
        Ok((self.u32()? as usize, self.u32()? as usize))
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
    pub fn operation(
        &mut self,
        counts: &mut Counts,
        resources: &ValueResourceAdmission,
        meter: &mut TaskSemanticMeter,
    ) -> Result<Row<'a>, Error> {
        let tag = self.byte()?;
        let metadata = self.frame()?;
        let value_count = self.u32()? as usize;
        counts.elements(value_count)?;
        let start = self.offset;
        for _ in 0..value_count {
            self.value()?;
        }
        let values = &self.bytes[start..self.offset];
        let body_count = self.u32()? as usize;
        counts.elements(body_count)?;
        let bodies = self.raw(body_count.checked_mul(4).ok_or(Error::Arithmetic)?)?;
        let mut body_reader = Self::new(bodies);
        let mut children = 0usize;
        for _ in 0..body_count {
            let count = body_reader.u32()? as usize;
            counts.body(count)?;
            children = children.checked_add(count).ok_or(Error::Arithmetic)?;
        }
        let row = Row {
            tag,
            metadata,
            values,
            value_count,
            bodies,
            body_count,
            children,
            prefix: counts.footprint,
        };
        fields::preflight(&row, counts, resources, meter)?;
        Ok(row)
    }
    pub fn value(&mut self) -> Result<(u8, &'a [u8]), Error> {
        let kind = self.byte()?;
        if kind > 1 {
            return Err(Error::Tag {
                kind: "value root",
                tag: kind,
            });
        }
        Ok((kind, self.frame()?))
    }
}
