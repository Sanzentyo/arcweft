//! Type checking for the closed `Fx` constructor namespace.

use arcweft_lang_hir::fx::FxConstructorKind;
use arcweft_lang_syntax::expr::{CallArg, Expr};

use super::super::{TypeCheckError, TypeChecker, TypeKind};

impl TypeChecker<'_> {
    pub(super) fn check_fx_constructor_call(
        &mut self,
        callee: &Expr,
        args: &[CallArg],
    ) -> Option<TypeKind> {
        let path = callee.dotted_selector_label()?;
        let member = path.strip_prefix("Fx.")?;
        if member.contains('.') {
            return None;
        }
        let Some(kind) = FxConstructorKind::from_member(member) else {
            self.errors.push(TypeCheckError::new(format!(
                "unknown Fx constructor `Fx.{member}`"
            )));
            for arg in args {
                self.check_expr(arg.value());
            }
            return Some(TypeKind::Named("Fx".to_owned()));
        };
        match kind {
            FxConstructorKind::Stack => self.check_fx_stack_args(args),
            FxConstructorKind::Conditional => self.check_fx_conditional_args(args),
            FxConstructorKind::Shader => self.check_fx_shader_args(args),
            _ => {
                for arg in args {
                    match arg {
                        CallArg::Named { name, value } => {
                            self.check_fx_property_value(name, value);
                        }
                        CallArg::Positional(_) | CallArg::Spread { .. } => {
                            self.errors.push(TypeCheckError::new(format!(
                                "`Fx.{member}` accepts named arguments only"
                            )));
                            self.check_expr(arg.value());
                        }
                    }
                }
            }
        }
        Some(TypeKind::Named("Fx".to_owned()))
    }

    fn check_fx_stack_args(&mut self, args: &[CallArg]) {
        let [CallArg::Positional(value)] = args else {
            self.errors.push(TypeCheckError::new(
                "`Fx.stack` requires one positional ordered Fx list".to_owned(),
            ));
            for arg in args {
                self.check_expr(arg.value());
            }
            return;
        };
        let Expr::BracketSeq(children) = value.as_ref() else {
            self.errors.push(TypeCheckError::new(
                "`Fx.stack` requires one positional ordered Fx list".to_owned(),
            ));
            return;
        };
        let expected = TypeKind::Named("Fx".to_owned());
        for child in children {
            self.expect_expr_type(child, &expected, "Fx.stack child");
        }
    }

    fn check_fx_conditional_args(&mut self, args: &[CallArg]) {
        let mut condition = false;
        let mut then_graph = false;
        let mut else_graph = false;
        for arg in args {
            match arg {
                CallArg::Named { name, value } if name == "condition" => {
                    condition = true;
                    self.expect_expr_type(value, &TypeKind::Bool, "Fx.conditional condition");
                }
                CallArg::Named { name, value } if matches!(name.as_str(), "then" | "else") => {
                    then_graph |= name == "then";
                    else_graph |= name == "else";
                    self.expect_expr_type(
                        value,
                        &TypeKind::Named("Fx".to_owned()),
                        "Fx.conditional branch",
                    );
                }
                CallArg::Named { name, value } => {
                    self.errors.push(TypeCheckError::new(format!(
                        "`Fx.conditional` has no argument named `{name}`"
                    )));
                    self.check_expr(value);
                }
                CallArg::Positional(_) | CallArg::Spread { .. } => {
                    self.errors.push(TypeCheckError::new(
                        "`Fx.conditional` accepts named arguments only".to_owned(),
                    ));
                    self.check_expr(arg.value());
                }
            }
        }
        for (present, name) in [
            (condition, "condition"),
            (then_graph, "then"),
            (else_graph, "else"),
        ] {
            if !present {
                self.errors.push(TypeCheckError::new(format!(
                    "`Fx.conditional` requires `{name} = ...`"
                )));
            }
        }
    }

    fn check_fx_shader_args(&mut self, args: &[CallArg]) {
        for (index, arg) in args.iter().enumerate() {
            match arg {
                CallArg::Positional(value) if index == 0 => {
                    self.check_expr(value);
                }
                CallArg::Named { name, value } => self.check_fx_property_value(name, value),
                CallArg::Positional(_) | CallArg::Spread { .. } => {
                    self.errors.push(TypeCheckError::new(
                        "`Fx.shader` accepts only its leading resource positionally".to_owned(),
                    ));
                    self.check_expr(arg.value());
                }
            }
        }
    }

    fn check_fx_property_value(&mut self, name: &str, value: &Expr) {
        if matches!(value, Expr::ShortVariant(_)) {
            return;
        }
        if name == "sample" {
            let expected = TypeKind::function(
                [TypeKind::Named("FxSampleContext".to_owned())],
                TypeKind::Named("Transform2D".to_owned()),
            );
            self.check_expr_with_expected(value, Some(&expected));
        } else if matches!(name, "color" | "tint" | "outline_color") {
            self.check_expr_with_expected(value, Some(&TypeKind::Named("Color".to_owned())));
        } else {
            self.check_expr(value);
        }
    }
}
