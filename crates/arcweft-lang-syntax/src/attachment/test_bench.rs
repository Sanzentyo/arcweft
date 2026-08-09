//! Attached `test` and `bench` declaration owners.

use crate::expressions::ExpressionProjection;
use crate::grammar::kinds::{SyntaxKind, SyntaxRole, SyntaxRoleClass};
use crate::grammar::test_projection::{KnownTestKind, PendingTestKindProjection};
use crate::id_ref::SyntaxIdRefSyntax;
use crate::name::SyntaxName;

use super::family::{ExpressionFamily, FamilyNode};
use super::node::{
    BenchItemKind, BlockKind, ErrorNodeKind, MissingBodyKind, MissingExpressionKind,
    MissingNameKind, NameReferenceKind, TestItemKind,
};
use super::{
    AstNode, AttachedExpressionNode, AttachedItemPrefix, StatementNode, SyntaxAccessError,
    SyntaxNodeHandle, TypedItemNode,
};

/// Typed entity-reference header owned by one test or benchmark plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedPlanId {
    Authored(Box<AttachedExpressionNode>),
    Missing(AstNode<MissingExpressionKind>),
}

impl AttachedPlanId {
    pub fn value(&self) -> Option<&SyntaxIdRefSyntax> {
        match self {
            Self::Authored(expression) => match expression.projection() {
                ExpressionProjection::EntityReference(value) => Some(value),
                _ => None,
            },
            Self::Missing(_) => None,
        }
    }

    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Authored(expression) => expression.syntax().syntax(),
            Self::Missing(missing) => missing.syntax(),
        }
    }

    pub fn has_recovery(&self) -> bool {
        match self {
            Self::Authored(expression) => match expression.projection() {
                ExpressionProjection::EntityReference(value) => value.value().is_err(),
                _ => true,
            },
            Self::Missing(_) => true,
        }
    }
}

/// Parser-selected test adapter kind bound to its exact syntax identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedTestKind {
    Scenario(AstNode<NameReferenceKind>),
    Visual(AstNode<NameReferenceKind>),
    Audio(AstNode<NameReferenceKind>),
    Fixture(AstNode<NameReferenceKind>),
    Custom {
        syntax: AstNode<NameReferenceKind>,
        value: SyntaxName,
    },
    Missing(AstNode<MissingNameKind>),
}

impl AttachedTestKind {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Scenario(syntax)
            | Self::Visual(syntax)
            | Self::Audio(syntax)
            | Self::Fixture(syntax)
            | Self::Custom { syntax, .. } => syntax.syntax(),
            Self::Missing(syntax) => syntax.syntax(),
        }
    }

    pub const fn custom_name(&self) -> Option<&SyntaxName> {
        match self {
            Self::Custom { value, .. } => Some(value),
            _ => None,
        }
    }

    pub const fn has_recovery(&self) -> bool {
        matches!(self, Self::Missing(_))
    }
}

/// Statement-only body of one test or benchmark plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttachedPlanBody {
    Missing(AstNode<MissingBodyKind>),
    Braced {
        syntax: AstNode<BlockKind>,
        statements: Box<[StatementNode]>,
        closed: bool,
    },
}

impl AttachedPlanBody {
    pub fn syntax(&self) -> SyntaxNodeHandle {
        match self {
            Self::Missing(syntax) => syntax.syntax(),
            Self::Braced { syntax, .. } => syntax.syntax(),
        }
    }

    pub const fn block(&self) -> Option<&AstNode<BlockKind>> {
        match self {
            Self::Braced { syntax, .. } => Some(syntax),
            Self::Missing(_) => None,
        }
    }

    pub const fn statements(&self) -> &[StatementNode] {
        match self {
            Self::Missing(_) => &[],
            Self::Braced { statements, .. } => statements,
        }
    }

    pub const fn is_closed(&self) -> bool {
        matches!(self, Self::Braced { closed: true, .. })
    }

    pub const fn has_recovery(&self) -> bool {
        !self.is_closed()
    }
}

