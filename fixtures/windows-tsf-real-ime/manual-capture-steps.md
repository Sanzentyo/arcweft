# Manual real Windows IME capture steps

These steps are retained for backend diagnostics only. Final Arcweft acceptance
must move to the seq06.4j player/DSL path described in
`docs/reviews/requests/2026-06-29-seq-06.4j-native-player-platform-text-input-bridge-package.md`.

1. Use a Windows machine with MSVC Rust toolchain and Microsoft Japanese IME.
2. For low-level TSF diagnostics only, run:

   ```bash
   cargo run -p arcweft-player-native --bin windows-tsf-ime-sample -- \
     --trace-out fixtures/windows-tsf-real-ime/microsoft-japanese-ime-hiragana.real.json
   ```

3. If the diagnostic harness shows only a blank window, cannot focus a visible
   Arcweft text field, or anchors candidate UI to the terminal, do not promote
   the trace. That result means the cross-platform native player text-input
   bridge is still missing.
4. In `TextField`, switch to Hiragana input and type `nihongo`, move candidates,
   choose `日本語`, and commit.
5. Move to `TextArea`, type and delete around a committed Japanese phrase.
6. Move to `SecureField`, type the same preedit and commit. Confirm trace shows
   operation kinds and lengths only, not text, native ranges, object ids, or
   character bounds.
7. Close the window to flush the trace.
8. Validate the trace against `schema.json` and add the exact diagnostic output to
   `docs/implementation/seq06-4b-2-windows-tsf-real-ime-window-integration-closure.md`.
