use arcweft_lang_syntax::parser::parse_source;

fn main() {
    let _ = parse_source("flow demo {}\n");
}
