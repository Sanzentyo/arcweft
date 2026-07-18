use arcweft_lang_syntax::{
    ast::common::TextRange,
    parser::recovery::{ParseError, ParseErrorKind},
};

fn main() {
    let _generic = ParseError::new(
        TextRange::new(0, 0),
        Vec::new(),
        None,
        String::new(),
        Vec::new(),
    );
    let _typed = ParseError::new_with_kind(
        ParseErrorKind::Generic,
        TextRange::new(0, 0),
        Vec::new(),
        None,
        String::new(),
        Vec::new(),
    );
    let _literal = ParseError {
        kind: ParseErrorKind::Generic,
        range: TextRange::new(0, 0),
        expected: Vec::new(),
        found: None,
        message: String::new(),
        recovery: Vec::new(),
    };
}
