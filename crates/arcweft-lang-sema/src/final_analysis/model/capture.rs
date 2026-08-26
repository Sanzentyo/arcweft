//! Generation-bound terminal capture facts.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use arcweft_lang_hir::{
    expr::HirPlaceholderKind, project::HirProjectEvaluationTopology, scope::CaptureAccess,
};
use thiserror::Error;

use super::super::{CheckedExpressionResolution, ExprId, LocalId, TypeKind};

/// Failure to seal or consume terminal capture evidence against its exact HIR
/// topology authority.
///
/// These are compiler invariants, not ordinary overload-candidate rejection.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum CheckedCaptureAuthorityViolation {
    #[error("checked capture fact belongs to another HIR topology allocation")]
    TopologyMismatch,
    #[error("checked capture fact producer differs: expected {expected:?}, found {actual:?}")]
    ProducerMismatch { expected: ExprId, actual: ExprId },
    #[error("checked capture producer is absent from the HIR topology: {owner:?}")]
    MissingProducer { owner: ExprId },
    #[error("checked capture use is absent from the HIR topology: {expression:?}")]
    MissingExpressionUse { expression: ExprId },
    #[error("checked capture local has no topology binding origin: {local:?}")]
    MissingLocalBinding { local: LocalId },
    #[error("checked implicit capture use resolves to a region-internal binding: {local:?}")]
    InternalLocalBinding { local: LocalId },
    #[error("checked implicit capture contains duplicate use evidence: {expression:?}")]
    DuplicateUse { expression: ExprId },
    #[error("checked implicit callable placeholder evidence differs from HIR topology")]
    PlaceholderEvidenceMismatch,
    #[error("checked terminal capture evidence differs from HIR topology")]
    CaptureEvidenceMismatch,
}

/// One accepted callable capture with its exact access mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedCapture {
    local: LocalId,
    mode: CaptureAccess,
}

impl CheckedCapture {
    const fn new(local: LocalId, mode: CaptureAccess) -> Self {
        Self { local, mode }
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn mode(&self) -> CaptureAccess {
        self.mode
    }
}

/// One topology-authenticated use that contributes to an implicit callable's
/// capture set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckedImplicitCaptureUse {
    expression: ExprId,
    local: LocalId,
    access: CaptureAccess,
    source_ordinal: u32,
}

impl CheckedImplicitCaptureUse {
    pub const fn expression(&self) -> ExprId {
        self.expression
    }

    pub const fn local(&self) -> LocalId {
        self.local
    }

    pub const fn access(&self) -> CaptureAccess {
        self.access
    }

    pub const fn source_ordinal(&self) -> u32 {
        self.source_ordinal
    }
}

/// Exact capture authority for one explicit closure producer.
#[derive(Clone)]
pub struct CheckedClosure {
    topology: Arc<HirProjectEvaluationTopology>,
    owner: ExprId,
    captures: Box<[CheckedCapture]>,
}

impl CheckedClosure {
    pub(crate) fn seal(
        topology: Arc<HirProjectEvaluationTopology>,
        owner: ExprId,
    ) -> Result<Self, CheckedCaptureAuthorityViolation> {
        let rows = topology
            .module(owner.module())
            .and_then(|module| module.captures().captures_for_closure(owner))
            .ok_or(CheckedCaptureAuthorityViolation::MissingProducer { owner })?;
        let captures = rows
            .iter()
            .map(|row| CheckedCapture::new(row.local(), row.access()))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let checked = Self {
            topology,
            owner,
            captures,
        };
        checked.validate_evidence()?;
        Ok(checked)
    }

    pub const fn owner(&self) -> ExprId {
        self.owner
    }

    #[cfg(test)]
    pub(crate) const fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        &self.topology
    }

    pub const fn captures(&self) -> &[CheckedCapture] {
        &self.captures
    }

    pub(crate) fn validate_authority(
        &self,
        expected: &Arc<HirProjectEvaluationTopology>,
        producer: ExprId,
    ) -> Result<&[CheckedCapture], CheckedCaptureAuthorityViolation> {
        if !Arc::ptr_eq(&self.topology, expected) {
            return Err(CheckedCaptureAuthorityViolation::TopologyMismatch);
        }
        if self.owner != producer {
            return Err(CheckedCaptureAuthorityViolation::ProducerMismatch {
                expected: producer,
                actual: self.owner,
            });
        }
        self.validate_evidence()?;
        Ok(&self.captures)
    }

    fn validate_evidence(&self) -> Result<(), CheckedCaptureAuthorityViolation> {
        let rows = self
            .topology
            .module(self.owner.module())
            .and_then(|module| module.captures().captures_for_closure(self.owner))
            .ok_or(CheckedCaptureAuthorityViolation::MissingProducer { owner: self.owner })?;
        let mut seen = BTreeSet::new();
        (rows.len() == self.captures.len()
            && rows.iter().zip(self.captures.iter()).all(|(row, checked)| {
                row.closure() == self.owner
                    && row.local() == checked.local
                    && row.access() == checked.mode
                    && seen.insert(checked.local)
            }))
        .then_some(())
        .ok_or(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)
    }
}

