use arcweft_data::ArcweftReflect;

#[derive(ArcweftReflect)]
#[arcweft(content = "payload")]
enum Bad {
    A,
}

fn main() {}
