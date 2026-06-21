use arcweft_data::ArcweftEncode;

#[derive(ArcweftEncode)]
#[arcweft(tag = "kind")]
enum Bad {
    Payload(u32),
}

fn main() {}
