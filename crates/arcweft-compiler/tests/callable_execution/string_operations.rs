#![expect(
    clippy::unicode_not_nfc,
    reason = "decomposed combining marks must remain distinct Unicode scalars in these fixtures"
)]

use super::*;

#[test]
fn readonly_string_methods_preserve_the_receiver_and_count_unicode_scalars() {
    for (text, expected) in [("  Aé🦀é  ", 5), ("", 0), ("👩‍💻", 3)] {
        let source = format!(
            r#"
entry cli @entry.main {{ goto @flow.main }}
flow main() -> usize {{
    let original: String = "{text}"
    let original_copy = original.to_string()
    let trimmed = original.trim()
    let copied = trimmed.to_string()
    let counter = || copied.scalar_count()
    let count = counter()
    if original != "{text}" {{ return 99usize }}
    if original_copy != original {{ return 97usize }}
    if trimmed != copied {{ return 98usize }}
    if count != copied.scalar_count() {{ return 96usize }}
    return count
}}
"#
        );
        assert_native_return(&source, &expected.to_string());
        assert_awbc_return(&source, RuntimeValue::usize(expected));
    }
}

#[test]
fn string_operations_and_captures_survive_each_awbc_step_public_cold_restore() {
    assert_awbc_return_after_public_cold_restore(
        r#"
entry cli @entry.main { goto @flow.main }
flow main() -> usize {
    let original = "  Aé🦀é  "
    let trimmed = original.trim()
    let copied = trimmed.to_string()
    let counter = || copied.scalar_count()
    let count = counter()
    if original != "  Aé🦀é  " { return 99usize }
    if trimmed != copied { return 98usize }
    return count
}
"#,
        "string-operations.arcw",
        "5",
        &[],
    );
}
#[test]
fn string_scalar_count_rejects_wrong_receivers_arity_and_result_kind() {
    for source in [
        "flow main() -> usize { let value = 42i64; return value.scalar_count() }",
        "flow main() -> usize { let value = [1i64]; return value.scalar_count() }",
        "flow main() -> usize { return \"text\".scalar_count(1usize) }",
        "flow main() -> usize { return \"text\".scalar_count(extra = 1usize) }",
        "flow main() -> u64 { return \"text\".scalar_count() }",
        "flow main() -> usize { return \"text\".len() }",
    ] {
        let source = format!("entry cli @entry.main {{ goto @flow.main }}\n{source}");
        assert!(compile_source(&source).is_err(), "{source}");
    }
}
