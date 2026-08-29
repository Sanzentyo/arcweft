//! Choice expression, candidate, and lifecycle-plan semantic records.
//!
//! Choice candidate control is distinct from ordinary Thread flow. Only
//! statement-only selection and lifecycle handlers use the shared
//! [`HirThreadBody`] owner.

use super::{
    HirExprInvariantError, HirThreadBody, validate_expr, validate_module, validate_optional_expr,
    validate_pattern, validate_scope,
};
use crate::identity::{ExprId, HirModuleId, LocalId, PatternId, ScopeId, StmtId};
use crate::leaf::{HirIdRefValue, HirName};
use crate::stmt::{HirStmtInvariantError, HirTrigger};

/// One Choice expression shared by direct Choice and `let ... = choice ...`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceExpr {
    id: Option<HirIdRefValue>,
    body: HirChoiceBody,
    plan: Option<HirChoicePlan>,
}

impl HirChoiceExpr {
    pub(crate) fn new(
        id: Option<HirIdRefValue>,
        body: HirChoiceBody,
        plan: Option<HirChoicePlan>,
    ) -> Self {
        Self { id, body, plan }
    }

    /// Returns the optional static Choice identity.
    pub const fn id(&self) -> Option<&HirIdRefValue> {
        self.id.as_ref()
    }

    /// Returns the source-ordered candidate body.
    pub const fn body(&self) -> &HirChoiceBody {
        &self.body
    }

    /// Returns the optional lifecycle plan.
    pub const fn plan(&self) -> Option<&HirChoicePlan> {
        self.plan.as_ref()
    }

