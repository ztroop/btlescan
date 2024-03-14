[![Build](https://github.com/ztroop/btlescan/actions/workflows/build.yml/badge.svg)](https://github.com/ztroop/btlescan/actions/workflows/build.yml)

# btlescan

## Summary

This tool provides a cross-platform CLI with an interactive way to view Bluetooth Low Energy (BTLE) devices, showcasing their Address/UUID, Name, TX Power, and RSSI (Received Signal Strength Indicator) in a neatly organized table format.

## Features

- Real-Time Discovery: Continuously scans for Bluetooth devices, updating the list in real-time as new devices appear or existing devices become unavailable.
- Device Information: Displays detailed information about each detected Bluetooth device, including:
    - **Address/UUID**: The unique address or UUID of the Bluetooth device.
    - **Name**: The name of the Bluetooth device, if available.
    - **TX Power**: The transmission power level, indicating the strength at which the device is broadcasting its signal.
    - **RSSI**: Received Signal Strength Indicator, a measure of the power present in the received signal, indicating how close or far the device is.
- Interactive UI: The terminal-based user interface allows users to scroll through the list of discovered devices, providing an easy way to browse and select devices of interest.
- Keyboard Navigation: Supports simple keyboard controls for navigation:
    - **Up/Down Arrows**: Scroll through the list of devices.
    - **Q**: Quit the application.
    - **S**: Toggle scanning.

## Installation

```sh
git clone git@github.com:ztroop/btlescan.git && cd ./btlescan
cargo install --path .
```

## Alternatives

- See [bluetui] for managing or pairing Bluetooth devices.