//! Source-bound construction operands; complete values remain owned by View.

use arcweft_lang_hir::identity::{ExprId, ItemId};
use arcweft_presentation::appearance::SystemColor;
use arcweft_view::style::{
    ViewFontFamily, ViewStyleTokenId, ViewStyleValueKind, ViewSystemFontFamily,
};

use crate::types::CompileTimeStyleType;

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
}

impl CheckedStyleOperand {
    pub const fn style_type(&self) -> CompileTimeStyleType {
        match self {
            Self::FontFamily(_) => CompileTimeStyleType::FontFamily,
            Self::SystemFontFamily(_) => CompileTimeStyleType::SystemFontFamily,
            Self::SystemColor(_) => CompileTimeStyleType::SystemColor,
            Self::Token(token) => CompileTimeStyleType::Token(token.kind()),
        }
    }
}