    /// Returns every Choice-owned required expression slot in canonical
    /// semantic preorder. Authored and recovered values occupy the same slot;
    /// an invalid assignment key retains an explicit unretained slot so later
    /// recovery identities cannot shift.
    #[allow(
        clippy::too_many_lines,
        reason = "the iterative visitor is one closed preorder over the complete Choice payload family"
    )]
    pub(crate) fn required_expression_slots(&self) -> Vec<HirChoiceRequiredExpressionSlot> {
        let mut slots = Vec::new();
        let mut work = Vec::new();
        if let Some(plan) = self.plan() {
            work.push(HirChoiceRequiredExpressionWork::Plan(plan));
        }
        work.push(HirChoiceRequiredExpressionWork::Body(self.body()));

        while let Some(next) = work.pop() {
            match next {
                HirChoiceRequiredExpressionWork::Slot(slot) => slots.push(slot),
                HirChoiceRequiredExpressionWork::Body(body) => {
                    work.extend(
                        body.items()
                            .iter()
                            .rev()
                            .map(HirChoiceRequiredExpressionWork::Item),
                    );
                }
                HirChoiceRequiredExpressionWork::Item(item) => match item {
                    HirChoiceItem::Let(_) | HirChoiceItem::Error => {}
                    HirChoiceItem::If(expression) => {
                        if let Some(body) = expression.else_body() {
                            work.push(HirChoiceRequiredExpressionWork::Body(body));
                        }
                        for branch in expression.branches().iter().rev() {
                            work.push(HirChoiceRequiredExpressionWork::Body(branch.body()));
                            work.push(HirChoiceRequiredExpressionWork::retained(
                                branch.condition(),
                            ));
                        }
                    }
                    HirChoiceItem::For(expression) => {
                        work.push(HirChoiceRequiredExpressionWork::Body(expression.body()));
                        work.push(HirChoiceRequiredExpressionWork::retained(
                            expression.source(),
                        ));
                    }
                    HirChoiceItem::Match(expression) => {
                        for arm in expression.arms().iter().rev() {
                            work.push(HirChoiceRequiredExpressionWork::Body(arm.body()));
                        }
                        work.push(HirChoiceRequiredExpressionWork::retained(
                            expression.scrutinee(),
                        ));
                    }
                    HirChoiceItem::Option(expression) => {
                        work.push(HirChoiceRequiredExpressionWork::OptionBody(
                            expression.body(),
                        ));
                        work.push(HirChoiceRequiredExpressionWork::retained(expression.id()));
                    }
                    HirChoiceItem::OptionFor(expression) => {
                        work.push(HirChoiceRequiredExpressionWork::OptionBody(
                            expression.body(),
                        ));
                        work.push(HirChoiceRequiredExpressionWork::retained(
                            expression.source(),
                        ));
                    }
                    HirChoiceItem::CompactArm(expression) => {
                        if let HirChoiceCompactAction::Out(value) = expression.action() {
                            work.push(HirChoiceRequiredExpressionWork::retained(*value));
                        }
                        if let Some(condition) = expression.condition() {
                            work.push(HirChoiceRequiredExpressionWork::retained(condition));
                        }
                        work.push(HirChoiceRequiredExpressionWork::retained(
                            expression.label(),
                        ));
                    }
                },
                HirChoiceRequiredExpressionWork::OptionBody(body) => {
                    for field in body.fields().iter().rev() {
                        match field {
                            HirChoiceOptionField::Label { value, .. }
                            | HirChoiceOptionField::Id(value)
                            | HirChoiceOptionField::Value(value)
                            | HirChoiceOptionField::Visible(value)
                            | HirChoiceOptionField::Enabled(value)
                            | HirChoiceOptionField::Order(value)
                            | HirChoiceOptionField::Hotkey(value) => {
                                work.push(HirChoiceRequiredExpressionWork::retained(*value));
                            }
                            HirChoiceOptionField::View(view) => {
                                for entry in view.entries().iter().rev() {
                                    work.push(HirChoiceRequiredExpressionWork::retained(
                                        entry.value(),
                                    ));
                                    work.push(HirChoiceRequiredExpressionWork::retained(
                                        entry.key(),
                                    ));
                                }
                            }
                            HirChoiceOptionField::Select(_)
                            | HirChoiceOptionField::Let(_)
                            | HirChoiceOptionField::Error => {}
                        }
                    }
                }
                HirChoiceRequiredExpressionWork::Plan(plan) => {
                    work.extend(
                        plan.items()
                            .iter()
                            .rev()
                            .map(HirChoiceRequiredExpressionWork::PlanItem),
                    );
                }
                HirChoiceRequiredExpressionWork::PlanItem(item) => match item {
                    HirChoicePlanItem::Assignment { value, .. } => {
                        work.push(HirChoiceRequiredExpressionWork::retained(*value));
                    }
                    HirChoicePlanItem::Timeout { duration, .. } => {
                        work.push(HirChoiceRequiredExpressionWork::retained(*duration));
                    }
                    HirChoicePlanItem::Cancel { trigger, .. } => {
                        work.push(HirChoiceRequiredExpressionWork::Trigger(trigger));
                    }
                    HirChoicePlanItem::OnSelect { .. }
                    | HirChoicePlanItem::Error(HirChoicePlanError::RecoveredSyntax) => {}
                    HirChoicePlanItem::Error(HirChoicePlanError::InvalidAssignmentKey) => {
                        work.push(HirChoiceRequiredExpressionWork::Slot(
                            HirChoiceRequiredExpressionSlot::UnretainedInvalidAssignmentValue,
                        ));
                    }
                },
                HirChoiceRequiredExpressionWork::Trigger(trigger) => match trigger {
                    HirTrigger::Signal { target, .. } | HirTrigger::Timeout(target) => {
                        work.push(HirChoiceRequiredExpressionWork::retained(*target));
                    }
                    HirTrigger::Input(_)
                    | HirTrigger::Event(_)
                    | HirTrigger::Mark(_)
                    | HirTrigger::Select(_)
                    | HirTrigger::Task(_)
                    | HirTrigger::Scope(_)
                    | HirTrigger::Expression(_)
                    | HirTrigger::Recovered(_) => {}
                },
            }
        }
        slots
    }

    pub(super) fn validate_module(
        &self,
        expected: HirModuleId,
    ) -> Result<(), HirExprInvariantError> {
        self.body.validate_module(expected)?;
        if let Some(plan) = &self.plan {
            plan.validate_module(expected)?;
        }
        Ok(())
    }

    pub(super) fn has_recovery(&self) -> bool {
        self.id.as_ref().is_some_and(HirIdRefValue::is_recovered)
            || self.body.has_recovery()
            || self.plan.as_ref().is_some_and(HirChoicePlan::has_recovery)
    }

    pub(super) fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.body
            .thread_body_for_scope(scope)
            .or_else(|| self.plan.as_ref()?.thread_body_for_scope(scope))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum HirChoiceRequiredExpressionSlot {
    Retained(ExprId),
    UnretainedInvalidAssignmentValue,
}

