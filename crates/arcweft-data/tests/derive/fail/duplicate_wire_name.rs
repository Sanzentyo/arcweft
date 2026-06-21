use arcweft_data::ArcweftReflect;

#[derive(ArcweftReflect)]
struct Bad {
    #[arcweft(rename = "same")]
    left: String,
    #[arcweft(rename = "same")]
    right: String,
}

fn main() {}
