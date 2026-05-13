use crate::ast::{DialogueToken, EntityRef, LinePlanItem, Stmt};
use crate::expr::{Expr, Literal};
use crate::lower::{HirFlowItem, HirModule};
use crate::symbols::{SymbolUseKind, collect_symbol_uses};
use std::collections::HashMap;
use thiserror::Error;

/// Entity family inferred from an Arcweft public id prefix.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum EntityKind {
    Flow,
    Fragment,
    Choice,
    ChoiceOption,
    Character,
    Textbox,
    DialogueLine,
    Asset,
    Animation,
    Hook,
    Signal,
    Scene,
    Other(String),
}

/// Minimal semantic type used by parser/HIR contract tests.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum TypeKind {
    Bool,
    Int,
    String,
    Duration,
    DisplayText,
    Ref(EntityKind),
    Need {
        ready: Box<TypeKind>,
        error: Box<TypeKind>,
    },
    Named(String),
    Tuple(Vec<TypeKind>),
    Unit,
}

/// Method signature known to the parser-side semantic checker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodSignature {
    return_type: TypeKind,
}

/// Small, explicit environment used to validate that HIR can feed type checking.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TypeCheckEnv {
    symbols: HashMap<String, TypeKind>,
    functions: HashMap<String, TypeKind>,
    methods: HashMap<(TypeKind, String), MethodSignature>,
    indexes: HashMap<TypeKind, TypeKind>,
}

/// Semantic type-checking diagnostic.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckError {
    message: String,
}

/// Syntax-to-HIR readiness error for the future type checker.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct TypeCheckReadinessError {
    message: String,
}

/// Verifies that lowered HIR no longer contains raw expression fragments.
///
/// This is not the type checker. It is the parser/HIR contract check that keeps
/// later name resolution and type checking from silently reparsing source text.
pub fn validate_typecheck_ready(module: &HirModule) -> Result<(), Vec<TypeCheckReadinessError>> {
    let errors = collect_symbol_uses(module)
        .into_iter()
        .filter(|symbol| symbol.kind() == SymbolUseKind::RawExpr)
        .map(|symbol| {
            TypeCheckReadinessError::new(format!(
                "raw expression is not ready for type checking: {}",
                symbol.name()
            ))
        })
        .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Type-checks the lowered HIR with an explicit symbol/method environment.
///
/// This is deliberately small but real: it verifies entity reference families,
/// dialogue callees, awaited `Need<T, E>` values, timed cue durations, and
/// expression symbols without reparsing source text.
pub fn typecheck_hir(module: &HirModule, env: &TypeCheckEnv) -> Result<(), Vec<TypeCheckError>> {
    let mut checker = TypeChecker {
        env,
        errors: Vec::new(),
    };
    checker.check_module(module);
    if checker.errors.is_empty() {
        Ok(())
    } else {
        Err(checker.errors)
    }
}

struct TypeChecker<'a> {
    env: &'a TypeCheckEnv,
    errors: Vec<TypeCheckError>,
}

