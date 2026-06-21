use arcweft_data::ArcweftReflect;

#[derive(ArcweftReflect)]
#[arcweft(rename_all = "scream_case")]
struct Bad {
    field: String,
}

fn main() {}
