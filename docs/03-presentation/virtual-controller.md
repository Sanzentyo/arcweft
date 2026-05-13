# Virtual Touch Controller

This chapter is kept as a compatibility entry point for older Arcweft design notes.

The canonical design is now:

- [Touch Virtual Controller](touch-virtual-controller.md)
- [Layer System / Input Routing](layers.md)
- [Layered input runtime](../02-runtime/layered-input.md)
- [Virtual Controller Manifest](../schemas/virtual-controller-manifest.md)
- [Touch Virtual Controller example](../examples/touch-virtual-controller.md)

Summary:

```text
Game Native UI virtual controller
  -> LayerTree hit-test
  -> touch/mouse capture
  -> virtual button/joystick/gesture state
  -> normalized InputAction / InputAxis
  -> gameplay/narrative layer routing
  -> Agent-observable ActionTargets
```

Virtual controller input is semantically equivalent to physical controller, keyboard, USB macro pad, and Agent input after normalization.
