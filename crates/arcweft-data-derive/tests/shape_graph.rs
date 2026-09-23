use arcweft_data::{
    Bytes, BytesFormat, EnumRepr, EnumTagStyle, FieldShape, Reflect, ShapeGraph, ShapeId, TypeShape,
};
use arcweft_data_derive::ArcweftReflect;

#[derive(ArcweftReflect)]
pub struct Recursive {
    pub children: Vec<Recursive>,
    pub unit: (),
    pub one_tuple: (bool,),
}

#[derive(ArcweftReflect)]
pub struct Left {
    pub right: Option<Right>,
}

#[derive(ArcweftReflect)]
pub struct Right {
    pub left: Vec<Left>,
}

#[derive(ArcweftReflect)]
pub struct Wrapper<T> {
    pub value: T,
}

#[derive(ArcweftReflect)]
pub struct GenericUses {
    pub first: Wrapper<u32>,
    pub repeated: Wrapper<u32>,
    pub text: Wrapper<String>,
}

mod first {
    use arcweft_data_derive::ArcweftReflect;

    #[derive(ArcweftReflect)]
    pub struct Shared {
        pub value: u8,
    }
}

mod second {
    use arcweft_data_derive::ArcweftReflect;

    #[derive(ArcweftReflect)]
    pub struct Shared {
        pub value: bool,
    }
}

#[derive(ArcweftReflect)]
pub struct SameNames {
    pub first: first::Shared,
    pub second: second::Shared,
}

#[derive(ArcweftReflect)]
pub struct NotDefault {
    pub code: u16,
}

#[derive(ArcweftReflect)]
#[arcweft(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WithFieldAttrs {
    #[arcweft(rename = "custom-name", default)]
    pub configured_value: NotDefault,
    #[arcweft(skip)]
    pub local_cache: bool,
    #[arcweft(bytes = "hex")]
    pub payload: Bytes,
}

#[derive(ArcweftReflect)]
#[arcweft(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum Tagged {
    Ready,
    Score(u32),
    Details {
        #[arcweft(default)]
        count: u8,
        #[arcweft(skip)]
        cached: bool,
    },
}

#[derive(ArcweftReflect)]
#[arcweft(repr = "u8", rename_all = "snake_case")]
pub enum Repr {
    First = 1,
    Second = 2,
}

#[derive(ArcweftReflect)]
pub enum TupleArities {
    Unit,
    Zero(),
    One(bool),
}

fn target(shape: &TypeShape) -> ShapeId {
    let TypeShape::Ref(id) = shape else {
        panic!("expected a shape reference, got {shape:?}");
    };
    *id
}

fn record_fields(graph: &ShapeGraph, id: ShapeId) -> &[FieldShape] {
    let Some(TypeShape::Record { fields, .. }) = graph.get(id) else {
        panic!("expected record node {id:?}");
    };
    fields
}

#[test]
fn recursive_and_mutually_recursive_fields_form_complete_graphs() {
    let (graph, root) = Recursive::shape_graph().expect("recursive graph");
    assert_eq!(graph.node_count(), 5);

    let fields = record_fields(&graph, root);
    let children = target(&fields[0].shape);
    let unit = target(&fields[1].shape);
    let one_tuple = target(&fields[2].shape);
    let TypeShape::Seq(child) = graph.get(children).expect("sequence node") else {
        panic!("expected sequence node");
    };
    assert_eq!(target(child), root);
    assert_eq!(graph.get(unit), Some(&TypeShape::Unit));
    let TypeShape::Tuple(items) = graph.get(one_tuple).expect("one-tuple node") else {
        panic!("expected one-tuple node");
    };
    assert_eq!(items.len(), 1);
    assert!(matches!(
        graph.get(target(&items[0])),
        Some(TypeShape::Bool)
    ));

    let (graph, root) = Left::shape_graph().expect("mutually recursive graph");
    assert_eq!(graph.node_count(), 4);
    let option = target(&record_fields(&graph, root)[0].shape);
    let TypeShape::Option(inner) = graph.get(option).expect("option node") else {
        panic!("expected option node");
    };
    let right = target(inner);
    let sequence = target(&record_fields(&graph, right)[0].shape);
    let TypeShape::Seq(inner) = graph.get(sequence).expect("sequence node") else {
        panic!("expected sequence node");
    };
    assert_eq!(target(inner), root);
}

