//! Lossless grammar vocabulary shared by attached syntax consumers.

pub(crate) mod assertion_projection;
pub(crate) mod attribute_projection;
pub(crate) mod budget;
pub(crate) mod build;
pub(crate) mod callable_projection;
pub(crate) mod contract_projection;
pub(crate) mod declaration_projection;
pub(crate) mod entry_projection;
pub(crate) mod event;
pub(crate) mod flow_projection;
pub(crate) mod keyword_statement_projection;
pub(crate) mod kinds;
pub(crate) mod roles;
pub(crate) mod source_projection;
pub(crate) mod style_projection;
pub(crate) mod test_projection;
pub(crate) mod view_projection;

#[cfg(test)]
mod projection_tests;

pub use keyword_statement_projection::{SyntaxAwaitBranchKind, SyntaxSelectStatementForm};
pub use kinds::{AstTag, IdentityClass, SyntaxKind};
pub use roles::{
    ActivityPolicySyntaxValue, LayerKindSyntaxValue, LayerMemberSyntaxKind, LayerPolicySyntaxValue,
    MetricKindSyntaxValue, SyntaxRole, SyntaxRoleClass,
};
