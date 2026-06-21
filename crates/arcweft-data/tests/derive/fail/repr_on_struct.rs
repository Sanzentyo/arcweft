use arcweft_data::ArcweftReflect;

#[derive(ArcweftReflect)]
#[arcweft(repr = "u8")]
struct Bad {
    field: String,
}

fn main() {}
