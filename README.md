[![Build](https://github.com/ztroop/btlescan/actions/workflows/build.yml/badge.svg)](https://github.com/ztroop/btlescan/actions/workflows/build.yml)

# btlescan

A cross-platform terminal UI for scanning Bluetooth Low Energy devices, inspecting GATT services and characteristics, and reading/writing characteristic values in real time.

![demo](./assets/demo.png)

## Features

- **Device Discovery** — Continuous scanning with live updates of address/UUID, name, TX power, and RSSI.
- **GATT Inspection** — Connect to a device, discover services, and browse characteristics with their properties.
- **Read / Write** — Read characteristic values or write data in hex or text format.
- **Notifications** — Subscribe to characteristic notifications with a timestamped message log.
- **Server Mode** — Framework for GATT server advertising (platform-specific backend required).
- **CSV Export** — Export the device list to a CSV file.

## Keyboard Controls

| Key | Action |
|-----|--------|
| `q` | Quit |
| `Tab` | Cycle focus between panels |
| `↑/↓` or `j/k` | Navigate lists / scroll log |
| `Enter` | Connect to selected device |
| `d` | Disconnect |
| `r` | Read selected characteristic |
| `w` / `i` | Write — enter editing mode |
| `n` | Toggle notification subscription |
| `t` | Toggle hex / text input format |
| `s` | Toggle scan pause/resume |
| `e` | Export devices to CSV |
| `m` | Switch client / server mode |
| `Esc` | Cancel editing |

## Installation

```sh
git clone git@github.com:ztroop/btlescan.git && cd ./btlescan
cargo install --path .
```

### Arch Linux (AUR)

```sh
paru -S btlescan
```

## Alternatives

If you're looking to manage or pair Bluetooth devices, check out [bluetui](https://github.com/pythops/bluetui)!