/// Complete attached `test` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedTestDeclaration {
    syntax: AstNode<TestItemKind>,
    prefix: AttachedItemPrefix,
    id: AttachedPlanId,
    kind: AttachedTestKind,
    body: AttachedPlanBody,
    trailing_recoveries: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedTestDeclaration {
    pub const fn syntax(&self) -> &AstNode<TestItemKind> {
        &self.syntax
    }
    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }
    pub const fn id(&self) -> &AttachedPlanId {
        &self.id
    }
    pub const fn kind(&self) -> &AttachedTestKind {
        &self.kind
    }
    pub const fn body(&self) -> &AttachedPlanBody {
        &self.body
    }
    pub const fn trailing_recoveries(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recoveries
    }
    pub fn has_recovery(&self) -> bool {
        self.id.has_recovery()
            || self.kind.has_recovery()
            || self.body.has_recovery()
            || !self.trailing_recoveries.is_empty()
    }
}

/// Complete attached `bench` declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachedBenchDeclaration {
    syntax: AstNode<BenchItemKind>,
    prefix: AttachedItemPrefix,
    id: AttachedPlanId,
    body: AttachedPlanBody,
    trailing_recoveries: Box<[AstNode<ErrorNodeKind>]>,
}

impl AttachedBenchDeclaration {
    pub const fn syntax(&self) -> &AstNode<BenchItemKind> {
        &self.syntax
    }
    pub const fn prefix(&self) -> &AttachedItemPrefix {
        &self.prefix
    }
    pub const fn id(&self) -> &AttachedPlanId {
        &self.id
    }
    pub const fn body(&self) -> &AttachedPlanBody {
        &self.body
    }
    pub const fn trailing_recoveries(&self) -> &[AstNode<ErrorNodeKind>] {
        &self.trailing_recoveries
    }
    pub fn has_recovery(&self) -> bool {
        self.id.has_recovery() || self.body.has_recovery() || !self.trailing_recoveries.is_empty()
    }
}

