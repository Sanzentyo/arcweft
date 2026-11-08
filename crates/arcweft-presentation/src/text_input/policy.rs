//! The same closed policy vocabulary is used by Source, product codecs and IME adapters.

use super::{EnterKeyHint, TextInputPurpose};
use arcweft_id::closed_enum::{
    ClosedEnumDomainDescriptor, ClosedEnumDomainId, ClosedEnumMemberDescriptor, ClosedEnumOwnerId,
    ClosedEnumValueId,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TextInputPolicyDomain {
    Purpose = 0,
    EnterKey = 1,
}

impl TextInputPolicyDomain {
    pub const fn domain_id(self) -> ClosedEnumDomainId {
        ClosedEnumDomainId::new(ClosedEnumOwnerId::TextInput, self as u8)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("text input policy value {actual:?} is outside domain {expected:?}")]
pub struct TextInputPolicyValueError {
    pub expected: ClosedEnumDomainId,
    pub actual: ClosedEnumValueId,
}

impl TextInputPurpose {
    pub const ALL: [Self; 11] = [
        Self::Text,
        Self::Search,
        Self::Name,
        Self::Email,
        Self::Url,
        Self::Telephone,
        Self::Number,
        Self::Decimal,
        Self::Password,
        Self::Pin,
        Self::Terminal,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Search => "search",
            Self::Name => "name",
            Self::Email => "email",
            Self::Url => "url",
            Self::Telephone => "telephone",
            Self::Number => "number",
            Self::Decimal => "decimal",
            Self::Password => "password",
            Self::Pin => "pin",
            Self::Terminal => "terminal",
        }
    }

    pub const fn value_id(self) -> ClosedEnumValueId {
        ClosedEnumValueId::new(TextInputPolicyDomain::Purpose.domain_id(), self as u16)
    }

    const MEMBERS: [ClosedEnumMemberDescriptor; Self::ALL.len()] = {
        let mut members = [ClosedEnumMemberDescriptor::new(0, ""); Self::ALL.len()];
        let mut index = 0;
        while index < Self::ALL.len() {
            let value = Self::ALL[index];
            members[index] = ClosedEnumMemberDescriptor::new(value as u16, value.source_name());
            index += 1;
        }
        members
    };
}

impl TryFrom<ClosedEnumValueId> for TextInputPurpose {
    type Error = TextInputPolicyValueError;

    fn try_from(actual: ClosedEnumValueId) -> Result<Self, Self::Error> {
        let expected = TextInputPolicyDomain::Purpose.domain_id();
        Self::ALL
            .into_iter()
            .find(|value| value.value_id() == actual)
            .ok_or(TextInputPolicyValueError { expected, actual })
    }
}

impl EnterKeyHint {
    pub const ALL: [Self; 7] = [
        Self::Default,
        Self::Enter,
        Self::Done,
        Self::Go,
        Self::Next,
        Self::Search,
        Self::Send,
    ];

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Enter => "enter",
            Self::Done => "done",
            Self::Go => "go",
            Self::Next => "next",
            Self::Search => "search",
            Self::Send => "send",
        }
    }

    pub const fn value_id(self) -> ClosedEnumValueId {
        ClosedEnumValueId::new(TextInputPolicyDomain::EnterKey.domain_id(), self as u16)
    }

    const MEMBERS: [ClosedEnumMemberDescriptor; Self::ALL.len()] = {
        let mut members = [ClosedEnumMemberDescriptor::new(0, ""); Self::ALL.len()];
        let mut index = 0;
        while index < Self::ALL.len() {
            let value = Self::ALL[index];
            members[index] = ClosedEnumMemberDescriptor::new(value as u16, value.source_name());
            index += 1;
        }
        members
    };
}

impl TryFrom<ClosedEnumValueId> for EnterKeyHint {
    type Error = TextInputPolicyValueError;

    fn try_from(actual: ClosedEnumValueId) -> Result<Self, Self::Error> {
        let expected = TextInputPolicyDomain::EnterKey.domain_id();
        Self::ALL
            .into_iter()
            .find(|value| value.value_id() == actual)
            .ok_or(TextInputPolicyValueError { expected, actual })
    }
}

pub const TEXT_INPUT_CLOSED_ENUM_DOMAINS: &[ClosedEnumDomainDescriptor] = &[
    ClosedEnumDomainDescriptor::new(
        TextInputPolicyDomain::Purpose.domain_id(),
        "text input purpose",
        &TextInputPurpose::MEMBERS,
    ),
    ClosedEnumDomainDescriptor::new(
        TextInputPolicyDomain::EnterKey.domain_id(),
        "text input enter key",
        &EnterKeyHint::MEMBERS,
    ),
];
