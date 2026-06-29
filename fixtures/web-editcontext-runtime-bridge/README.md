# Web EditContext Runtime Bridge Fixture

This fixture is the intended browser-real acceptance surface for seq06.4a.3:

```text
web/index.html?bundle=fixtures/web-editcontext-runtime-bridge/web-editcontext-runtime-bridge.awfb
startArcweftWebPlayer(...)
```

The `.arcw` source below is a deterministic scene contract for a bundle built by
the normal Arcweft build pipeline.  The zip does not include a prebuilt `.awfb`
because bundle generation must run in the target Arcweft checkout with the same
compiler revision used to apply the overlay.

Controls and stable evidence ids:

| control | target id intent | purpose |
| --- | --- | --- |
| `plain_name` | `target.text.runtime.plain_name` | single-line TextField for Japanese preedit/commit, deletion, replacement, and caret movement |
| `notes` | `target.text.runtime.notes` | multiline TextArea for line start/end, Enter behavior, pointer selection, and scroll geometry refresh |
| `secret_pin` | `target.text.runtime.secret_pin` | SecureField proving text, clipboard, candidate geometry, character bounds, and evidence payload redaction |

Manual browser evidence to capture after applying the overlay:

1. Build this fixture to `web-editcontext-runtime-bridge.awfb`.
2. Serve `web/index.html?bundle=fixtures/web-editcontext-runtime-bridge/web-editcontext-runtime-bridge.awfb` from localhost/HTTPS.
3. Use an EditContext-capable Chromium/Chrome build with Japanese IME enabled.
4. Record `arcweft-text-input-status`, `arcweft-text-input-render`, and `arcweft-text-input-runtime-command` events.
5. Verify candidate window geometry is near the Arcweft-rendered caret and is not at viewport origin.
6. Verify SecureField events expose only redacted lengths/status, not plaintext or character bounds.
