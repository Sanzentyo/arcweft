use super::*;
use crate::types::parse_type_ref;

fn path(steps: &[TypeRefNodeStep]) -> TypeRefNodePath {
    TypeRefNodePath(steps.into())
}

fn whole(authored: &AuthoredTypeRef, steps: &[TypeRefNodeStep]) -> TextRange {
    *authored
        .source_at(&path(steps))
        .expect("fixture node has exact source")
        .whole()
}

fn head(authored: &AuthoredTypeRef, steps: &[TypeRefNodeStep]) -> TextRange {
    *authored
        .source_at(&path(steps))
        .and_then(TypeRefNodeSource::head)
        .expect("fixture node has a diagnostic head")
        .range()
}

fn terminal(authored: &AuthoredTypeRef, steps: &[TypeRefNodeStep]) -> TextRange {
    *authored
        .source_at(&path(steps))
        .and_then(TypeRefNodeSource::head)
        .and_then(TypeRefHeadSource::terminal)
        .expect("fixture node has an exact terminal segment")
}

#[test]
fn qualified_constructor_records_exact_terminal_segment() {
    let source = "crate.model.Wrapper<other.Value>";
    let authored = parse_type_ref(source).expect("qualified generic type parses");

    assert_eq!(head(&authored, &[]), TextRange::new(0, 19));
    assert_eq!(terminal(&authored, &[]), TextRange::new(12, 19));
    assert_eq!(
        terminal(&authored, &[TypeRefNodeStep::GenericArgument(0)]),
        TextRange::new(26, 31)
    );

    let mut rebased = authored;
    rebased.rebase(11);
    assert_eq!(terminal(&rebased, &[]), TextRange::new(23, 30));
    assert_eq!(
        terminal(&rebased, &[TypeRefNodeStep::GenericArgument(0)]),
        TextRange::new(37, 42)
    );
}

#[test]
fn repeated_function_type_nodes_keep_distinct_exact_ranges() {
    let source = "Missing -> Missing";
    let authored = parse_type_ref(source).expect("function type parses");

    assert_eq!(whole(&authored, &[]), TextRange::new(0, source.len()));
    assert!(authored.root_source().head().is_none());
    assert_eq!(
        whole(&authored, &[TypeRefNodeStep::FunctionParameter(0)]),
        TextRange::new(0, 7)
    );
    assert_eq!(
        head(&authored, &[TypeRefNodeStep::FunctionParameter(0)]),
        TextRange::new(0, 7)
    );
    assert_eq!(
        whole(&authored, &[TypeRefNodeStep::FunctionReturn]),
        TextRange::new(11, 18)
    );
}

#[test]
fn nested_reference_slice_generic_tuple_maps_every_structural_node() {
    let source = "  &[Option<(Missing, Missing)>]  ";
    let authored = parse_type_ref(source).expect("nested type parses");
    let first_missing = source.find("Missing").expect("first spelling");
    let second_missing = source
        .rfind("Missing")
        .filter(|offset| *offset != first_missing)
        .expect("second spelling");

    assert_eq!(authored.source().nodes().len(), 6);
    assert_eq!(whole(&authored, &[]), TextRange::new(2, source.len() - 2));
    assert_eq!(
        whole(&authored, &[TypeRefNodeStep::ReferenceReferent]),
        TextRange::new(3, source.len() - 2)
    );
    assert_eq!(
        head(
            &authored,
            &[
                TypeRefNodeStep::ReferenceReferent,
                TypeRefNodeStep::SliceItem,
            ],
        ),
        TextRange::new(4, 10)
    );
    assert_eq!(
        whole(
            &authored,
            &[
                TypeRefNodeStep::ReferenceReferent,
                TypeRefNodeStep::SliceItem,
                TypeRefNodeStep::GenericArgument(0),
                TypeRefNodeStep::TupleItem(0),
            ],
        ),
        TextRange::new(first_missing, first_missing + "Missing".len())
    );
    assert_eq!(
        whole(
            &authored,
            &[
                TypeRefNodeStep::ReferenceReferent,
                TypeRefNodeStep::SliceItem,
                TypeRefNodeStep::GenericArgument(0),
                TypeRefNodeStep::TupleItem(1),
            ],
        ),
        TextRange::new(second_missing, second_missing + "Missing".len())
    );

    let TypeRef::Reference(reference) = authored.value() else {
        panic!("fixture root must be a reference")
    };
    assert_eq!(reference.range(), whole(&authored, &[]));
}

#[test]
fn trait_arguments_and_associated_values_keep_independent_paths() {
    let source = "Iterator<Missing, Item = Missing>";
    let authored = parse_type_ref(source).expect("trait bound parses");
    let first_missing = source.find("Missing").expect("first spelling");
    let second_missing = source.rfind("Missing").expect("second spelling");

    assert_eq!(head(&authored, &[]), TextRange::new(0, 8));
    assert_eq!(
        whole(&authored, &[TypeRefNodeStep::TraitArgument(0)]),
        TextRange::new(first_missing, first_missing + 7)
    );
    assert_eq!(
        whole(&authored, &[TypeRefNodeStep::AssociatedBinding(0)]),
        TextRange::new(second_missing, second_missing + 7)
    );
}

