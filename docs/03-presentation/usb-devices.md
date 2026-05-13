# USB / HID / Serial device support

This chapter is kept as a compatibility entry point for older Arcweft design notes.

The canonical device I/O design is now:

- [Device I/O / USB / HID / Gamepads](device-io-usb.md)
- [Device Profiles, Generators, and USB](device-generator-and-usb.md)
- [Streams, Generators, and Live Device Sources](../02-runtime/streams-generators.md)
- [Touch Virtual Controller](touch-virtual-controller.md)

Summary:

```text
USB/HID/serial/gamepad device
  -> permissioned device declaration
  -> generated typed profile where useful
  -> granted device port
  -> Parser<T, E> for reports/packets
  -> Stream<T, E> / Watch<T>
  -> InputAction / Signal / Activity port
```

Do not expose raw native USB APIs directly to DSL scripts or Activities.
