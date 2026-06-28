# Manual real Windows IME capture steps

1. Apply the overlay to `Sanzentyo/arcweft`.
2. Use a Windows machine with MSVC Rust toolchain and Microsoft Japanese IME.
3. Run:

   ```bash
   cargo run -p arcweft-player-native --bin windows-tsf-ime-sample -- \
     --trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.real.json
   ```

4. In `TextField`, switch to Hiragana input and type `nihongo`, move candidates,
   choose `日本語`, and commit.
5. Move to `TextArea`, type and delete around a committed Japanese phrase.
6. Move to `SecureField`, type the same preedit and commit. Confirm trace shows
   operation kinds and lengths only, not text, native ranges, object ids, or
   character bounds.
7. Close the window to flush the trace.
8. Validate the trace against `schema.json` and add the exact command output to
   `docs/implementation/seq06-4b-2-windows-tsf-real-ime-window-integration-closure.md`.
