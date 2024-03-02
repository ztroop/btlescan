[![Build](https://github.com/ztroop/bluescan/actions/workflows/build.yml/badge.svg)](https://github.com/ztroop/bluescan/actions/workflows/build.yml)

# bluescan

## Summary

This tool provides a CLI with a real-time, interactive way to view Bluetooth devices, showcasing their Name, TX Power, Address, and RSSI (Received Signal Strength Indicator) in a neatly organized table format.

## Features

- Real-Time Discovery: Continuously scans for Bluetooth devices, updating the list in real-time as new devices appear or existing devices become unavailable.
- Device Information: Displays detailed information about each detected Bluetooth device, including:
    - **Name**: The name of the Bluetooth device, if available.
    - **TX Power**: The transmission power level, indicating the strength at which the device is broadcasting its signal.
    - **Address**: The unique address of the Bluetooth device.
    - **RSSI**: Received Signal Strength Indicator, a measure of the power present in the received signal, indicating how close or far the device is.
- Interactive UI: The terminal-based user interface allows users to scroll through the list of discovered devices, providing an easy way to browse and select devices of interest.
- Keyboard Navigation: Supports simple keyboard controls for navigation:
    - **Up/Down Arrows**: Scroll through the list of devices.
    - **Q**: Quit the application.