use arcweft_tooling::{format::format_source, model::FormatOptions};

#[test]
fn formatter_preserves_canonical_capability_members_without_synthesizing_policy() {
    let source = r"pub extern capability fs {
    pub type Path

    pub fn read_text(path: Path) -> Need<String, FsError>
        effects { fs.read }

    pub fn write_text(path: Path)(text: String) -> Need<Unit, FsError>
        effects { fs.write }
}
";

    let first = format_source(source, FormatOptions::default()).expect("format succeeds");
    assert!(!first.changed);
    assert_eq!(first.output, source);
    assert!(first.diagnostics.is_empty());

    let second =
        format_source(&first.output, FormatOptions::default()).expect("second format succeeds");
    assert!(!second.changed);
    assert_eq!(second.output, source);
}

#[test]
fn formatter_preserves_unknown_capability_member_bytes() {
    let source = r"extern capability fs {
    /// Unknown member stays lossless for syntax diagnostics.
    #[audit(external)]
    policy legacy { allow = fs.read }
    fn read_text(path: String) -> String effects { fs.read }
}
";

    let report = format_source(source, FormatOptions::default()).expect("format succeeds");
    assert!(!report.changed);
    assert_eq!(report.output, source);
}