enum HirChoiceRequiredExpressionWork<'choice> {
    Slot(HirChoiceRequiredExpressionSlot),
    Body(&'choice HirChoiceBody),
    Item(&'choice HirChoiceItem),
    OptionBody(&'choice HirChoiceOptionBody),
    Plan(&'choice HirChoicePlan),
    PlanItem(&'choice HirChoicePlanItem),
    Trigger(&'choice HirTrigger),
}

impl HirChoiceRequiredExpressionWork<'_> {
    const fn retained(expression: ExprId) -> Self {
        Self::Slot(HirChoiceRequiredExpressionSlot::Retained(expression))
    }
}

/// One lexical, source-ordered Choice candidate body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceBody {
    scope: ScopeId,
    items: Box<[HirChoiceItem]>,
}

impl HirChoiceBody {
    pub(crate) const fn new(scope: ScopeId, items: Box<[HirChoiceItem]>) -> Self {
        Self { scope, items }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn items(&self) -> &[HirChoiceItem] {
        &self.items
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_scope(expected, self.scope)?;
        for item in &self.items {
            item.validate_module(expected)?;
        }
        Ok(())
    }

    fn has_recovery(&self) -> bool {
        self.items.iter().any(HirChoiceItem::has_recovery)
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.items
            .iter()
            .find_map(|item| item.thread_body_for_scope(scope))
    }
}

/// Closed semantic child inventory of a Choice candidate body.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirChoiceItem {
    Let(StmtId),
    If(HirChoiceIf),
    For(HirChoiceFor),
    Match(HirChoiceMatch),
    Option(HirChoiceOption),
    OptionFor(HirChoiceOptionFor),
    CompactArm(HirChoiceCompactArm),
    /// A recovered source row with no executable semantic child.
    Error,
}

impl HirChoiceItem {
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        match self {
            Self::Let(statement) => validate_module(expected, statement.module()),
            Self::If(value) => value.validate_module(expected),
            Self::For(value) => value.validate_module(expected),
            Self::Match(value) => value.validate_module(expected),
            Self::Option(value) => value.validate_module(expected),
            Self::OptionFor(value) => value.validate_module(expected),
            Self::CompactArm(value) => value.validate_module(expected),
            Self::Error => Ok(()),
        }
    }

    fn has_recovery(&self) -> bool {
        match self {
            Self::If(value) => value.has_recovery(),
            Self::For(value) => value.body().has_recovery(),
            Self::Match(value) => value.has_recovery(),
            Self::Option(value) => value.body().has_recovery(),
            Self::OptionFor(value) => value.body().has_recovery(),
            Self::CompactArm(value) => value.has_recovery(),
            Self::Error => true,
            Self::Let(_) => false,
        }
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        match self {
            Self::If(value) => value.thread_body_for_scope(scope),
            Self::For(value) => value.body().thread_body_for_scope(scope),
            Self::Match(value) => value.thread_body_for_scope(scope),
            Self::Option(value) => value.body().thread_body_for_scope(scope),
            Self::OptionFor(value) => value.body().thread_body_for_scope(scope),
            Self::Let(_) | Self::CompactArm(_) | Self::Error => None,
        }
    }
}

/// Flat, stack-safe Choice conditional candidate gate.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceIf {
    branches: Box<[HirChoiceIfBranch]>,
    else_body: Option<HirChoiceBody>,
}

impl HirChoiceIf {
    pub(crate) const fn new(
        branches: Box<[HirChoiceIfBranch]>,
        else_body: Option<HirChoiceBody>,
    ) -> Self {
        Self {
            branches,
            else_body,
        }
    }

    pub fn branches(&self) -> &[HirChoiceIfBranch] {
        &self.branches
    }

    pub const fn else_body(&self) -> Option<&HirChoiceBody> {
        self.else_body.as_ref()
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        for branch in &self.branches {
            branch.validate_module(expected)?;
        }
        if let Some(body) = &self.else_body {
            body.validate_module(expected)?;
        }
        Ok(())
    }

    fn has_recovery(&self) -> bool {
        self.branches
            .iter()
            .any(|branch| branch.body().has_recovery())
            || self
                .else_body
                .as_ref()
                .is_some_and(HirChoiceBody::has_recovery)
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.branches
            .iter()
            .find_map(|branch| branch.body().thread_body_for_scope(scope))
            .or_else(|| self.else_body.as_ref()?.thread_body_for_scope(scope))
    }
}

/// One ordered branch of a Choice `if` chain.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceIfBranch {
    condition: ExprId,
    body: HirChoiceBody,
}

