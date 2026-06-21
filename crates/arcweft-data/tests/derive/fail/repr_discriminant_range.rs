use arcweft_data::ArcweftReflect;

#[derive(ArcweftReflect)]
#[arcweft(repr = "u8")]
enum Bad {
    TooBig = 300,
}

fn main() {}
