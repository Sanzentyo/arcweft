# Historical DOM IME sample evidence

Before seq06.4a.4, `web/ime-sample.html` rendered a sample textbox with visible
HTML/CSS:

- `div[role=textbox]`
- committed/composition mirror spans
- CSS `.caret` driven by `--arcweft-caret-*` variables
- visible status/selection/font output cards

That sample is not kept active. It exists only as historical evidence for why the
player-rendered sample replaced it.
