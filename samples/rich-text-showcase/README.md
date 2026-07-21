# Rich Text Showcase

This project demonstrates dialogue rich-text rendering with character-owned
typography and ruby defaults, an authored `RichTextPanel` View, and line-local
rich-text overrides. The `main` launch profile in `arcw.toml` selects that
View; source files do not carry global dialogue presentation defaults.

Check the project from the repository root:

```bash
cargo run -p arcweft-cli -- check --manifest-path samples/rich-text-showcase/arcw.toml --profile main
```
