use super::*;
use crate::types::parse_attached_type_for_test;

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

fn lexeme(
    authored: &AuthoredTypeRef,
    steps: &[TypeRefNodeStep],
    kind: TypeRefLexemeKind,
) -> TextRange {
    let owner = path(steps);
    *authored
        .source()
        .lexemes()
        .iter()
        .find(|lexeme| lexeme.owner() == &owner && lexeme.kind() == &kind)
        .expect("fixture has the requested typed lexeme")
        .range()
}

#[test]
fn qualified_constructor_records_exact_terminal_segment() {
    let source = "crate.model.Wrapper<other.Value>";
    let authored = parse_attached_type_for_test(source).expect("qualified generic type parses");

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
    let authored = parse_attached_type_for_test(source).expect("function type parses");

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
    let authored = parse_attached_type_for_test(source).expect("nested type parses");
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
    let authored = parse_attached_type_for_test(source).expect("trait bound parses");
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
    let authored = parse_attached_type_for_test(source).expect("multiline generic parses");
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
fn qualified_turbofish_and_nested_generic_lexemes_are_exact() {
    let source = "pkg::types::Vec::<I32, Option<T>,>";
    let authored = parse_attached_type_for_test(source).expect("qualified turbofish type parses");

    assert_eq!(
        lexeme(
            &authored,
            &[],
            TypeRefLexemeKind::PathSegment { ordinal: 0 }
        ),
        TextRange::new(0, 3)
    );
    assert_eq!(
        lexeme(
            &authored,
            &[],
            TypeRefLexemeKind::PathSeparator { before: 1 }
        ),
        TextRange::new(3, 5)
    );
    assert_eq!(
        lexeme(
            &authored,
            &[],
            TypeRefLexemeKind::PathSegment { ordinal: 2 }
        ),
        TextRange::new(12, 15)
    );
    assert_eq!(
        lexeme(&authored, &[], TypeRefLexemeKind::TurbofishSeparator),
        TextRange::new(15, 17)
    );
    assert_eq!(
        lexeme(&authored, &[], TypeRefLexemeKind::OpenAngle),
        TextRange::new(17, 18)
    );
    assert_eq!(
        lexeme(
            &authored,
            &[],
            TypeRefLexemeKind::ArgumentSeparator { before: 1 }
        ),
        TextRange::new(21, 22)
    );
    assert_eq!(
        lexeme(&authored, &[], TypeRefLexemeKind::TrailingArgumentSeparator),
        TextRange::new(32, 33)
    );
    assert_eq!(
        lexeme(&authored, &[], TypeRefLexemeKind::CloseAngle),
        TextRange::new(33, 34)
    );
    assert_eq!(
        lexeme(
            &authored,
            &[TypeRefNodeStep::GenericArgument(1)],
            TypeRefLexemeKind::OpenAngle
        ),
        TextRange::new(29, 30)
    );
    assert_eq!(
        whole(
            &authored,
            &[
                TypeRefNodeStep::GenericArgument(1),
                TypeRefNodeStep::GenericArgument(0),
            ]
        ),
        TextRange::new(30, 31)
    );
}

#[test]
fn type_source_map_maps_nodes_and_lexemes_together() {
    let authored = parse_attached_type_for_test("Vec<Option<T>>").expect("nested generic parses");
    let mapped = authored
        .source()
        .try_map(|range| Ok::<_, ()>((range.start() + 11, range.end() + 11)))
        .expect("range mapping succeeds");

    assert_eq!(mapped.nodes().len(), authored.source().nodes().len());
    for ((mapped_path, mapped_source), (path, source)) in
        mapped.nodes().iter().zip(authored.source().nodes())
    {
        assert_eq!(mapped_path, path);
        assert_eq!(
            *mapped_source.whole(),
            (source.whole().start() + 11, source.whole().end() + 11)
        );
        assert_eq!(
            mapped_source.head().map(TypeRefHeadSource::kind),
            source.head().map(TypeRefHeadSource::kind)
        );
        assert_eq!(
            mapped_source.head().map(|head| *head.range()),
            source
                .head()
                .map(TypeRefHeadSource::range)
                .map(|range| (range.start() + 11, range.end() + 11))
        );
        assert_eq!(
            mapped_source
                .head()
                .and_then(TypeRefHeadSource::terminal)
                .copied(),
            source
                .head()
                .and_then(TypeRefHeadSource::terminal)
                .map(|range| (range.start() + 11, range.end() + 11))
        );
    }
    for (mapped, source) in mapped.lexemes().iter().zip(authored.source().lexemes()) {
        assert_eq!(mapped.owner(), source.owner());
        assert_eq!(mapped.kind(), source.kind());
        assert_eq!(
            *mapped.range(),
            (source.range().start() + 11, source.range().end() + 11)
        );
    }
}

#[test]
fn type_source_map_rejects_missing_duplicate_and_out_of_order_lexemes() {
    let authored = parse_attached_type_for_test("Vec<T>").expect("generic parses");
    let root = TypeRefNodePath::root();

    let mut missing = authored.source.lexemes.to_vec();
    missing.retain(|lexeme| {
        !(lexeme.owner() == &root && lexeme.kind() == &TypeRefLexemeKind::OpenAngle)
    });
    assert_eq!(
        TypeRefSourceMap::try_new(
            authored.value(),
            authored.source.nodes.to_vec(),
            missing,
            authored.source.components.to_vec(),
        ),
        Err(TypeRefSourceMapError::MissingLexeme {
            owner: root.clone(),
            kind: TypeRefLexemeKind::OpenAngle,
        })
    );

    let mut duplicate = authored.source.lexemes.to_vec();
    duplicate.push(duplicate[0].clone());
    assert_eq!(
        TypeRefSourceMap::try_new(
            authored.value(),
            authored.source.nodes.to_vec(),
            duplicate,
            authored.source.components.to_vec(),
        ),
        Err(TypeRefSourceMapError::DuplicateLexeme {
            owner: root.clone(),
            kind: TypeRefLexemeKind::PathSegment { ordinal: 0 },
        })
    );

    let mut out_of_order = authored.source.lexemes.to_vec();
    out_of_order.swap(0, 1);
    assert_eq!(
        TypeRefSourceMap::try_new(
            authored.value(),
            authored.source.nodes.to_vec(),
            out_of_order,
            authored.source.components.to_vec(),
        ),
        Err(TypeRefSourceMapError::LexemeOutOfOrder {
            owner: root,
            kind: TypeRefLexemeKind::PathSegment { ordinal: 0 },
        })
    );
}

#[test]
fn source_map_constructor_rejects_missing_extra_and_duplicate_paths() {
    let authored = parse_attached_type_for_test("Vec<Missing>").expect("generic parses");
    let mut missing = authored.source.nodes.to_vec();
    let missing_path = path(&[TypeRefNodeStep::GenericArgument(0)]);
    missing.retain(|(candidate, _)| candidate != &missing_path);
    assert_eq!(
        TypeRefSourceMap::try_new(
            authored.value(),
            missing,
            authored.source.lexemes.to_vec(),
            authored.source.components.to_vec(),
        ),
        Err(TypeRefSourceMapError::MissingNode(missing_path.clone()))
    );

    let mut extra = authored.source.nodes.to_vec();
    let extra_path = path(&[TypeRefNodeStep::SliceItem]);
    extra.push((
        extra_path.clone(),
        TypeRefNodeSource::new(TextRange::new(0, 0), None),
    ));
    assert_eq!(
        TypeRefSourceMap::try_new(
            authored.value(),
            extra,
            authored.source.lexemes.to_vec(),
            authored.source.components.to_vec(),
        ),
        Err(TypeRefSourceMapError::ExtraNode(extra_path))
    );

    let mut duplicate = authored.source.nodes.to_vec();
    duplicate.push(duplicate[0].clone());
    assert_eq!(
        TypeRefSourceMap::try_new(
            authored.value(),
            duplicate,
            authored.source.lexemes.to_vec(),
            authored.source.components.to_vec(),
        ),
        Err(TypeRefSourceMapError::DuplicateNode(TypeRefNodePath::root()))
    );
}

#[test]
fn source_map_constructor_rejects_heads_and_children_outside_their_owner() {
    const TEST_ID: &str = "SRC-MAP-MISMATCH";
    let authored = parse_attached_type_for_test("Vec<Missing>").expect("generic parses");

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
        TypeRefSourceMap::try_new(
            authored.value(),
            head_outside,
            authored.source.lexemes.to_vec(),
            authored.source.components.to_vec(),
        ),
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
        TypeRefSourceMap::try_new(
            authored.value(),
            child_outside,
            authored.source.lexemes.to_vec(),
            authored.source.components.to_vec(),
        ),
        Err(TypeRefSourceMapError::ChildOutsideParent(generic_argument)),
        "{TEST_ID}: child structural ranges cannot escape their parent",
    );
}

