//! Attached closed policy vocabularies for Source declarations.

use crate::name::SyntaxName;

use super::{AstNode, AttachedSourceExpression, CallArgumentKind};

/// One selected `bounded(...)` argument with exact Call argument ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceBoundedArgument {
    Missing,
    Present {
        syntax: AstNode<CallArgumentKind>,
        ordinal: u16,
        value: AttachedSourceExpression,
        duplicate: bool,
    },
}

impl AttachedSourceBoundedArgument {
    pub const fn value(&self) -> Option<&AttachedSourceExpression> {
        match self {
            Self::Missing => None,
            Self::Present { value, .. } => Some(value),
        }
    }

    pub const fn is_duplicate(&self) -> bool {
        matches!(
            self,
            Self::Present {
                duplicate: true,
                ..
            }
        )
    }

    pub fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing)
            || self
                .value()
                .is_some_and(AttachedSourceExpression::has_recovery)
            || self.is_duplicate()
    }
}

/// Closed overflow policy or typed recovery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceOverflowPolicy {
    DropOldest(AttachedSourceBoundedArgument),
    DropNewest(AttachedSourceBoundedArgument),
    Error(AttachedSourceBoundedArgument),
    Coalesce(AttachedSourceBoundedArgument),
    Missing,
    Unknown {
        argument: AttachedSourceBoundedArgument,
        value: Option<SyntaxName>,
    },
    Invalid(AttachedSourceBoundedArgument),
}

impl AttachedSourceOverflowPolicy {
    pub const fn argument(&self) -> Option<&AttachedSourceBoundedArgument> {
        match self {
            Self::DropOldest(argument)
            | Self::DropNewest(argument)
            | Self::Error(argument)
            | Self::Coalesce(argument)
            | Self::Unknown { argument, .. }
            | Self::Invalid(argument) => Some(argument),
            Self::Missing => None,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::DropOldest(argument)
            | Self::DropNewest(argument)
            | Self::Error(argument)
            | Self::Coalesce(argument) => argument.has_recovery(),
            Self::Missing | Self::Unknown { .. } | Self::Invalid(_) => true,
        }
    }
}

/// Closed backpressure policy or typed recovery without a fabricated default.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedSourceBackpressurePolicy {
    Latest(AttachedSourceExpression),
    Bounded {
        expression: AttachedSourceExpression,
        capacity: AttachedSourceBoundedArgument,
        overflow: AttachedSourceOverflowPolicy,
        unexpected_arguments: bool,
        recovered_call: bool,
    },
    BlockingNotAllowed(AttachedSourceExpression),
    Missing(AttachedSourceExpression),
    Unknown {
        expression: AttachedSourceExpression,
        value: Option<SyntaxName>,
    },
    Invalid(AttachedSourceExpression),
}

impl AttachedSourceBackpressurePolicy {
    pub const fn expression(&self) -> &AttachedSourceExpression {
        match self {
            Self::Latest(expression)
            | Self::BlockingNotAllowed(expression)
            | Self::Missing(expression)
            | Self::Unknown { expression, .. }
            | Self::Invalid(expression)
            | Self::Bounded { expression, .. } => expression,
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Latest(value) | Self::BlockingNotAllowed(value) => value.has_recovery(),
            Self::Bounded {
                expression,
                capacity,
                overflow,
                unexpected_arguments,
                recovered_call,
            } => {
                expression.has_recovery()
                    || capacity.has_recovery()
                    || overflow.has_recovery()
                    || *unexpected_arguments
                    || *recovered_call
            }
            Self::Missing(_) | Self::Unknown { .. } | Self::Invalid(_) => true,
        }
    }
}

macro_rules! named_source_policy {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Clone, Debug, Eq, PartialEq)]
        pub enum $name {
            $($variant(AttachedSourceExpression)),+,
            Missing(AttachedSourceExpression),
            Unknown {
                expression: AttachedSourceExpression,
                value: Option<SyntaxName>,
            },
            Invalid(AttachedSourceExpression),
        }

        impl $name {
            pub const fn expression(&self) -> &AttachedSourceExpression {
                match self {
                    $(Self::$variant(expression))|+
                    | Self::Missing(expression)
                    | Self::Unknown { expression, .. }
                    | Self::Invalid(expression) => expression,
                }
            }

            pub fn has_recovery(&self) -> bool {
                match self {
                    $(Self::$variant(value) => value.has_recovery(),)+
                    Self::Missing(_) | Self::Unknown { .. } | Self::Invalid(_) => true,
                }
            }
        }
    };
}

named_source_policy!(AttachedSourceReplayPolicy {
    Full,
    HashOnly,
    Summary,
    EventOnly,
    None,
});
named_source_policy!(AttachedSourcePrivacyPolicy {
    Transient,
    Redacted,
    Recordable,
    Private,
});