impl fmt::Debug for CheckedClosure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedClosure")
            .field("topology", &"generation-bound")
            .field("owner", &self.owner)
            .field("captures", &self.captures)
            .finish()
    }
}

impl PartialEq for CheckedClosure {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.topology, &other.topology)
            && self.owner == other.owner
            && self.captures == other.captures
    }
}

impl Eq for CheckedClosure {}

/// Checked implicit callable introduced by one or more `_` placeholders.
#[derive(Clone)]
pub struct CheckedImplicitCallable {
    topology: Arc<HirProjectEvaluationTopology>,
    owner: ExprId,
    parameter: TypeKind,
    result: TypeKind,
    placeholders: Box<[ExprId]>,
    capture_uses: Box<[CheckedImplicitCaptureUse]>,
    captures: Box<[CheckedCapture]>,
    body_resolution: Box<CheckedExpressionResolution>,
}

impl CheckedImplicitCallable {
    pub(crate) fn seal(
        topology: Arc<HirProjectEvaluationTopology>,
        owner: ExprId,
        parameter: TypeKind,
        result: TypeKind,
        uses: impl Into<Box<[(ExprId, LocalId)]>>,
        body_resolution: CheckedExpressionResolution,
    ) -> Result<Self, CheckedCaptureAuthorityViolation> {
        let module = topology
            .module(owner.module())
            .ok_or(CheckedCaptureAuthorityViolation::MissingProducer { owner })?;
        let region = module
            .expression_uses()
            .implicit_callable_region(owner, HirPlaceholderKind::PartialApplication)
            .map_err(|_| CheckedCaptureAuthorityViolation::MissingProducer { owner })?;
        let placeholders = region.placeholders().collect::<Vec<_>>().into_boxed_slice();
        if placeholders.is_empty() {
            return Err(CheckedCaptureAuthorityViolation::PlaceholderEvidenceMismatch);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut capture_uses = uses
            .into()
            .iter()
            .map(|(expression, local)| {
                if !seen.insert(*expression) {
                    return Err(CheckedCaptureAuthorityViolation::DuplicateUse {
                        expression: *expression,
                    });
                }
                let row = module
                    .expression_uses()
                    .row(*expression)
                    .filter(|_| region.contains_expression(*expression))
                    .ok_or(CheckedCaptureAuthorityViolation::MissingExpressionUse {
                        expression: *expression,
                    })?;
                let binding = module.local_origins().binding(*local).ok_or(
                    CheckedCaptureAuthorityViolation::MissingLocalBinding { local: *local },
                )?;
                if region.contains_binding(binding) {
                    return Err(CheckedCaptureAuthorityViolation::InternalLocalBinding {
                        local: *local,
                    });
                }
                Ok(CheckedImplicitCaptureUse {
                    expression: *expression,
                    local: *local,
                    access: row.capture_access(),
                    source_ordinal: row.source_ordinal(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        capture_uses.sort_by_key(CheckedImplicitCaptureUse::source_ordinal);
        let captures = aggregate_uses(&capture_uses);
        let checked = Self {
            topology,
            owner,
            parameter,
            result,
            placeholders,
            capture_uses: capture_uses.into_boxed_slice(),
            captures,
            body_resolution: Box::new(body_resolution),
        };
        checked.validate_evidence()?;
        Ok(checked)
    }

    pub const fn owner(&self) -> ExprId {
        self.owner
    }

    #[cfg(test)]
    pub(crate) const fn topology(&self) -> &Arc<HirProjectEvaluationTopology> {
        &self.topology
    }

    pub const fn parameter(&self) -> &TypeKind {
        &self.parameter
    }

    pub const fn result(&self) -> &TypeKind {
        &self.result
    }

    pub const fn placeholders(&self) -> &[ExprId] {
        &self.placeholders
    }

    pub const fn capture_uses(&self) -> &[CheckedImplicitCaptureUse] {
        &self.capture_uses
    }

    pub const fn captures(&self) -> &[CheckedCapture] {
        &self.captures
    }

    pub fn body_resolution(&self) -> &CheckedExpressionResolution {
        self.body_resolution.as_ref()
    }

    pub(crate) fn visit_types<E>(
        &self,
        visitor: &mut impl FnMut(&TypeKind) -> Result<(), E>,
    ) -> Result<(), E> {
        visitor(self.parameter())?;
        visitor(self.result())?;
        self.body_resolution().visit_types(visitor)
    }

    pub(crate) fn validate_authority(
        &self,
        expected: &Arc<HirProjectEvaluationTopology>,
        producer: ExprId,
    ) -> Result<&[CheckedCapture], CheckedCaptureAuthorityViolation> {
        if !Arc::ptr_eq(&self.topology, expected) {
            return Err(CheckedCaptureAuthorityViolation::TopologyMismatch);
        }
        if self.owner != producer {
            return Err(CheckedCaptureAuthorityViolation::ProducerMismatch {
                expected: producer,
                actual: self.owner,
            });
        }
        self.validate_evidence()?;
        Ok(&self.captures)
    }

    fn validate_evidence(&self) -> Result<(), CheckedCaptureAuthorityViolation> {
        let module = self
            .topology
            .module(self.owner.module())
            .ok_or(CheckedCaptureAuthorityViolation::MissingProducer { owner: self.owner })?;
        let region = module
            .expression_uses()
            .implicit_callable_region(self.owner, HirPlaceholderKind::PartialApplication)
            .map_err(|_| CheckedCaptureAuthorityViolation::MissingProducer { owner: self.owner })?;
        let placeholders = region.placeholders().collect::<Vec<_>>();
        if placeholders.as_slice() != self.placeholders.as_ref() {
            return Err(CheckedCaptureAuthorityViolation::PlaceholderEvidenceMismatch);
        }
        let mut previous = None;
        for capture_use in &self.capture_uses {
            if previous.is_some_and(|ordinal| ordinal >= capture_use.source_ordinal) {
                return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
            }
            previous = Some(capture_use.source_ordinal);
            let row = module
                .expression_uses()
                .row(capture_use.expression)
                .filter(|_| region.contains_expression(capture_use.expression))
                .ok_or(CheckedCaptureAuthorityViolation::MissingExpressionUse {
                    expression: capture_use.expression,
                })?;
            let binding = module.local_origins().binding(capture_use.local).ok_or(
                CheckedCaptureAuthorityViolation::MissingLocalBinding {
                    local: capture_use.local,
                },
            )?;
            if region.contains_binding(binding) {
                return Err(CheckedCaptureAuthorityViolation::InternalLocalBinding {
                    local: capture_use.local,
                });
            }
            if row.source_ordinal() != capture_use.source_ordinal
                || row.capture_access() != capture_use.access
            {
                return Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch);
            }
        }
        (aggregate_uses(&self.capture_uses) == self.captures)
            .then_some(())
            .ok_or(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch)
    }
}

impl fmt::Debug for CheckedImplicitCallable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CheckedImplicitCallable")
            .field("topology", &"generation-bound")
            .field("owner", &self.owner)
            .field("parameter", &self.parameter)
            .field("result", &self.result)
            .field("placeholders", &self.placeholders)
            .field("capture_uses", &self.capture_uses)
            .field("captures", &self.captures)
            .field("body_resolution", &self.body_resolution)
            .finish()
    }
}

impl PartialEq for CheckedImplicitCallable {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.topology, &other.topology)
            && self.owner == other.owner
            && self.parameter == other.parameter
            && self.result == other.result
            && self.placeholders == other.placeholders
            && self.capture_uses == other.capture_uses
            && self.captures == other.captures
            && self.body_resolution == other.body_resolution
    }
}

impl Eq for CheckedImplicitCallable {}

fn aggregate_uses(uses: &[CheckedImplicitCaptureUse]) -> Box<[CheckedCapture]> {
    let mut by_local = BTreeMap::<LocalId, usize>::new();
    let mut captures = Vec::<CheckedCapture>::new();
    for capture_use in uses {
        if let Some(index) = by_local.get(&capture_use.local).copied() {
            if matches!(capture_use.access, CaptureAccess::Reassign) {
                captures[index].mode = CaptureAccess::Reassign;
            }
        } else {
            by_local.insert(capture_use.local, captures.len());
            captures.push(CheckedCapture::new(capture_use.local, capture_use.access));
        }
    }
    captures.into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use arcweft_lang_hir::expr::HirExprKind;
    use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;

    use super::*;

    #[test]
    fn checked_closure_equality_and_validation_require_exact_topology_and_evidence() {
        let fixture = crate::final_analysis::tests::fixture(
            "fn caller() { let first = 1i64; let second = 2i64; let value = || -> i64 { second + first }; value(); }\n",
            None,
        );
        let executable = fixture.project.executable_view().expect("executable HIR");
        let module = executable
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Closure(_)).then_some(owner)
            })
            .expect("closure owner");
        let topology = executable
            .accept_symbol_generation(&fixture.symbols)
            .expect("accepted generation")
            .into_evaluation_topology()
            .expect("topology");
        let foreign = executable
            .accept_symbol_generation(&fixture.symbols)
            .expect("second accepted generation")
            .into_evaluation_topology()
            .expect("foreign topology allocation");
        let checked = CheckedClosure::seal(Arc::clone(&topology), owner).expect("sealed closure");
        let foreign_checked =
            CheckedClosure::seal(Arc::clone(&foreign), owner).expect("foreign sealed closure");

        assert_eq!(checked, checked.clone());
        assert_ne!(checked, foreign_checked);
        assert_eq!(
            checked.validate_authority(&foreign, owner),
            Err(CheckedCaptureAuthorityViolation::TopologyMismatch),
        );

        let mut tampered = checked.clone();
        assert!(tampered.captures.len() >= 2);
        tampered.captures.swap(0, 1);
        assert_eq!(
            tampered.validate_authority(&topology, owner),
            Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch),
        );
    }

    #[test]
    fn implicit_capture_seal_uses_first_use_order_and_reassign_dominance() {
        let fixture = crate::final_analysis::tests::fixture(
            concat!(
                "fn caller() {\n",
                "    let mut first = 1i64;\n",
                "    let second = 2i64;\n",
                "    result { second; first = _; second; () };\n",
                "}\n",
            ),
            None,
        );
        let executable = fixture.project.executable_view().expect("executable HIR");
        let module = executable
            .module(&CanonicalModulePath::crate_root())
            .expect("root module");
        let topology = executable
            .accept_symbol_generation(&fixture.symbols)
            .expect("accepted generation")
            .into_evaluation_topology()
            .expect("topology");
        let module_topology = topology
            .module(module.module_id())
            .expect("module topology");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::ComputationBlock(_)).then_some(owner)
            })
            .expect("implicit callable root");
        let region = module_topology
            .expression_uses()
            .implicit_callable_region(owner, HirPlaceholderKind::PartialApplication)
            .expect("implicit region");
        let local = |name: &str| {
            module
                .locals()
                .find_map(|(owner, local)| (local.name().as_str() == name).then_some(owner))
                .unwrap_or_else(|| panic!("local `{name}`"))
        };
        let use_of = |name: &str, access: CaptureAccess| {
            module
                .expressions()
                .find_map(|(expression, value)| {
                    let HirExprKind::Path(path) = value.kind() else {
                        return None;
                    };
                    (path.as_resolved().and_then(|path| path.lexical_name()) == Some(name)
                        && region.contains_expression(expression)
                        && module_topology
                            .expression_uses()
                            .row(expression)
                            .is_some_and(|row| row.capture_access() == access))
                    .then_some(expression)
                })
                .unwrap_or_else(|| panic!("{access:?} use of `{name}`"))
        };
        let first = local("first");
        let second = local("second");
        let first_use = use_of("first", CaptureAccess::Reassign);
        let second_use = use_of("second", CaptureAccess::Read);
        let checked = CheckedImplicitCallable::seal(
            Arc::clone(&topology),
            owner,
            TypeKind::I64,
            TypeKind::Result {
                ok: Box::new(TypeKind::Unit),
                error: Box::new(TypeKind::Never),
            },
            vec![(first_use, first), (second_use, second)].into_boxed_slice(),
            CheckedExpressionResolution::Structural,
        )
        .expect("sealed implicit callable");

        assert_eq!(
            checked
                .captures()
                .iter()
                .map(CheckedCapture::local)
                .collect::<Vec<_>>(),
            vec![second, first],
        );
        assert_eq!(checked.captures()[0].mode(), CaptureAccess::Read);
        assert_eq!(checked.captures()[1].mode(), CaptureAccess::Reassign);
        assert!(
            checked.capture_uses()[0].source_ordinal() < checked.capture_uses()[1].source_ordinal()
        );

        let mut tampered = checked.clone();
        tampered.capture_uses[0].access = CaptureAccess::Reassign;
        assert_eq!(
            tampered.validate_authority(&topology, owner),
            Err(CheckedCaptureAuthorityViolation::CaptureEvidenceMismatch),
        );
    }
}