impl HirChoiceIfBranch {
    pub(crate) const fn new(condition: ExprId, body: HirChoiceBody) -> Self {
        Self { condition, body }
    }

    pub const fn condition(&self) -> ExprId {
        self.condition
    }

    pub const fn body(&self) -> &HirChoiceBody {
        &self.body
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_expr(expected, self.condition)?;
        self.body.validate_module(expected)
    }
}

/// Dynamic candidate loop and its pattern bindings.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceFor {
    pattern: PatternId,
    source: ExprId,
    body: HirChoiceBody,
    locals: Box<[LocalId]>,
}

impl HirChoiceFor {
    pub(crate) const fn new(
        pattern: PatternId,
        source: ExprId,
        body: HirChoiceBody,
        locals: Box<[LocalId]>,
    ) -> Self {
        Self {
            pattern,
            source,
            body,
            locals,
        }
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn source(&self) -> ExprId {
        self.source
    }

    pub const fn body(&self) -> &HirChoiceBody {
        &self.body
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_pattern(expected, self.pattern)?;
        validate_expr(expected, self.source)?;
        self.body.validate_module(expected)?;
        validate_locals(expected, &self.locals)
    }
}

/// Choice-specific Match candidate control.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceMatch {
    scrutinee: ExprId,
    arms: Box<[HirChoiceMatchArm]>,
}

impl HirChoiceMatch {
    pub(crate) const fn new(scrutinee: ExprId, arms: Box<[HirChoiceMatchArm]>) -> Self {
        Self { scrutinee, arms }
    }

    pub const fn scrutinee(&self) -> ExprId {
        self.scrutinee
    }

    pub fn arms(&self) -> &[HirChoiceMatchArm] {
        &self.arms
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_expr(expected, self.scrutinee)?;
        for arm in &self.arms {
            arm.validate_module(expected)?;
        }
        Ok(())
    }

    fn has_recovery(&self) -> bool {
        self.arms.iter().any(|arm| arm.body().has_recovery())
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.arms
            .iter()
            .find_map(|arm| arm.body().thread_body_for_scope(scope))
    }
}

/// One Choice Match arm with an isolated candidate scope.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceMatchArm {
    pattern: PatternId,
    guard: Option<ExprId>,
    body: HirChoiceBody,
    locals: Box<[LocalId]>,
}

impl HirChoiceMatchArm {
    pub(crate) const fn new(
        pattern: PatternId,
        guard: Option<ExprId>,
        body: HirChoiceBody,
        locals: Box<[LocalId]>,
    ) -> Self {
        Self {
            pattern,
            guard,
            body,
            locals,
        }
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn guard(&self) -> Option<ExprId> {
        self.guard
    }

    pub const fn body(&self) -> &HirChoiceBody {
        &self.body
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_pattern(expected, self.pattern)?;
        validate_optional_expr(expected, self.guard)?;
        self.body.validate_module(expected)?;
        validate_locals(expected, &self.locals)
    }
}

/// One full Choice option with a dynamic or static expression identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceOption {
    id: ExprId,
    body: HirChoiceOptionBody,
}

impl HirChoiceOption {
    pub(crate) const fn new(id: ExprId, body: HirChoiceOptionBody) -> Self {
        Self { id, body }
    }

    pub const fn id(&self) -> ExprId {
        self.id
    }

    pub const fn body(&self) -> &HirChoiceOptionBody {
        &self.body
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_expr(expected, self.id)?;
        self.body.validate_module(expected)
    }
}

/// Pattern-expanded option candidates.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceOptionFor {
    pattern: PatternId,
    source: ExprId,
    body: HirChoiceOptionBody,
    locals: Box<[LocalId]>,
}

impl HirChoiceOptionFor {
    pub(crate) const fn new(
        pattern: PatternId,
        source: ExprId,
        body: HirChoiceOptionBody,
        locals: Box<[LocalId]>,
    ) -> Self {
        Self {
            pattern,
            source,
            body,
            locals,
        }
    }

    pub const fn pattern(&self) -> PatternId {
        self.pattern
    }

    pub const fn source(&self) -> ExprId {
        self.source
    }

