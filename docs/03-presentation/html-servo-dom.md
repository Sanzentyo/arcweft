# HTML / Servo / DOM View

Game Native View とは別に、HTML/CSS View を提供する。

```text
Native:
  Servo WebView backend

Web:
  Browser DOM backend
```

## Host boundary

Native and Web provide host adapters for HTML panels. Core stays independent
of Servo and browser DOM; panel placement, input policy, and Agent projection
follow the shared [Layer System](layers.md) contract.

## DSL

```arcw
html panel @view.settings_html from "view/settings.html" {
    mount = overlay
    z = 100

    props SettingsProps {
        text_speed: f32
        master_volume: f32
    }

    on "close" => GameEvent.View(.SettingsClosed)
    on "set-master-volume" payload { value: f32 } =>
        GameEvent.View(.SetMasterVolume { value })
}
```

## Native Servo

- `arcweft-view-servo` に隔離。
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

DOM 版は `data-arcweft-entity` / `data-arcweft-action` を使い、Agent 観測に View tree / bbox / actions を返す。

```html
<button data-arcweft-entity="choice.opening.listen" data-arcweft-action="select">
  聞いてみる
</button>
```

## Headless

headless では実 Servo/DOM がなくても、`HtmlPanelSpec` と View metadata から仮想 View tree を返す。
