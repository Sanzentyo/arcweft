# Example: USB Sensor and HID Dial

```awft
mod game::devices

pub device @device.usb.sensor: UsbDevice {
    permission = user_prompt
    backend = prefer(native_nusb, web_usb)

    filter {
        vendor_id = 0x1209
        product_id = 0xA001
        class = vendor_specific
    }

    endpoints {
        bulk_in = 0x81
        bulk_out = 0x01
    }

    replay = packets_when_test
}

pub parser parse_sensor_packet: Parser<SensorFrame, UsbParseError> {
    // Sans I/O parser for fixed-length sensor packets.
}

stream fn sensor_frames(dev: DevicePort<UsbDevice>) -> Stream<SensorFrame, SensorError> {
    for await packet in dev.bulk_in(0x81) {
        yield parse(parse_sensor_packet, packet.bytes)
            .map_err(.Parse)?
    }
}

pub device @device.hid.dial: HidDevice {
    permission = user_prompt
    backend = prefer(native_hidapi, web_hid)

    filter {
        vendor_id = 0x1209
        product_id = 0xD1A1
        usage_page = 0xFF00
    }
}
```

Use in a flow:

```awft
flow @flow.device_demo device_demo(state: GameState) -> Result<FlowExit, FlowError> {
    let dev =
        try await device.usb(@device.usb.sensor) with {
            pending _ => scene.show(@scene.device_wait); text.show("USBセンサーを接続してください")
            denied _ => return Ok(FlowExit::Goto(@flow.device_optional))
        }

    let frames = sensor_frames(dev)

    select {
        sample = frames.next? => {
            signal.set(@signal.sensor_value, sample.value)
        }

        event .Back => {
            return Ok(FlowExit::Goto(@flow.title))
        }

        frame _ => {
            scene.show(@scene.sensor_live)
            scope {
                text.show("センサー入力を待機中")
            }
            continue
        }
    }
}
```
