# HTML / Servo / DOM UI

Game Native UI とは別に、HTML/CSS UI を提供する。

```text
Native:
  Servo WebView backend

Web:
  Browser DOM backend
```

## HtmlUiHost

```rust
pub trait HtmlUiHost {
    fn create_panel(&mut self, spec: HtmlPanelSpec) -> Result<HtmlPanelId>;
    fn destroy_panel(&mut self, id: HtmlPanelId);
    fn set_props(&mut self, id: HtmlPanelId, props: serde_json::Value);
    fn poll_events(&mut self) -> Vec<HtmlUiEvent>;
}
```

## DSL

```arcw
html panel @ui.settings_html from "ui/settings.html" {
    mount = overlay
    z = 100

    props SettingsProps {
        text_speed: f32
        master_volume: f32
    }

    on "close" => GameEvent.Ui(.SettingsClosed)
    on "set-master-volume" payload { value: f32 } =>
        GameEvent.Ui(.SetMasterVolume { value })
}
```

## Native Servo

- `arcweft-ui-servo` に隔離。
- Core は Servo に依存しない。
- wgpu pass へ無理に合成しない。
- 初期は overlay / panel window / separate layer。
- `app://` scheme の bundle resource のみ許可。
- JS bridge は typed message のみ。

## Web DOM

```html
<div id="arcweft-root">
  <canvas id="arcweft-canvas"></canvas>
  <div id="arcweft-html-overlay"></div>
</div>
```

DOM 版は `data-arcweft-entity` / `data-arcweft-action` を使い、Agent 観測に UI tree / bbox / actions を返す。

```html
<button data-arcweft-entity="choice.opening.listen" data-arcweft-action="select">
  聞いてみる
</button>
```

## Headless

headless では実 Servo/DOM がなくても、`HtmlPanelSpec` と UI metadata から仮想 UI tree を返す。


