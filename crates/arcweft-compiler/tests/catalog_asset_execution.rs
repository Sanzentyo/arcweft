#[path = "support/execution.rs"]
mod execution;

use arcweft_core::value::RuntimeValue;

fn source(reference: &str, selected: bool) -> String {
    format!(
        "entry cli @entry.main {{ goto @flow.main }}\nfn choose(flag: bool) -> Ref<Asset> {{ match flag {{ true => {reference}, false => @asset:.bg.room }} }}\nflow main() -> String {{ return choose({selected}).id }}"
    )
}

#[test]
fn catalog_asset_references_execute_through_ordinary_native_and_decoded_awbc_calls() {
    for reference in ["@asset:.bg.pulse", "@asset.bg.pulse"] {
        for (selected, expected) in [(true, "asset.bg.pulse"), (false, "asset.bg.room")] {
            let source = source(reference, selected);
            execution::assert_native_return(&source, expected);
            execution::assert_awbc_return(&source, RuntimeValue::String(expected.to_owned()));
        }
    }
}

#[test]
fn constant_type_arguments_execute_through_ordinary_native_and_decoded_awbc_calls() {
    let source = "entry cli @entry.main { goto @flow.main }\nfn pair() -> Array<String, 2> { [\"left\", \"right\"] }\nflow main() -> String { let values = pair(); return values[1usize] }";
    execution::assert_native_return(source, "right");
    execution::assert_awbc_return(source, RuntimeValue::String("right".to_owned()));
}

#[test]
fn concrete_asset_projection_survives_generic_callable_type_instantiation_in_native_and_awbc() {
    for marker in ["7i64", "\"marker\""] {
        for (selected, expected) in [(true, "asset.bg.pulse"), (false, "asset.bg.room")] {
            let source = format!(
                "entry cli @entry.main {{ goto @flow.main }}\nfn choose<T>(flag: bool, marker: T) -> Ref<Asset> {{ match flag {{ true => @asset.bg.pulse, false => @asset.bg.room }} }}\nflow main() -> String {{ return choose({selected}, {marker}).id }}"
            );
            execution::assert_native_return(&source, expected);
            execution::assert_awbc_return(&source, RuntimeValue::String(expected.to_owned()));
        }
    }
}
