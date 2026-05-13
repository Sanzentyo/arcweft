# LSP

## 基本機能

- diagnostics
- hover
- completion
- go to definition
- find references
- semantic tokens
- inlay hints
- code actions
- formatter
- rename
- prepare rename

## DSL固有

- ID inference / materialize / rename
- Ref resolution
- Sugar expansion code actions:
  - `with:` → `with { ... }`
  - `speaker:` → `speaker.say()[...]`
  - `speaker(args):` → `speaker.say(args)[...]`
  - `await? expr with ...` → `try await expr with ...`
  - `parent::path` → `super::path`
- ID code actions:
  - materialize `id=.suffix` as a normalized `#say...` ID
  - materialize `choice .suffix` and relative option IDs as normalized `#choice...` IDs
  - keep `.suffix` relative IDs by default during formatting
- `Need` unhandled diagnostics
- naked await in flow diagnostics
- borrow crosses await diagnostics
- Option/Result unhandled diagnostics
- contract hover / counterexample display
- parser preview
- shader diagnostics
- audio cue / signal / BGM completion
- UI component preview

## Custom requests

```text
arcweft/getNodeAtPosition
arcweft/getGraphSlice
arcweft/getNodeHistory
arcweft/previewGraphPatch
arcweft/applyGraphPatch
arcweft/getRagContext
arcweft/renderRouteMap
arcweft/parseInput
arcweft/expandSugar
arcweft/shaderPreview
arcweft/audioCuePreview
```

## Agent-oriented JSON

CLI/LSP は machine-readable diagnostics を出す。

```json
{
  "code": "E_PENDING_UNHANDLED",
  "message": "Need<ImageHandle, AssetError> must be awaited with pending branch",
  "span": { "file": "game/routes/opening.awft", "start": 120, "end": 155 },
  "suggestions": [
    { "title": "Wrap with await-with pending branch" }
  ]
}
```