#[test]
fn type_ref_exact_generic_argument_limit() {
    let exact_arguments = format!(
        "Many<{}>",
        std::iter::repeat_n("T", 256).collect::<Vec<_>>().join(",")
    );
    let authored = parse_attached_type_for_test(&exact_arguments).expect("limit is inclusive");
    assert_eq!(authored.source().nodes().len(), 257);
}

#[test]
fn type_ref_one_over_generic_argument_limit() {
    let too_many_arguments = format!(
        "Many<{}>",
        std::iter::repeat_n("T", 257).collect::<Vec<_>>().join(",")
    );
    assert_eq!(
        parse_attached_type_for_test(&too_many_arguments)
            .expect_err("257 arguments exceed the limit")
            .to_string(),
        "type constructor exceeds the 256 argument limit"
    );
}

#[test]
fn type_ref_exact_type_node_limit() {
    let exact_nodes = format!(
        "({})",
        std::iter::repeat_n("T", 4_095)
            .collect::<Vec<_>>()
            .join(",")
    );
    let authored = parse_attached_type_for_test(&exact_nodes).expect("limit is inclusive");
    assert_eq!(authored.source().nodes().len(), 4_096);
}

#[test]
fn type_ref_one_over_type_node_limit() {
    let too_many_nodes = format!(
        "({})",
        std::iter::repeat_n("T", 4_096)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_eq!(
        parse_attached_type_for_test(&too_many_nodes)
            .expect_err("4097 nodes exceed the limit")
            .to_string(),
        "type exceeds the 4096 node limit"
    );
}
