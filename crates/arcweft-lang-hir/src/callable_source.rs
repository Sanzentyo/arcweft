//! Revision-bound source publication for project callables.

use std::sync::Arc;

use arcweft_lang_syntax::{
    ast::{common::DocBlock, module_path::CanonicalModulePath, symbol_path::SymbolPath},
    types::FnSignature,
};
use arcweft_source::SourceSpan;
use thiserror::Error;

use crate::symbol::{CallableDeclarationId, CallablePackageId};

/// One source declaration's typed callable signature and exact source evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallableSignatureSource {
    declaration: CallableDeclarationId,
    package: CallablePackageId,
    module: CanonicalModulePath,
    path: SymbolPath,
    signature: FnSignature,
    documentation: Option<DocBlock>,
    declaration_span: SourceSpan,
    name_span: SourceSpan,
    signature_span: SourceSpan,
    result_span: Option<SourceSpan>,
    parameter_spans: Arc<[HirCallableParameterSource]>,
    effects: HirCallableEffects,
}

/// Exact source evidence for one callable parameter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallableParameterSource {
    group: u16,
    parameter: u16,
    whole: SourceSpan,
    name: Option<SourceSpan>,
    ty: Option<SourceSpan>,
    default: Option<SourceSpan>,
}

/// Validated source spelling of a declared effect capability.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirEffectName(Arc<str>);

/// Declared effect capabilities for one source callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallableEffects {
    declared: Arc<[HirEffectName]>,
}

/// Invalid source effect spelling.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum HirEffectNameError {
    #[error("HIR effect name cannot be empty")]
    Empty,
    #[error("HIR effect name contains a control character at byte {byte}")]
    Control { byte: usize },
}

impl HirCallableSignatureSource {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        declaration: CallableDeclarationId,
        package: CallablePackageId,
        module: CanonicalModulePath,
        path: SymbolPath,
        signature: FnSignature,
        documentation: Option<DocBlock>,
        declaration_span: SourceSpan,
        name_span: SourceSpan,
        signature_span: SourceSpan,
        result_span: Option<SourceSpan>,
        parameter_spans: Vec<HirCallableParameterSource>,
        effects: HirCallableEffects,
    ) -> Self {
        Self {
            declaration,
            package,
            module,
            path,
            signature,
            documentation,
            declaration_span,
            name_span,
            signature_span,
            result_span,
            parameter_spans: parameter_spans.into(),
            effects,
        }
    }

    pub const fn declaration(&self) -> &CallableDeclarationId {
        &self.declaration
    }

    pub const fn package(&self) -> &CallablePackageId {
        &self.package
    }

    pub const fn module(&self) -> &CanonicalModulePath {
        &self.module
    }

    pub const fn path(&self) -> &SymbolPath {
        &self.path
    }

    pub const fn signature(&self) -> &FnSignature {
        &self.signature
    }

    pub const fn documentation(&self) -> Option<&DocBlock> {
        self.documentation.as_ref()
    }

    pub const fn declaration_span(&self) -> &SourceSpan {
        &self.declaration_span
    }

    pub const fn name_span(&self) -> &SourceSpan {
        &self.name_span
    }

    pub const fn signature_span(&self) -> &SourceSpan {
        &self.signature_span
    }

    pub const fn result_span(&self) -> Option<&SourceSpan> {
        self.result_span.as_ref()
    }

    pub fn parameter_spans(&self) -> &[HirCallableParameterSource] {
        &self.parameter_spans
    }

    pub const fn effects(&self) -> &HirCallableEffects {
        &self.effects
    }
}

impl HirCallableParameterSource {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        group: u16,
        parameter: u16,
        whole: SourceSpan,
        name: Option<SourceSpan>,
        ty: Option<SourceSpan>,
        default: Option<SourceSpan>,
    ) -> Self {
        Self {
            group,
            parameter,
            whole,
            name,
            ty,
            default,
        }
    }

    pub const fn group(&self) -> u16 {
        self.group
    }

    pub const fn parameter(&self) -> u16 {
        self.parameter
    }

    pub const fn whole(&self) -> &SourceSpan {
        &self.whole
    }

    pub const fn name(&self) -> Option<&SourceSpan> {
        self.name.as_ref()
    }

    pub const fn ty(&self) -> Option<&SourceSpan> {
        self.ty.as_ref()
    }

    pub const fn default(&self) -> Option<&SourceSpan> {
        self.default.as_ref()
    }
}

impl HirEffectName {
    pub fn try_new(value: impl Into<Arc<str>>) -> Result<Self, HirEffectNameError> {
        let value = value.into();
        if value.is_empty() {
            return Err(HirEffectNameError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(HirEffectNameError::Control { byte });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl HirCallableEffects {
    pub(crate) fn new(declared: Vec<HirEffectName>) -> Self {
        Self {
            declared: declared.into(),
        }
    }

    pub fn declared(&self) -> &[HirEffectName] {
        &self.declared
    }
}
