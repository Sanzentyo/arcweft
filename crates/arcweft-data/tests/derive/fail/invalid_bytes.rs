use arcweft_data::{ArcweftReflect, Bytes};

#[derive(ArcweftReflect)]
struct Bad {
    #[arcweft(bytes = "gzip")]
    blob: Bytes,
}

fn main() {}
