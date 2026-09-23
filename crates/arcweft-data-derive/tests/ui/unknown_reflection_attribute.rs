use arcweft_data_derive::ArcweftReflect;

#[derive(ArcweftReflect)]
struct Invalid {
    #[arcweft(guess_default)]
    value: u32,
}

fn main() {}
