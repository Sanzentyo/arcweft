use arcweft_lsp::{
    diagnostics::DocumentAnalysis, positions::PositionEncoding, profiles::LspProfile,
};

fn analyze_raw_text(profile: &LspProfile) {
    let _ = DocumentAnalysis::analyze("fn main() {}\n", PositionEncoding::Utf16, profile);
}

fn main() {}
