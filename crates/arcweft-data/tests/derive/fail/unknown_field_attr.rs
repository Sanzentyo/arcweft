use arcweft_data::ArcweftReflect;

#[derive(ArcweftReflect)]
struct Bad {
    #[arcweft(unknown)]
    field: String,
}

fn main() {}