impl TypeChecker<'_> {
    fn check_module(&mut self, module: &HirModule) {
        if let Err(errors) = validate_typecheck_ready(module) {
            self.errors.extend(
                errors
                    .into_iter()
                    .map(|error| TypeCheckError::new(error.message().to_owned())),
            );
        }

        for flow in module.flows() {
            if let Some(id) = flow.id() {
                self.expect_entity_kind(id, &EntityKind::Flow, "flow id");
            }
            for item in flow.body() {
                self.check_flow_item(item);
            }
        }
        for item in module.top_level_items() {
            self.check_flow_item(item);
        }
    }

    fn check_flow_item(&mut self, item: &HirFlowItem) {
        match item {
            HirFlowItem::Stmt(stmt) => self.check_stmt(stmt),
            HirFlowItem::Dialogue(dialogue) => {
                let callee_type = self.env.symbol_type(dialogue.callee());
                if !is_dialogue_callee_type(callee_type) {
                    self.errors.push(TypeCheckError::new(format!(
                        "dialogue callee `{}` must resolve to Ref<Character> or SpeakerPreset",
                        dialogue.callee()
                    )));
                }
                self.check_dialogue_content(dialogue.content().tokens());
                if let Some(plan) = dialogue.plan() {
                    for item in plan.items() {
                        self.check_line_plan_item(item);
                    }
                }
            }
            HirFlowItem::Choice(choice) => {
                if let Some(id) = choice.id() {
                    self.expect_entity_kind(id, &EntityKind::Choice, "choice id");
                }
                for option in choice.options() {
                    if let Some(id) = option.id() {
                        self.expect_entity_kind(id, &EntityKind::ChoiceOption, "choice option id");
                    }
                    if let Some(condition) = option.condition() {
                        self.expect_expr_type(condition, &TypeKind::Bool, "choice condition");
                    }
                    self.expect_entity_kind(option.target(), &EntityKind::Flow, "choice target");
                }
            }
            HirFlowItem::Include(entity) => {
                let kind = entity_kind(entity);
                if !matches!(kind, Some(EntityKind::Fragment | EntityKind::Flow)) {
                    self.errors.push(TypeCheckError::new(format!(
                        "include target `{}` must be a flow or fragment reference",
                        entity.body()
                    )));
                }
            }
            HirFlowItem::Await { expr, .. } => {
                let ty = self.check_expr(expr);
                if !matches!(ty, Some(TypeKind::Need { .. })) {
                    self.errors.push(TypeCheckError::new(
                        "await expression must have Need<T, E> type".to_owned(),
                    ));
                }
            }
            HirFlowItem::Scenario { .. } => {}
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { expr, .. } | Stmt::Return(expr) | Stmt::Expr(expr) => {
                self.check_expr(expr);
            }
            Stmt::Goto(expr) => {
                self.expect_expr_type(expr, &TypeKind::Ref(EntityKind::Flow), "goto destination");
            }
            Stmt::Raw(raw) => self.errors.push(TypeCheckError::new(format!(
                "raw statement is not type-checkable: {raw}"
            ))),
        }
    }

    fn check_line_plan_item(&mut self, item: &LinePlanItem) {
        match item {
            LinePlanItem::Option { value, .. }
            | LinePlanItem::Let { expr: value, .. }
            | LinePlanItem::Return(value) => {
                self.check_expr(value);
            }
            LinePlanItem::TimedCue { anchor, body } => {
                self.expect_expr_type(anchor, &TypeKind::Duration, "timeline anchor");
                self.check_expr(body);
            }
            LinePlanItem::CancelRule(_)
            | LinePlanItem::StartGroup(_)
            | LinePlanItem::TogetherGroup(_)
            | LinePlanItem::Memo(_)
            | LinePlanItem::Assert(_) => {}
            LinePlanItem::Raw(raw) => self.errors.push(TypeCheckError::new(format!(
                "raw line-plan item is not type-checkable: {raw}"
            ))),
        }
    }

    fn check_dialogue_content(&mut self, tokens: &[DialogueToken]) {
        for token in tokens {
            if let DialogueToken::Expr(expr) = token {
                self.check_expr(expr);
            }
        }
    }

    fn expect_entity_kind(&mut self, entity: &EntityRef, expected: &EntityKind, context: &str) {
        let actual = entity_kind(entity);
        if actual.as_ref() == Some(expected)
            || (expected == &EntityKind::ChoiceOption && actual == Some(EntityKind::Choice))
        {
            return;
        }
        self.errors.push(TypeCheckError::new(format!(
            "{context} `{}` must be a {expected:?} reference",
            entity.body()
        )));
    }

    fn expect_expr_type(&mut self, expr: &Expr, expected: &TypeKind, context: &str) {
        let actual = self.check_expr(expr);
        if actual.as_ref() != Some(expected) {
            self.errors.push(TypeCheckError::new(format!(
                "{context} must have type {expected:?}, found {actual:?}"
            )));
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Option<TypeKind> {
        match expr {
            Expr::Literal(literal) => Some(literal_type(literal)),
            Expr::EntityRef(entity) => entity_kind(entity).map(TypeKind::Ref).or_else(|| {
                self.errors.push(TypeCheckError::new(format!(
                    "unknown entity reference kind: {}",
                    entity.body()
                )));
                None
            }),
            Expr::Path(path) => self.env.symbol_type(path).cloned().or_else(|| {
                self.errors
                    .push(TypeCheckError::new(format!("unknown symbol `{path}`")));
                None
            }),
            Expr::Placeholder(_) => None,
            Expr::Tuple(items) => Some(TypeKind::Tuple(
                items
                    .iter()
                    .filter_map(|item| self.check_expr(item))
                    .collect(),
            )),
            Expr::Call { callee, args } => {
                for arg in args {
                    self.check_expr(arg);
                }
                if let Expr::Path(name) = callee.as_ref() {
                    return self.env.function_type(name).cloned().or_else(|| {
                        self.errors
                            .push(TypeCheckError::new(format!("unknown function `{name}`")));
                        None
                    });
                }
                self.check_expr(callee)
            }
            Expr::NamedArg { value, .. } => self.check_expr(value),
            Expr::MethodCall {
                receiver,
                method,
                args,
            } => {
                let receiver_type = self.check_expr(receiver);
                for arg in args {
                    self.check_expr(arg);
                }
                receiver_type.and_then(|receiver_type| {
                    self.env
                        .method_type(&receiver_type, method)
                        .cloned()
                        .or_else(|| {
                            self.errors.push(TypeCheckError::new(format!(
                                "unknown method `{method}` on {receiver_type:?}"
                            )));
                            None
                        })
                })
            }
            Expr::DialogueCall { callee, .. } => {
                self.check_expr(callee);
                Some(TypeKind::Named("DialogueLine".to_owned()))
            }
            Expr::Index { target, index } => {
                let target_type = self.check_expr(target);
                self.check_expr(index);
                target_type.and_then(|target_type| {
                    self.env.index_type(&target_type).cloned().or_else(|| {
                        self.errors.push(TypeCheckError::new(format!(
                            "type {target_type:?} is not indexable"
                        )));
                        None
                    })
                })
            }
            Expr::Pipe { lhs, rhs } => {
                self.check_expr(lhs);
                self.check_expr(rhs)
            }
            Expr::Binary { lhs, rhs, .. } => {
                self.check_expr(lhs);
                self.check_expr(rhs);
                Some(TypeKind::Bool)
            }
            Expr::Raw(raw) => {
                self.errors.push(TypeCheckError::new(format!(
                    "raw expression is not type-checkable: {raw}"
                )));
                None
            }
        }
    }
}

fn entity_kind(entity: &EntityRef) -> Option<EntityKind> {
    let head = entity.body().split(['.', '@', ':']).next()?;
    Some(match head {
        "flow" => EntityKind::Flow,
        "frag" | "fragment" => EntityKind::Fragment,
        "choice" => EntityKind::Choice,
        "character" => EntityKind::Character,
        "textbox" => EntityKind::Textbox,
        "say" => EntityKind::DialogueLine,
        "asset" => EntityKind::Asset,
        "anim" => EntityKind::Animation,
        "hook" => EntityKind::Hook,
        "signal" => EntityKind::Signal,
        "scene" => EntityKind::Scene,
        "ent" => EntityKind::Other("ent".to_owned()),
        _ => return None,
    })
}

fn literal_type(literal: &Literal) -> TypeKind {
    match literal {
        Literal::String(_) => TypeKind::String,
        Literal::Int(_) => TypeKind::Int,
        Literal::Bool(_) => TypeKind::Bool,
        Literal::Duration { .. } => TypeKind::Duration,
    }
}

fn is_dialogue_callee_type(ty: Option<&TypeKind>) -> bool {
    matches!(ty, Some(TypeKind::Ref(EntityKind::Character)))
        || matches!(ty, Some(TypeKind::Named(name)) if name == "SpeakerPreset")
}

impl TypeCheckReadinessError {
    fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable readiness failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl TypeCheckEnv {
    /// Creates an empty type-checking environment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a variable, constant, or resolved path.
    #[must_use]
    pub fn with_symbol(mut self, name: impl Into<String>, ty: TypeKind) -> Self {
        self.symbols.insert(name.into(), ty);
        self
    }

    /// Registers a free function return type.
    #[must_use]
    pub fn with_function(mut self, name: impl Into<String>, return_type: TypeKind) -> Self {
        self.functions.insert(name.into(), return_type);
        self
    }

    /// Registers a method return type for a receiver type.
    #[must_use]
    pub fn with_method(
        mut self,
        receiver: TypeKind,
        method: impl Into<String>,
        return_type: TypeKind,
    ) -> Self {
        self.methods
            .insert((receiver, method.into()), MethodSignature { return_type });
        self
    }

    /// Registers index result type for a collection-like type.
    #[must_use]
    pub fn with_index(mut self, target: TypeKind, return_type: TypeKind) -> Self {
        self.indexes.insert(target, return_type);
        self
    }

    fn symbol_type(&self, name: &str) -> Option<&TypeKind> {
        self.symbols.get(name)
    }

    fn function_type(&self, name: &str) -> Option<&TypeKind> {
        self.functions.get(name)
    }

    fn method_type(&self, receiver: &TypeKind, method: &str) -> Option<&TypeKind> {
        self.methods
            .get(&(receiver.clone(), method.to_owned()))
            .map(|signature| &signature.return_type)
    }

    fn index_type(&self, target: &TypeKind) -> Option<&TypeKind> {
        self.indexes.get(target)
    }
}

impl TypeCheckError {
    fn new(message: String) -> Self {
        Self { message }
    }

    /// Human-readable type-checking failure.
    pub fn message(&self) -> &str {
        &self.message
    }
}
