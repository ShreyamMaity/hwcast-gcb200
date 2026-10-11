# hwcast-gcb200

[![Rust](assets/badges/rust.svg)](https://www.rust-lang.org/)
[![Windows](assets/badges/windows.svg)](#requirements)
[![hwcast mod](assets/badges/hwcast-mod.svg)](https://github.com/ShreyamMaity/hwcast#mods)
[![License](assets/badges/license.svg)](LICENSE)
[![GitHub stars](https://badgen.net/github/stars/ShreyamMaity/hwcast-gcb200)](https://github.com/ShreyamMaity/hwcast-gcb200/stargazers)
[![GitHub forks](https://badgen.net/github/forks/ShreyamMaity/hwcast-gcb200)](https://github.com/ShreyamMaity/hwcast-gcb200/network)
[![Setup with an agent](assets/badges/agent.svg)](https://shreyammaity.github.io/hwcast-gcb200/setup/)

A [hwcast](https://github.com/ShreyamMaity/hwcast) mod for the [Ant Esports GCB200](https://link.amazon/B0ajyTKiZ) GPU support bracket. It takes over the bracket's little segment display and shows whatever you want on it, live: both GPUs on a dual GPU build, CPU and GPU, loads, memory, hotspot or VRAM temperatures, even a thermistor probe on your motherboard.

The display has two 2-digit numbers and a 7-block bar under each, labelled CPU and GPU. The vendor app only ever shows CPU and GPU temperature there. With this mod each number and each bar can be any hwcast metric, temperatures or percentages.

## Why use it instead of the vendor app

- **Any metric, not just CPU and GPU temperature.** 28 choices on a typical dual GPU PC: every GPU's temperature, hotspot, memory temperature, load, VRAM, power and fan; CPU temperature, load and busiest thread; RAM and page file; every motherboard sensor. Numbers can be degrees or percent, and the bars can follow something different from the numbers.
- **Made for two GPUs.** One GPU per side out of the box when you have two.
- **Tiny footprint.** One 435 KB exe running as a background service: about 1.5 MB of private memory and close to 0% CPU. No tray app, no .NET runtime, no window.
- **Choose at setup, change any time.** The installer asks what to show. Open **hwcast GCB200 display** from the Start menu to pick again, no admin prompt needed.
- **Nothing is flashed.** It sends the same data frames the vendor app sends. The display firmware is untouched, and uninstalling gives the display back to the vendor app.

## Requirements

- Windows 10 or 11, x64
- [hwcast](https://github.com/ShreyamMaity/hwcast) installed and running (it reads the sensors; this mod only draws them)
- The [GCB200 bracket](https://link.amazon/B0ajyTKiZ) with its display connected over USB. It shows up as a USB serial device, ID `1A86:5722`

## Install

**Easiest:** click **Setup with an agent** above and let your coding agent (Claude Code, Codex, Antigravity, Grok Build or any other) check your PC, ask what you want shown and install it.

**By hand:** download `hwcast-gcb200.exe` from [Releases](https://github.com/ShreyamMaity/hwcast-gcb200/releases/latest) and double-click it.

1. It asks once for admin, copies itself to `Program Files\hwcast-gcb200` and registers a background service that starts at boot under the low-privilege LocalService account.
2. If the vendor GCB200 app is installed, it offers to stop it and turn off its autostart, because only one program can talk to the display.
3. It asks what the display should show:

```
  1. Two GPUs: GPU 0 on the left, GPU 1 on the right, temperature with a load bar
  2. CPU and GPU: temperatures with load bars, the way the box prints it
  3. Load in %: CPU load on the left, GPU 0 load on the right
  4. Memory in %: RAM used on the left, GPU 0 VRAM used on the right
  5. Custom: pick any number and any bar for each side
```

Custom lists every metric your PC offers with its live value, so you can see what you are picking.

## Commands

```
hwcast-gcb200 configure          choose what is shown (same as the Start menu entry)
hwcast-gcb200 configure --preset dual-gpu | cpu-gpu | loads | memory
hwcast-gcb200 configure --left gpu0.temp:gpu0.load --right gpu1.temp:gpu1.load
hwcast-gcb200 status             service, display port, hwcast connection, current layout
hwcast-gcb200 start | stop | restart
hwcast-gcb200 run                run in the foreground and print what is shown
hwcast-gcb200 test               light every segment (stop the service first)
hwcast-gcb200 install [--disable-vendor-app] [--dir <path>]
hwcast-gcb200 uninstall [--keep-config]
```

## Metrics

| Metric | Shows |
|---|---|
| `gpuN.temp`, `gpuN.hotspot`, `gpuN.memtemp` | GPU number N (as `nvidia-smi` counts them): core, hotspot and memory temperature. Hotspot and memory need a card that reports them |
| `gpuN.load`, `gpuN.vram`, `gpuN.power`, `gpuN.fan` | Load, VRAM used, power as % of the limit, fan, all in % |
| `cpu.temp`, `cpu.load`, `cpu.maxcore` | CPU temperature, total load %, busiest thread % |
| `mem.used`, `pagefile.used` | RAM and page file used, % |
| `probeN.temp`, `board.<sensor id>` | Motherboard thermistor probes in order, or any board sensor by its hwcast id |
| `none` | Leave it blank |

Numbers show 0 to 99. Bars fill by percent, or for temperatures from 30 C (empty) to 90 C (full).

## Configuration file

`configure` writes `config.json` in the install folder. You can edit it by hand too, then run `hwcast-gcb200 restart`.

```json
{
  "hwcast": "127.0.0.1:8340",
  "port": null,
  "left":  { "value": "gpu0.temp", "bar": "gpu0.load" },
  "right": { "value": "gpu1.temp", "bar": "gpu1.load" },
  "bar_temp_range": [30, 90]
}
```

| Key | Meaning |
|---|---|
| `hwcast` | Address of hwcast. Another PC works too if its hwcast allows it |
| `port` | COM port of the display, or `null` to find it by USB ID |
| `left`, `right` | `value` is the number, `bar` fills the 7 blocks |
| `bar_temp_range` | Temperatures fill the bar from the first value (empty) to the second (full) |

The printed CPU and GPU labels are part of the glass and cannot change.

## How it works

```
sensors  >  hwcast  >  /events stream  >  hwcast-gcb200  >  USB serial  >  GCB200 display
```

The display is a WCH CH552 microcontroller behind a USB serial port, driving a Holtek HT1621 segment controller. It takes a 20-byte frame: a 6-byte header with command `169`, then the four digits, the two bar levels and a trailer. hwcast publishes one snapshot per second over its [mod API](https://github.com/ShreyamMaity/hwcast/blob/main/docs/mods.md); this mod turns each one into a frame. It reconnects on its own when the display is unplugged or hwcast restarts.

## Build

```
cargo test
./scripts/build.ps1
```

## License

MIT. Not affiliated with Ant Esports.