#[test]
fn concrete_generic_arguments_and_declarations_keep_distinct_ids() {
    let (graph, root) = GenericUses::shape_graph().expect("generic graph");
    let fields = record_fields(&graph, root);
    let first = target(&fields[0].shape);
    let repeated = target(&fields[1].shape);
    let text = target(&fields[2].shape);
    assert_eq!(first, repeated);
    assert_ne!(first, text);

    let first_field = record_fields(&graph, first);
    let text_field = record_fields(&graph, text);
    assert_eq!(first_field[0].rust_name, "value");
    assert!(matches!(
        graph.get(target(&first_field[0].shape)),
        Some(TypeShape::U32)
    ));
    assert!(matches!(
        graph.get(target(&text_field[0].shape)),
        Some(TypeShape::String)
    ));

    let (graph, root) = SameNames::shape_graph().expect("same-name declarations graph");
    let fields = record_fields(&graph, root);
    let first = target(&fields[0].shape);
    let second = target(&fields[1].shape);
    assert_ne!(first, second);
    assert_eq!(graph.node_count(), 5);
    assert!(matches!(
        graph.get(target(&record_fields(&graph, first)[0].shape)),
        Some(TypeShape::U8)
    ));
    assert!(matches!(
        graph.get(target(&record_fields(&graph, second)[0].shape)),
        Some(TypeShape::Bool)
    ));
}

#[test]
fn field_and_enum_attributes_survive_graph_registration() {
    let (graph, root) = WithFieldAttrs::shape_graph().expect("field attribute graph");
    let TypeShape::Record { fields, policy, .. } = graph.get(root).expect("root record") else {
        panic!("expected record root");
    };
    assert!(policy.deny_unknown_fields);
    assert_eq!(fields[0].wire_name, "custom-name");
    assert!(fields[0].has_default);
    assert_eq!(fields[1].wire_name, "local-cache");
    assert!(fields[1].skip);
    assert_eq!(fields[2].wire_name, "payload");
    assert_eq!(fields[2].bytes_format, Some(BytesFormat::Hex));

    let (graph, root) = Tagged::shape_graph().expect("tagged enum graph");
    let TypeShape::Enum { variants, tag, .. } = graph.get(root).expect("root enum") else {
        panic!("expected enum root");
    };
    assert_eq!(
        tag,
        &EnumTagStyle::Adjacent {
            tag: "kind".to_owned(),
            content: "value".to_owned(),
        }
    );
    assert_eq!(variants[0].wire_name, "ready");
    assert!(variants[0].payload.is_none());
    assert_eq!(variants[1].wire_name, "score");
    assert!(matches!(variants[1].payload, Some(TypeShape::Ref(_))));
    let Some(TypeShape::Record { fields, .. }) = &variants[2].payload else {
        panic!("expected named payload record");
    };
    assert!(fields[0].has_default);
    assert!(fields[1].skip);

    let (graph, root) = Repr::shape_graph().expect("repr enum graph");
    let TypeShape::Enum { variants, repr, .. } = graph.get(root).expect("repr root") else {
        panic!("expected enum root");
    };
    assert_eq!(*repr, Some(EnumRepr::U8));
    assert_eq!(variants[0].discriminant, Some(1));
    assert_eq!(variants[1].discriminant, Some(2));

    let (graph, root) = TupleArities::shape_graph().expect("tuple arity graph");
    let TypeShape::Enum { variants, .. } = graph.get(root).expect("tuple arity root") else {
        panic!("expected enum root");
    };
    assert!(variants[0].payload.is_none());
    assert_eq!(variants[1].payload, Some(TypeShape::Tuple(Vec::new())));
    assert!(matches!(variants[2].payload, Some(TypeShape::Ref(_))));
}