impl AstNode<TestItemKind> {
    /// Binds the parser-owned test projection to exact snapshot descendants.
    pub fn semantics(&self) -> Result<AttachedTestDeclaration, SyntaxAccessError> {
        let pending = self
            .syntax()
            .test_kind_projection()
            .cloned()
            .ok_or(SyntaxAccessError::MissingTestKindProjection { id: self.id() })?;
        Ok(AttachedTestDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::Test(self.clone()).attached_prefix()?,
            id: attached_plan_id(&self.syntax())?,
            kind: attached_test_kind(self, pending)?,
            body: attached_plan_body(&self.syntax())?,
            trailing_recoveries: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

impl AstNode<BenchItemKind> {
    /// Attaches a benchmark plan without introducing a second parser reader.
    pub fn semantics(&self) -> Result<AttachedBenchDeclaration, SyntaxAccessError> {
        Ok(AttachedBenchDeclaration {
            syntax: self.clone(),
            prefix: TypedItemNode::Bench(self.clone()).attached_prefix()?,
            id: attached_plan_id(&self.syntax())?,
            body: attached_plan_body(&self.syntax())?,
            trailing_recoveries: self
                .ordered_exact_children::<ErrorNodeKind>(SyntaxRoleClass::Recovery)?
                .into_boxed_slice(),
        })
    }
}

fn attached_plan_id(owner: &SyntaxNodeHandle) -> Result<AttachedPlanId, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(SyntaxRole::Reference(0))?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
    match syntax.kind() {
        SyntaxKind::EntityReferenceExpression => {
            let expression = FamilyNode::<ExpressionFamily>::new(syntax)?.semantic()?;
            if !matches!(
                expression.projection(),
                ExpressionProjection::EntityReference(_)
            ) {
                return Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() });
            }
            Ok(AttachedPlanId::Authored(Box::new(expression)))
        }
        SyntaxKind::MissingExpression => Ok(AttachedPlanId::Missing(syntax.cast()?)),
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

fn attached_test_kind(
    owner: &AstNode<TestItemKind>,
    pending: PendingTestKindProjection,
) -> Result<AttachedTestKind, SyntaxAccessError> {
    let syntax = owner
        .syntax()
        .optional_unique_child(SyntaxRole::Kind)?
        .ok_or(SyntaxAccessError::InvalidTestKindProjection { id: owner.id() })?;
    match pending {
        PendingTestKindProjection::Known { value, source }
            if syntax.kind() == SyntaxKind::NameReference && syntax.range() == source =>
        {
            let syntax = syntax.cast::<NameReferenceKind>()?;
            Ok(match value {
                KnownTestKind::Scenario => AttachedTestKind::Scenario(syntax),
                KnownTestKind::Visual => AttachedTestKind::Visual(syntax),
                KnownTestKind::Audio => AttachedTestKind::Audio(syntax),
                KnownTestKind::Fixture => AttachedTestKind::Fixture(syntax),
            })
        }
        PendingTestKindProjection::Custom { value, source }
            if syntax.kind() == SyntaxKind::NameReference && syntax.range() == source =>
        {
            Ok(AttachedTestKind::Custom {
                syntax: syntax.cast()?,
                value,
            })
        }
        PendingTestKindProjection::Missing { insertion }
            if syntax.kind() == SyntaxKind::MissingName && syntax.range() == insertion =>
        {
            Ok(AttachedTestKind::Missing(syntax.cast()?))
        }
        _ => Err(SyntaxAccessError::InvalidTestKindProjection { id: owner.id() }),
    }
}

fn attached_plan_body(owner: &SyntaxNodeHandle) -> Result<AttachedPlanBody, SyntaxAccessError> {
    let syntax = owner
        .optional_unique_child(SyntaxRole::Body)?
        .ok_or(SyntaxAccessError::InvalidItemProjection { id: owner.id() })?;
    match syntax.kind() {
        SyntaxKind::MissingBody => Ok(AttachedPlanBody::Missing(syntax.cast()?)),
        SyntaxKind::Block => {
            let block = syntax.cast::<BlockKind>()?;
            let _ = block.open_delimiter()?;
            let close = block.close_delimiter()?;
            if block.optional_tail()?.is_some() {
                return Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() });
            }
            let statements = block.statements()?.into_boxed_slice();
            Ok(AttachedPlanBody::Braced {
                syntax: block,
                statements,
                closed: !close.range().is_empty(),
            })
        }
        _ => Err(SyntaxAccessError::InvalidItemProjection { id: owner.id() }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::num::NonZeroU64;
    use std::sync::Arc;

    use arcweft_source::identity::SourceSnapshotId;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{
        AstNode, AttachedPlanBody, AttachedPlanId, AttachedTestKind, BenchItemKind, TestItemKind,
    };
    use crate::attachment::node::{ExpressionStatementKind, GotoStatementKind};
    use crate::attachment::{
        GrammarIdentityMap, RequiredStatementExpressionNode, SyntaxDatabaseId, SyntaxLineageId,
        SyntaxNodeId, SyntaxSnapshotData, SyntaxSnapshotId, attach_typed_tree,
    };
    use crate::expressions::ExpressionProjection;
    use crate::grammar::kinds::SyntaxKind;
    use crate::parser::{ParseOptions, parse_document};

    fn attach(text: &str) -> Arc<SyntaxSnapshotData> {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new("arcw:/test-bench-attachment-test").unwrap(),
                SourceName::path("test-bench-attachment-test.arcw"),
                text,
            )
            .unwrap(),
        );
        let build = parse_document(&document, ParseOptions::default()).unwrap();
        let database = SyntaxDatabaseId::from_raw_for_test(NonZeroU64::new(173).unwrap());
        let lineage = SyntaxLineageId::from_raw_for_test(database, NonZeroU64::new(1).unwrap());
        let snapshot = SyntaxSnapshotId::new(
            lineage,
            SourceSnapshotId::initial(document.display_name().clone()),
        );
        let identities = build
            .index()
            .entries()
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                (
                    entry.path().clone(),
                    SyntaxNodeId::new(
                        lineage,
                        NonZeroU64::new(u64::try_from(index).unwrap() + 1).unwrap(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        attach_typed_tree(
            &build,
            &GrammarIdentityMap::new(identities),
            snapshot,
            document,
        )
        .unwrap()
    }

    fn tests(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<TestItemKind>> {
        snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::TestItem)
            .map(|node| node.cast().unwrap())
            .collect()
    }

    fn benches(snapshot: &Arc<SyntaxSnapshotData>) -> Vec<AstNode<BenchItemKind>> {
        snapshot
            .nodes()
            .filter(|node| node.kind() == SyntaxKind::BenchItem)
            .map(|node| node.cast().unwrap())
            .collect()
    }

    #[test]
    fn test_and_bench_attachment_preserves_typed_id_kind_and_statement_roles() {
        let snapshot = attach(concat!(
            "/// Scenario plan\n",
            "#[tool.fixture]\n",
            "test @test.scenario scenario {\n",
            "    goto @flow.opening\n",
            "    true\n",
            "}\n",
            "test @test.custom headless {}\n",
            "bench @bench.score {\n",
            "    setup { true }\n",
            "    measure { false }\n",
            "    report { true }\n",
            "}\n",
        ));
        let declarations = tests(&snapshot);
        let scenario = declarations[0].semantics().unwrap();
        assert_eq!(
            scenario.prefix().documentation().unwrap().markdown(),
            "Scenario plan"
        );
        assert_eq!(scenario.prefix().attributes().len(), 1);
        assert!(matches!(scenario.id(), AttachedPlanId::Authored(_)));
        let id = scenario.id().value().unwrap().value().unwrap();
        assert_eq!(
            id.segments()
                .iter()
                .map(crate::id_ref::AuthoredIdSegment::as_str)
                .collect::<Vec<_>>(),
            ["test", "scenario"]
        );
        assert!(matches!(scenario.kind(), AttachedTestKind::Scenario(_)));
        let AttachedPlanBody::Braced {
            statements, closed, ..
        } = scenario.body()
        else {
            panic!("authored plan body must remain braced")
        };
        assert!(*closed);
        assert_eq!(statements.len(), 2);
        let goto = statements[0].cast::<GotoStatementKind>().unwrap();
        let goto = goto.semantics().unwrap();
        let RequiredStatementExpressionNode::Expression(target) = goto.target() else {
            panic!("authored Test Goto target")
        };
        assert!(matches!(
            target.semantic().unwrap().projection(),
            ExpressionProjection::EntityReference(_)
        ));
        let expression = statements[1].cast::<ExpressionStatementKind>().unwrap();
        assert!(matches!(
            expression
                .expression()
                .unwrap()
                .semantic()
                .unwrap()
                .projection(),
            ExpressionProjection::Literal(_)
        ));

        let custom = declarations[1].semantics().unwrap();
        assert!(matches!(
            custom.kind(),
            AttachedTestKind::Custom { value, .. } if value.as_str() == "headless"
        ));
        let bench = benches(&snapshot)[0].semantics().unwrap();
        assert!(matches!(bench.id(), AttachedPlanId::Authored(_)));
        assert_eq!(bench.body().statements().len(), 3);
        for statement in bench.body().statements() {
            let expression = statement.cast::<ExpressionStatementKind>().unwrap();
            let semantic = expression.expression().unwrap().semantic().unwrap();
            assert!(
                matches!(semantic.projection(), ExpressionProjection::NamedBlock(_)),
                "{:?}",
                semantic.projection()
            );
        }
        assert!(!scenario.has_recovery());
        assert!(!custom.has_recovery());
        assert!(!bench.has_recovery());
    }

    #[test]
    fn test_and_bench_attachment_retains_missing_and_unclosed_recovery() {
        let snapshot = attach(concat!(
            "test scenario {}\n",
            "test @test.no_kind {}\n",
            "test @test.no_body scenario\n",
            "bench {}\n",
            "bench @bench.no_body\n",
            "test @test.unclosed scenario { true\n",
        ));
        let declarations = tests(&snapshot);
        let missing_id = declarations[0].semantics().unwrap();
        assert!(matches!(missing_id.id(), AttachedPlanId::Missing(_)));
        assert!(matches!(missing_id.kind(), AttachedTestKind::Scenario(_)));

        let missing_kind = declarations[1].semantics().unwrap();
        assert!(matches!(missing_kind.kind(), AttachedTestKind::Missing(_)));
        assert!(matches!(
            declarations[2].semantics().unwrap().body(),
            AttachedPlanBody::Missing(_)
        ));
        let unclosed = declarations[3].semantics().unwrap();
        assert!(matches!(
            unclosed.body(),
            AttachedPlanBody::Braced { closed: false, .. }
        ));

        let benches = benches(&snapshot);
        assert!(matches!(
            benches[0].semantics().unwrap().id(),
            AttachedPlanId::Missing(_)
        ));
        assert!(matches!(
            benches[1].semantics().unwrap().body(),
            AttachedPlanBody::Missing(_)
        ));
    }
}
