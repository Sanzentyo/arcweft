# seq06.9c compositing capture fixtures

`scene.css` covers `filter`, `backdrop-filter`, `mask`, `clip-path`, and
`mix-blend-mode` with stable Arcweft fixture ids. `expected-evidence.json` is the
reviewable JSON packet used before any exact PNG promotion.

The exact PNG lane remains ignored/manual because compositor output can be GPU
and driver sensitive. The JSON packet is intended to be the stable CI evidence
surface.
