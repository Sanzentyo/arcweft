use super::*;

fn array_type(item: TypeKind, length: usize) -> TypeKind {
    TypeKind::Array {
        item: Box::new(item),
        len: crate::types::ArrayLength::Const(length),
    }
}

#[test]
fn bracket_sequences_follow_array_and_vec_contexts() {
    let fixture = fixture(
        r#"
fn fixed_numeric() -> Array<i32, 3> {
    let values: Array<i32, 3> = [1i32, 2i32, 3i32]
    values
}
fn fixed_general() -> Array<String, 2> { ["north", "south"] }
fn dynamic_numeric() -> Vec<i32> { [4i32, 5i32] }
fn inferred_numeric() { let values = [6i32, 7i32] }
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("bracket sequences follow their collection context");
    let numeric_array = array_type(TypeKind::I32, 3);
    let general_array = array_type(TypeKind::String, 2);
    let vector = TypeKind::Vec(Box::new(TypeKind::I32));

    assert!(
        analysis
            .expressions()
            .any(|(_, expression)| expression.value_type() == Some(&numeric_array))
    );
    assert!(
        analysis
            .expressions()
            .any(|(_, expression)| expression.value_type() == Some(&general_array))
    );
    assert!(
        analysis
            .expressions()
            .filter(|(_, expression)| expression.value_type() == Some(&vector))
            .count()
            >= 2
    );
}

#[test]
fn bracket_sequences_reject_array_length_mismatches() {
    for (source, description) in [
        (
            "fn invalid() -> Array<i32, 2> { [1i32, 2i32, 3i32] }\n",
            "numeric sequence",
        ),
        (
            "fn invalid() -> Array<String, 2> { [\"north\", \"south\", \"east\"] }\n",
            "general sequence",
        ),
    ] {
        assert!(
            analyze(&fixture(source, None)).is_err(),
            "{description} must not satisfy an Array with a different length"
        );
    }
}

#[test]
fn compact_numeric_elements_fit_the_selected_integer_type() {
    let accepted = fixture(
        "fn unsigned() -> Vec<u8> { [255u8] }\nfn signed() -> Vec<i8> { [127i8] }\nfn wide() -> Vec<u128> { [340282366920938463463374607431768211455u128] }\n",
        None,
    );
    analyze(&accepted).expect("exact integer maxima fit their selected item types");

    for (source, item) in [
        ("fn rejected() -> Vec<u8> { [256u8] }\n", TypeKind::U8),
        ("fn rejected() -> Vec<i8> { [128i8] }\n", TypeKind::I8),
        (
            "fn rejected() -> Vec<u128> { [340282366920938463463374607431768211456u128] }\n",
            TypeKind::U128,
        ),
    ] {
        assert!(matches!(
            analyze(&fixture(source, None)),
            Err(FinalSemanticAnalysisError::CompactNumericElementOutOfRange {
                ordinal: 0,
                item: actual,
                ..
            }) if *actual == item
        ));
    }
}

#[test]
fn array_repeats_preserve_constant_lengths_and_contextual_items() {
    let fixture = fixture(
        r#"
fn repeated_numeric() -> Array<i32, 4> {
    let values: Array<i32, 4> = [0i32; 4i64]
    values
}
fn repeated_general() -> Array<String, 2> { ["echo"; 2u64] }
fn repeated_inferred() { let values = [7i32; 3u64] }
"#,
        None,
    );
    let analysis = analyze(&fixture).expect("integer array-repeat lengths are retained");

    for expected in [
        array_type(TypeKind::I32, 4),
        array_type(TypeKind::String, 2),
        array_type(TypeKind::I32, 3),
    ] {
        assert!(
            analysis
                .expressions()
                .any(|(_, expression)| expression.value_type() == Some(&expected))
        );
    }
}

#[test]
fn array_repeats_reject_length_mismatches_and_runtime_lengths() {
    for (source, description) in [
        (
            "fn invalid() -> Array<i32, 3> { [0i32; 4i64] }\n",
            "numeric repeat",
        ),
        (
            "fn invalid() -> Array<String, 2> { [\"echo\"; 3u64] }\n",
            "general repeat",
        ),
        (
            "fn invalid(length: usize) -> Array<i32, 4> { [0i32; length] }\n",
            "runtime repeat length",
        ),
    ] {
        assert!(
            analyze(&fixture(source, None)).is_err(),
            "{description} must not satisfy a fixed compile-time Array shape"
        );
    }
}
