use arcweft_data::ArcweftEncode;

#[derive(ArcweftEncode)]
enum Bad {
    Pair(u32, u32),
}

fn main() {}
