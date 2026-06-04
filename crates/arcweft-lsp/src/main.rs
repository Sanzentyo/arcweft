use arcweft_lsp::{LspConfig, run_stdio};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init()?;
    run_stdio(LspConfig::default())?;
    Ok(())
}