    pub const fn body(&self) -> &HirChoiceOptionBody {
        &self.body
    }

    pub fn locals(&self) -> &[LocalId] {
        &self.locals
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_pattern(expected, self.pattern)?;
        validate_expr(expected, self.source)?;
        self.body.validate_module(expected)?;
        validate_locals(expected, &self.locals)
    }
}

/// Lexical, ordered field body of one full option.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceOptionBody {
    scope: ScopeId,
    fields: Box<[HirChoiceOptionField]>,
}

impl HirChoiceOptionBody {
    pub(crate) const fn new(scope: ScopeId, fields: Box<[HirChoiceOptionField]>) -> Self {
        Self { scope, fields }
    }

    pub const fn scope(&self) -> ScopeId {
        self.scope
    }

    pub fn fields(&self) -> &[HirChoiceOptionField] {
        &self.fields
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_scope(expected, self.scope)?;
        for field in &self.fields {
            field.validate_module(expected)?;
        }
        Ok(())
    }

    fn has_recovery(&self) -> bool {
        self.fields.iter().any(HirChoiceOptionField::has_recovery)
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.fields
            .iter()
            .find_map(|field| field.thread_body_for_scope(scope))
    }
}

/// Closed semantic field inventory of one full option.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirChoiceOptionField {
    Label {
        text_key: Option<HirIdRefValue>,
        value: ExprId,
    },
    Id(ExprId),
    Value(ExprId),
    Visible(ExprId),
    Enabled(ExprId),
    Order(ExprId),
    Hotkey(ExprId),
    View(HirChoiceView),
    Select(HirThreadBody),
    Let(StmtId),
    /// A recovered field with no executable semantic child.
    Error,
}

impl HirChoiceOptionField {
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        match self {
            Self::Label { value, .. }
            | Self::Id(value)
            | Self::Value(value)
            | Self::Visible(value)
            | Self::Enabled(value)
            | Self::Order(value)
            | Self::Hotkey(value) => validate_expr(expected, *value),
            Self::View(value) => value.validate_module(expected),
            Self::Select(body) => validate_thread_body(expected, body),
            Self::Let(statement) => validate_module(expected, statement.module()),
            Self::Error => Ok(()),
        }
    }

    fn has_recovery(&self) -> bool {
        match self {
            Self::Label { text_key, .. } => {
                text_key.as_ref().is_some_and(HirIdRefValue::is_recovered)
            }
            Self::Error => true,
            Self::Id(_)
            | Self::Value(_)
            | Self::Visible(_)
            | Self::Enabled(_)
            | Self::Order(_)
            | Self::Hotkey(_)
            | Self::View(_)
            | Self::Select(_)
            | Self::Let(_) => false,
        }
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        match self {
            Self::Select(body) if body.scope() == scope => Some(body),
            _ => None,
        }
    }
}

/// Open-ended, source-ordered option View projection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceView {
    entries: Box<[HirChoiceViewEntry]>,
}

impl HirChoiceView {
    pub(crate) const fn new(entries: Box<[HirChoiceViewEntry]>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[HirChoiceViewEntry] {
        &self.entries
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        for entry in &self.entries {
            entry.validate_module(expected)?;
        }
        Ok(())
    }
}

/// One typed key/value relation in an option View projection.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceViewEntry {
    key: ExprId,
    value: ExprId,
}

impl HirChoiceViewEntry {
    pub(crate) const fn new(key: ExprId, value: ExprId) -> Self {
        Self { key, value }
    }

    pub const fn key(&self) -> ExprId {
        self.key
    }

    pub const fn value(&self) -> ExprId {
        self.value
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_expr(expected, self.key)?;
        validate_expr(expected, self.value)
    }
}

/// One compact static option arm.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoiceCompactArm {
    id: HirIdRefValue,
    label: ExprId,
    condition: Option<ExprId>,
    action: HirChoiceCompactAction,
}

impl HirChoiceCompactArm {
    pub(crate) const fn new(
        id: HirIdRefValue,
        label: ExprId,
        condition: Option<ExprId>,
        action: HirChoiceCompactAction,
    ) -> Self {
        Self {
            id,
            label,
            condition,
            action,
        }
    }

    pub const fn id(&self) -> &HirIdRefValue {
        &self.id
    }

    pub const fn label(&self) -> ExprId {
        self.label
    }

    pub const fn condition(&self) -> Option<ExprId> {
        self.condition
    }

