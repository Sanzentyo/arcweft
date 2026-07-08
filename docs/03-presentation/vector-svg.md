# Vector / SVG

## Vector IR

SVGだけに頼らず、エンジン内の Vector IR を持つ。

```arcw
pub vector @vector.icon.play {
    viewport 0 0 24 24

    path @path.triangle {
        move_to 8 5
        line_to 19 12
        line_to 8 19
        close
    }
    .fill(.current)
}
```

## VectorScene

```rust
pub struct VectorScene {
    pub viewport: Rect,
    pub items: Vec<VectorItem>,
}

pub enum VectorItem {
    Path(PathItem),
    Rect(RectItem),
    RoundedRect(RoundedRectItem),
    Circle(CircleItem),
    Ellipse(EllipseItem),
    Line(LineItem),
    TextOutline(TextOutlineItem),
    Group(GroupItem),
    Use(SymbolRef),
}
```

## Canvas

```arcw
Canvas(size = fill) |ctx| {
    ctx.path {
        move_to(0, 0)
        cubic_to(120, 30, 180, 90, 240, 0)
    }.stroke(.color(.primary), width = 3)
}
```

Canvas は `Props + Theme + LogicalTime -> VectorScene` の純粋関数として扱う。

## SVG import

```arcw
pub svg @svg.icon.settings from "view/icons/settings.svg"
sandbox {
    external_resources = false
    scripting = false
    animation = false
}
theme {
    current_color = theme.color.text
}
```

## Build-time normalize

```text
SVG file
  → sandbox check
  → SVG parse
  → Vector IR normalize
  → bbox / hit region
  → source map
  → bundle cache
```

## Render backend

```rust
pub enum ViewRenderBackend {
    WgpuNative,
    Vello,
    VelloHybrid,
    CpuTinySkia,
    WebDomMetadataOnly,
}
```


