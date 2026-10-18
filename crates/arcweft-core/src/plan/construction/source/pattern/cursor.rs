//! One borrowed row grammar. A row view owns no children or runtime payload.

use super::{
    DOMAIN, RuntimePatternSourceError as Error, RuntimePatternSourceLimits, limit, reference,
};
use crate::runtime_id::{RuntimeLocalDeclarationId, RuntimePlanTypeId};
use std::num::NonZeroU32;

#[derive(Clone, Copy)]
#[repr(u8)]
pub(super) enum Tag {
    Bind,
    Discard,
    Literal,
    Entity,
    Tuple,
    Or,
    Record,
    Sequence,
    Variant,
    Whole,
    Typed,
}

impl Tag {
    fn decode(tag: u8) -> Result<Self, Error> {
        Ok(match tag {
            0 => Self::Bind,
            1 => Self::Discard,
            2 => Self::Literal,
            3 => Self::Entity,
            4 => Self::Tuple,
            5 => Self::Or,
            6 => Self::Record,
            7 => Self::Sequence,
            8 => Self::Variant,
            9 => Self::Whole,
            10 => Self::Typed,
            _ => return Err(Error::Tag { tag }),
        })
    }
}

pub(super) enum WireRest {
    Exact,
    Ignore,
    Bind(RuntimeLocalDeclarationId),
}

pub(super) struct Row<'a> {
    pub(super) ty: RuntimePlanTypeId,
    pub(super) kind: Kind<'a>,
}

/// Ephemeral views of scalar source fields, without a second pattern tree.
pub(super) enum Kind<'a> {
    Bind {
        mutable: bool,
        local: RuntimeLocalDeclarationId,
    },
    Discard,
    Literal(&'a [u8]),
    Entity(&'a [u8]),
    Tuple(usize),
    Or(usize),
    Record {
        fields: &'a [u8],
        rest: WireRest,
    },
    Sequence {
        items: usize,
        rest: WireRest,
    },
    Variant {
        ordinal: u32,
        payload: bool,
    },
    Whole(RuntimeLocalDeclarationId),
    Typed(RuntimeLocalDeclarationId),
}

impl Kind<'_> {
    pub(super) fn children(&self) -> usize {
        match self {
            Self::Tuple(n) | Self::Or(n) | Self::Sequence { items: n, .. } => *n,
            Self::Record { fields, .. } => fields.len() / 4,
            Self::Variant { payload, .. } => usize::from(*payload),
            Self::Whole(_) => 1,
            Self::Bind { .. }
            | Self::Discard
            | Self::Literal(_)
            | Self::Entity(_)
            | Self::Typed(_) => 0,
        }
    }
    pub(super) fn descends(&self) -> bool {
        matches!(
            self,
            Self::Tuple(_) | Self::Record { .. } | Self::Sequence { .. } | Self::Variant { .. }
        )
    }
}

pub(super) struct Cursor<'a> {
    bytes: &'a [u8],
    pub(super) offset: usize,
    limits: RuntimePatternSourceLimits,
}

impl<'a> Cursor<'a> {
    pub(super) fn new(bytes: &'a [u8], limits: RuntimePatternSourceLimits) -> Self {
        Self {
            bytes,
            offset: 0,
            limits,
        }
    }
    fn slice(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Arithmetic)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(Error::Truncated {
            offset: self.offset,
        })?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.slice(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.slice(4)?.try_into().expect("four source bytes"),
        ))
    }
    fn boolean(&mut self) -> Result<bool, Error> {
        match self.byte()? {
            0 => Ok(false),
            1 => Ok(true),
            actual => Err(Error::Flag { actual }),
        }
    }
    fn count(&mut self) -> Result<usize, Error> {
        let count = self.u32()? as usize;
        limit("source elements", count, self.limits.elements as usize)?;
        Ok(count)
    }
    fn local(&mut self) -> Result<RuntimeLocalDeclarationId, Error> {
        let ordinal = self.u32()?;
        reference("local", ordinal, self.limits.local_count)?;
        Ok(RuntimeLocalDeclarationId::from_accepted_ordinal(
            NonZeroU32::new(ordinal).expect("checked source ordinal"),
        ))
    }
    fn literal(&mut self) -> Result<&'a [u8], Error> {
        let length = self.u32()? as usize;
        self.slice(length)
    }
    fn rest(&mut self) -> Result<WireRest, Error> {
        Ok(match self.byte()? {
            0 => WireRest::Exact,
            1 => WireRest::Ignore,
            2 => WireRest::Bind(self.local()?),
            tag => return Err(Error::Tag { tag }),
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
        let nodes = self.u32()? as usize;
        limit("pattern nodes", nodes, self.limits.nodes as usize)?;
        if nodes == 0 {
            return Err(Error::Roots { actual: 0 });
        }
        Ok(nodes)
    }
    pub(super) fn row(&mut self) -> Result<Row<'a>, Error> {
        let tag = Tag::decode(self.byte()?)?;
        let ordinal = self.u32()?;
        reference("type", ordinal, self.limits.type_count)?;
        let ty = RuntimePlanTypeId::from_accepted_ordinal(
            NonZeroU32::new(ordinal).expect("checked source type"),
        );
        let kind = match tag {
            Tag::Bind => Kind::Bind {
                mutable: self.boolean()?,
                local: self.local()?,
            },
            Tag::Discard => Kind::Discard,
            Tag::Literal => Kind::Literal(self.literal()?),
            Tag::Entity => Kind::Entity(self.literal()?),
            Tag::Tuple => Kind::Tuple(self.count()?),
            Tag::Or => {
                let count = self.count()?;
                if count < 2 {
                    return Err(Error::OrArity { actual: count });
                }
                Kind::Or(count)
            }
            Tag::Record => {
                let count = self.count()?;
                let fields = self.slice(count.checked_mul(4).ok_or(Error::Arithmetic)?)?;
                for field in fields.chunks_exact(4) {
                    let actual = u32::from_le_bytes(field.try_into().expect("source field width"));
                    reference("record field", actual, u32::MAX as usize)?;
                }
                Kind::Record {
                    fields,
                    rest: self.rest()?,
                }
            }
            Tag::Sequence => Kind::Sequence {
                items: self.count()?,
                rest: self.rest()?,
            },
            Tag::Variant => Kind::Variant {
                ordinal: self.u32()?,
                payload: self.boolean()?,
            },
            Tag::Whole => Kind::Whole(self.local()?),
            Tag::Typed => Kind::Typed(self.local()?),
        };
        Ok(Row { ty, kind })
    }
    pub(super) fn finish(&self) -> Result<(), Error> {
        if self.offset != self.bytes.len() {
            return Err(Error::Trailing {
                offset: self.offset,
            });
        }
        Ok(())
    }
}