#[test]
fn multiline_and_utf8_paths_are_byte_exact() {
    let source = "Result<\n  Missing,\n  名前.Type\n>";
    let authored = parse_type_ref(source).expect("multiline generic parses");
    let utf8_start = source.find("名前.Type").expect("utf8 path");

    assert_eq!(
        head(&authored, &[TypeRefNodeStep::GenericArgument(1)]),
        TextRange::new(utf8_start, utf8_start + "名前.Type".len())
    );
    for (_, node) in authored.source().nodes() {
        assert!(source.is_char_boundary(node.whole().start()));
        assert!(source.is_char_boundary(node.whole().end()));
        if let Some(head) = node.head() {
            assert!(source.is_char_boundary(head.range().start()));
            assert!(source.is_char_boundary(head.range().end()));
        }
    }
}

#[test]
fn source_map_constructor_rejects_missing_extra_and_duplicate_paths() {
    let authored = parse_type_ref("Vec<Missing>").expect("generic parses");
    let mut missing = authored.source.nodes.to_vec();
    let missing_path = path(&[TypeRefNodeStep::GenericArgument(0)]);
    missing.retain(|(candidate, _)| candidate != &missing_path);
    assert_eq!(
        TypeRefSourceMap::try_new(authored.value(), missing),
        Err(TypeRefSourceMapError::MissingNode(missing_path.clone()))
    );

    let mut extra = authored.source.nodes.to_vec();
    let extra_path = path(&[TypeRefNodeStep::SliceItem]);
    extra.push((
        extra_path.clone(),
        TypeRefNodeSource::new(TextRange::new(0, 0), None),
    ));
    assert_eq!(
        TypeRefSourceMap::try_new(authored.value(), extra),
        Err(TypeRefSourceMapError::ExtraNode(extra_path))
    );

    let mut duplicate = authored.source.nodes.to_vec();
    duplicate.push(duplicate[0].clone());
    assert_eq!(
        TypeRefSourceMap::try_new(authored.value(), duplicate),
        Err(TypeRefSourceMapError::DuplicateNode(TypeRefNodePath::root()))
    );
}

#[test]
fn source_map_constructor_rejects_heads_and_children_outside_their_owner() {
    const TEST_ID: &str = "SRC-MAP-MISMATCH";
    let authored = parse_type_ref("Vec<Missing>").expect("generic parses");

    let mut head_outside = authored.source.nodes.to_vec();
    let root = head_outside
        .iter_mut()
        .find(|(path, _)| path == &TypeRefNodePath::root())
        .expect("fixture root source");
    root.1 = TypeRefNodeSource::new(
        TextRange::new(1, 2),
        Some(TypeRefHeadSource::new(
            TypeRefHeadKind::Path,
            TextRange::new(0, 3),
        )),
    );
    assert_eq!(
        TypeRefSourceMap::try_new(authored.value(), head_outside),
        Err(TypeRefSourceMapError::HeadOutsideWhole(
            TypeRefNodePath::root()
        )),
        "{TEST_ID}: a diagnostic head must remain inside its structural node",
    );

    let mut child_outside = authored.source.nodes.to_vec();
    let generic_argument = path(&[TypeRefNodeStep::GenericArgument(0)]);
    let child = child_outside
        .iter_mut()
        .find(|(path, _)| path == &generic_argument)
        .expect("fixture generic argument source");
    child.1 = TypeRefNodeSource::new(TextRange::new(13, 13), None);
    assert_eq!(
        TypeRefSourceMap::try_new(authored.value(), child_outside),
        Err(TypeRefSourceMapError::ChildOutsideParent(generic_argument)),
        "{TEST_ID}: child structural ranges cannot escape their parent",
    );
}

#[test]
fn type_limits_are_inclusive_and_fail_before_source_map_conversion() {
    let exact_arguments = format!(
        "Many<{}>",
        std::iter::repeat_n("T", 256).collect::<Vec<_>>().join(",")
    );
    assert!(parse_type_ref(&exact_arguments).is_ok());
    let too_many_arguments = format!(
        "Many<{}>",
        std::iter::repeat_n("T", 257).collect::<Vec<_>>().join(",")
    );
    assert_eq!(
        parse_type_ref(&too_many_arguments)
            .expect_err("257 arguments exceed the limit")
            .to_string(),
        "type constructor exceeds the 256 argument limit"
    );

    let exact_nodes = format!(
        "({})",
        std::iter::repeat_n("T", 4_095)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert!(parse_type_ref(&exact_nodes).is_ok());
    let too_many_nodes = format!("{exact_nodes} | T");
    assert_eq!(
        parse_type_ref(&too_many_nodes)
            .expect_err("4097 nodes exceed the limit")
            .to_string(),
        "type exceeds the 4096 node limit"
    );
}