    pub const fn action(&self) -> &HirChoiceCompactAction {
        &self.action
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        validate_expr(expected, self.label)?;
        validate_optional_expr(expected, self.condition)?;
        self.action.validate_module(expected)
    }

    fn has_recovery(&self) -> bool {
        self.id.is_recovered() || self.action.has_recovery()
    }
}

/// Compact-arm action after syntax sugar has been typed.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirChoiceCompactAction {
    Goto(HirIdRefValue),
    Out(ExprId),
    /// A missing required action retained for outer Choice poison.
    Missing,
}

impl HirChoiceCompactAction {
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        match self {
            Self::Out(value) => validate_expr(expected, *value),
            Self::Goto(_) | Self::Missing => Ok(()),
        }
    }

    fn has_recovery(&self) -> bool {
        match self {
            Self::Goto(target) => target.is_recovered(),
            Self::Missing => true,
            Self::Out(_) => false,
        }
    }
}

/// Ordered Choice lifecycle plan.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HirChoicePlan {
    items: Box<[HirChoicePlanItem]>,
}

impl HirChoicePlan {
    pub(crate) const fn new(items: Box<[HirChoicePlanItem]>) -> Self {
        Self { items }
    }

    pub fn items(&self) -> &[HirChoicePlanItem] {
        &self.items
    }

    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        for item in &self.items {
            item.validate_module(expected)?;
        }
        Ok(())
    }

    fn has_recovery(&self) -> bool {
        self.items.iter().any(HirChoicePlanItem::has_recovery)
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        self.items
            .iter()
            .find_map(|item| item.thread_body_for_scope(scope))
    }
}

/// Closed semantic item inventory of a Choice lifecycle plan.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirChoicePlanItem {
    Assignment {
        key: HirName,
        value: ExprId,
    },
    Timeout {
        duration: ExprId,
        body: HirThreadBody,
    },
    Cancel {
        trigger: HirTrigger,
        body: HirThreadBody,
    },
    OnSelect {
        pattern: PatternId,
        locals: Box<[LocalId]>,
        body: HirThreadBody,
    },
    /// A recovered plan row with no executable semantic child.
    Error(HirChoicePlanError),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HirChoicePlanError {
    RecoveredSyntax,
    InvalidAssignmentKey,
}

impl HirChoicePlanItem {
    fn validate_module(&self, expected: HirModuleId) -> Result<(), HirExprInvariantError> {
        match self {
            Self::Assignment { value, .. } => validate_expr(expected, *value),
            Self::Timeout { duration, body } => {
                validate_expr(expected, *duration)?;
                validate_thread_body(expected, body)
            }
            Self::Cancel { trigger, body } => {
                trigger
                    .validate_module(expected)
                    .map_err(|error| match error {
                        HirStmtInvariantError::ForeignChild { expected, actual } => {
                            HirExprInvariantError::ForeignChild { expected, actual }
                        }
                        HirStmtInvariantError::Thread(_)
                        | HirStmtInvariantError::InvalidPoisonState => {
                            unreachable!("trigger validation only checks qualified child modules")
                        }
                    })?;
                validate_thread_body(expected, body)
            }
            Self::OnSelect {
                pattern,
                locals,
                body,
            } => {
                validate_pattern(expected, *pattern)?;
                validate_locals(expected, locals)?;
                validate_thread_body(expected, body)
            }
            Self::Error(_) => Ok(()),
        }
    }

    const fn has_recovery(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    fn thread_body_for_scope(&self, scope: ScopeId) -> Option<&HirThreadBody> {
        match self {
            Self::Timeout { body, .. }
            | Self::Cancel { body, .. }
            | Self::OnSelect { body, .. }
                if body.scope() == scope =>
            {
                Some(body)
            }
            _ => None,
        }
    }
}

fn validate_locals(expected: HirModuleId, locals: &[LocalId]) -> Result<(), HirExprInvariantError> {
    for local in locals {
        validate_module(expected, local.module())?;
    }
    Ok(())
}

fn validate_thread_body(
    expected: HirModuleId,
    body: &HirThreadBody,
) -> Result<(), HirExprInvariantError> {
    body.validate_module(expected)
        .map_err(|actual| HirExprInvariantError::ForeignChild { expected, actual })
}

#[cfg(test)]
#[path = "choice/tests.rs"]
mod tests;
