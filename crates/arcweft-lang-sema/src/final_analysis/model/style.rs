//! Source-bound construction operands; complete values remain owned by View.

use arcweft_lang_hir::identity::{ExprId, ItemId};
use arcweft_presentation::appearance::SystemColor;
use arcweft_view::style::{
    ViewFilter, ViewFontFamily, ViewPropertyKind, ViewShadow, ViewStyleTokenId,
    ViewStyleTransition, ViewStyleValueKind, ViewSystemFontFamily,
};

use crate::types::CompileTimeStyleType;

#[path = "style_number.rs"]
mod number;
pub use number::{CheckedStyleNumber, CheckedStyleNumberError};

/// A reference issued from the original sheet-local token declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedStyleTokenReference {
    style: ItemId,
    ordinal: u32,
    initializer: ExprId,
    id: ViewStyleTokenId,
    kind: ViewStyleValueKind,
}

impl CheckedStyleTokenReference {
    pub(in crate::final_analysis) const fn new(
        style: ItemId,
        ordinal: u32,
        initializer: ExprId,
        id: ViewStyleTokenId,
        kind: ViewStyleValueKind,
    ) -> Self {
        Self {
            style,
            ordinal,
            initializer,
            id,
            kind,
        }
    }

    pub const fn style(&self) -> ItemId {
        self.style
    }
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
    pub const fn initializer(&self) -> ExprId {
        self.initializer
    }
    pub const fn id(&self) -> &ViewStyleTokenId {
        &self.id
    }
    pub const fn kind(&self) -> ViewStyleValueKind {
        self.kind
    }

    pub(in crate::final_analysis) fn matches_source_path(
        &self,
        module: &arcweft_lang_hir::module::HirModule,
        mut expression: ExprId,
    ) -> bool {
        use arcweft_lang_hir::{
            expr::{HirExprKind, HirSelectedMember},
            leaf::{HirPathRoot, HirPathSegment},
        };
        let mut expected = self.id.public_id().as_str().split('.').rev();
        loop {
            let Ok(node) = module.resolve_expr(expression) else {
                return false;
            };
            match node.kind() {
                HirExprKind::Select(select) => {
                    let HirSelectedMember::Name(member) = select.member() else {
                        return false;
                    };
                    if expected.next() != Some(member.as_str()) {
                        return false;
                    }
                    expression = select.target();
                }
                HirExprKind::Path(path) => {
                    let Some(path) = path.as_resolved() else {
                        return false;
                    };
                    return path.root() == HirPathRoot::ImplicitCrate
                        && path
                            .segments()
                            .iter()
                            .rev()
                            .map(|segment| match segment {
                                HirPathSegment::Identifier(name) => name.as_str(),
                                HirPathSegment::ProjectSymbol(name) => name.as_str(),
                            })
                            .eq(expected);
                }
                _ => return false,
            }
        }
    }
}

/// Operands are neither standalone property values nor Core runtime values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedStyleOperand {
    FontFamily(ViewFontFamily),
    SystemFontFamily(ViewSystemFontFamily),
    SystemColor(SystemColor),
    Token(CheckedStyleTokenReference),
    Number(CheckedStyleNumber),
    Shadow(ViewShadow),
    Filter(ViewFilter),
    Transition(ViewStyleTransition),
    Property(ViewPropertyKind),
    Duration(CheckedStyleDuration),
    ResourceId(arcweft_id::PublicId),
}

impl CheckedStyleOperand {
    pub const fn style_type(&self) -> CompileTimeStyleType {
        match self {
            Self::FontFamily(_) => CompileTimeStyleType::FontFamily,
            Self::SystemFontFamily(_) => CompileTimeStyleType::SystemFontFamily,
            Self::SystemColor(_) => CompileTimeStyleType::SystemColor,
            Self::Token(token) => CompileTimeStyleType::Token(token.kind()),
            Self::Number(number) => CompileTimeStyleType::NumericLiteral(number.kind()),
            Self::Shadow(_) => CompileTimeStyleType::Shadow,
            Self::Filter(_) => CompileTimeStyleType::Filter,
            Self::Transition(_) => CompileTimeStyleType::Transition,
            Self::Property(_) => CompileTimeStyleType::Property,
            Self::Duration(_) => CompileTimeStyleType::Duration,
            Self::ResourceId(_) => CompileTimeStyleType::ResourceId,
        }
    }
}

/// Exact whole milliseconds admitted by the native transition carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedStyleDuration(u32);

#[derive(Clone, Copy, Debug, Eq, thiserror::Error, PartialEq)]
pub enum CheckedStyleDurationError {
    #[error("expected a nonnegative Duration literal")]
    WrongLiteral,
    #[error("Style transition times must be exact whole milliseconds")]
    PrecisionLoss,
    #[error("Style transition time exceeds the native u32 millisecond range")]
    OutOfRange,
}

impl CheckedStyleDuration {
    pub(in crate::final_analysis) const ZERO: Self = Self(0);

    pub const fn milliseconds(self) -> u32 {
        self.0
    }
}

impl TryFrom<&arcweft_lang_hir::leaf::HirLiteral> for CheckedStyleDuration {
    type Error = CheckedStyleDurationError;

    fn try_from(literal: &arcweft_lang_hir::leaf::HirLiteral) -> Result<Self, Self::Error> {
        use arcweft_lang_hir::leaf::{HirDurationLiteral, HirLiteral};
        let HirLiteral::Duration(HirDurationLiteral::Value(duration)) = literal else {
            return Err(CheckedStyleDurationError::WrongLiteral);
        };
        let nanos = match duration.semantic_value().nanoseconds().limbs_le() {
            [] => 0,
            [low] => u64::from(*low),
            [low, high] => u64::from(*low) | (u64::from(*high) << 32),
            _ => return Err(CheckedStyleDurationError::OutOfRange),
        };
        if nanos % 1_000_000 != 0 {
            return Err(CheckedStyleDurationError::PrecisionLoss);
        }
        u32::try_from(nanos / 1_000_000)
            .map(Self)
            .map_err(|_| CheckedStyleDurationError::OutOfRange)
    }
}
