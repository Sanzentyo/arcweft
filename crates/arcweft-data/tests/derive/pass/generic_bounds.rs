use std::collections::BTreeMap;
use std::marker::PhantomData;

use arcweft_data::{
    ArcweftDecode, ArcweftEncode, ArcweftReflect, Decode, Encode, Number, Reflect, TypeShape,
    Value,
};

struct Marker;

#[derive(ArcweftEncode, ArcweftDecode)]
struct SkippedMarker<T> {
    id: u32,
    #[arcweft(skip)]
    marker: PhantomData<T>,
}

#[derive(ArcweftReflect)]
struct GenericShape<T> {
    value: T,
}

fn main() {
    let encoded = SkippedMarker::<Marker> {
        id: 7,
        marker: PhantomData,
    }
    .encode()
    .expect("encode");
    let Value::Record(record) = encoded else {
        panic!("expected record");
    };
    assert_eq!(record.len(), 1);
    assert!(record.contains_key("id"));

    let mut source = BTreeMap::new();
    source.insert("id".to_owned(), Value::Number(Number::U(11)));
    let decoded = SkippedMarker::<Marker>::decode(&Value::Record(source)).expect("decode");
    assert_eq!(decoded.id, 11);

    let TypeShape::Record { fields, .. } = GenericShape::<u32>::shape() else {
        panic!("expected record shape");
    };
    assert_eq!(fields[0].rust_name, "value");
}
