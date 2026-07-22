use arcweft_lang_syntax::expr::{Expr, TryExpr, parse_expr};

fn main() {
    let source = match parse_expr("value?").unwrap() {
        Expr::Try(parsed) => parsed.source(),
        _ => unreachable!(),
    };
    let operand = Expr::Path("value".into());
    let _try_expr = TryExpr::new(Box::new(operand), source);
}
