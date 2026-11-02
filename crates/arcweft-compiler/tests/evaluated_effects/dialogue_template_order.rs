use super::{AwbcLowerer, compile_source};
use arcweft_core::awbc::{codec::AwbcDecodeBudget, schema::AwbcProgram};

fn multiple_entry_templates(format_values: bool) {
    let mut source = String::from("pub character narrator { display = \"Narrator\" }\n");
    for name in [
        "main", "zeta", "alpha", "delta", "beta", "gamma", "eta", "theta", "iota",
    ] {
        let format = if format_values {
            format!("let label = fmt(\"{name}\");")
        } else {
            String::new()
        };
        source.push_str(&format!(
            "flow {name}() -> String {{ {format} narrator(id=@say.order.{name}, voice=auto)[{name}[p]]; return \"{name}\" }}\nentry cli @entry.{name} {{ goto @flow.{name} }}\n"
        ));
    }
    let compiled = compile_source(&source).expect("all entry-owned templates compile together");
    let contents = compiled.plan.dialogue_content();
    assert_eq!(contents.rows().len(), 9);
    assert_eq!(
        contents.templates().rows().len(),
        if format_values { 18 } else { 9 }
    );
    compiled
        .plan
        .verify()
        .expect("template references remain valid");
    let program = AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "template-order.arcw",
    )
    .lower()
    .expect("all template identities lower to AWBC")
    .program;
    let bytes = program.encode_canonical().expect("canonical AWBC encodes");
    assert_eq!(
        AwbcProgram::decode_canonical(&bytes, AwbcDecodeBudget::default())
            .expect("all template references decode"),
        program
    );
}

#[test]
fn multiple_entry_dialogue_templates_keep_their_issued_ids() {
    multiple_entry_templates(false);
}

#[test]
fn dialogue_and_format_templates_share_one_ordered_id_catalog() {
    multiple_entry_templates(true);
}
