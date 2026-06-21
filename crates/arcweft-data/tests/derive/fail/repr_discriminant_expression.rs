use arcweft_data::ArcweftReflect;

const VALUE: isize = 1;

#[derive(ArcweftReflect)]
#[arcweft(repr = "i8")]
enum Bad {
    Computed = VALUE,
}

fn main() {}
