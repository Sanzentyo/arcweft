use arcweft_data::{ArcweftReflect, Bytes, BytesFormat, Reflect, TypeShape};

#[derive(ArcweftReflect)]
struct Asset {
    #[arcweft(bytes)]
    blob: Bytes,
}

fn main() {
    let TypeShape::Record { fields, .. } = Asset::shape() else {
        panic!("expected record shape");
    };
    assert_eq!(fields[0].bytes_format, Some(BytesFormat::Binary));
}
