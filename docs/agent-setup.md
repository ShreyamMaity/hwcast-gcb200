# hwcast-gcb200 setup guide (for AI coding agents)

This file is written for an AI coding agent (Claude Code, Codex, Antigravity, Grok Build or any other agent that can run commands). A user clicked "Setup with an agent" in the hwcast-gcb200 README and wants you to set it up on their Windows PC. Follow these steps in order.

## What hwcast-gcb200 is

hwcast-gcb200 is a small open-source Windows program (one Rust exe, under 500 KB) that drives the segment display on the Ant Esports GCB200 GPU support bracket. It replaces the vendor app for the display only. It reads live data from [hwcast](https://github.com/ShreyamMaity/hwcast) and shows any two metrics on the display (temperatures or percentages), each with a 7-block bar. It runs as a background Windows service under the low-privilege LocalService account. Source and releases: https://github.com/ShreyamMaity/hwcast-gcb200

## Ground rules

* Do not change anything on the PC until the user has answered the questions in step 2.
* Before each step that changes the system (installing hwcast, installing this mod, stopping the vendor app, changing what is shown), say in one line what you are about to do and wait for a yes.
* Installing needs administrator rights. Windows shows a UAC prompt; tell the user to expect it and approve it. Never try to bypass UAC or change security settings.
* Download only from the official GitHub releases of ShreyamMaity/hwcast-gcb200 (and ShreyamMaity/hwcast) and verify the SHA256 checksum before running anything.
* Never flash or write firmware to the display. This mod only sends the same data frames the vendor app sends.
* Use PowerShell for all commands. If something fails, stop, show the error, and suggest the next step instead of retrying blindly.

## Step 1: check the PC (read only)

```powershell
[Environment]::OSVersion.VersionString; $env:PROCESSOR_ARCHITECTURE
Get-Service hwcast, hwcast-gcb200 -ErrorAction SilentlyContinue | Select-Object Name, Status
Get-PnpDevice -PresentOnly | Where-Object InstanceId -like '*VID_1A86&PID_5722*' | Select-Object Status, FriendlyName
Get-Process GCB200 -ErrorAction SilentlyContinue | Select-Object Id, Path
schtasks /query /tn UsbMonitor 2>$null
nvidia-smi --query-gpu=index,name --format=csv,noheader 2>$null
```

Tell the user what you found:

* Windows 10 or 11 on x64 is required.
* **hwcast must be installed and running.** If the `hwcast` service is missing, explain that this mod needs it and offer to set it up first by following https://raw.githubusercontent.com/ShreyamMaity/hwcast/main/docs/agent-setup.md, then come back to step 2 here.
* **The display** shows up as a USB serial device with ID `1A86:5722`. If it is missing, ask the user to check the USB cable (or, if they wired it to an internal header themselves, the data wires) and that the display lights up. It can be installed anyway and picks the display up when it appears.
* **The vendor GCB200 app** (process `GCB200.exe`, autostart task `UsbMonitor`) and this mod cannot use the display at the same time.
* If hwcast-gcb200 is already installed, offer to update it (same steps, the settings are kept) or only to change what is shown (step 5).

## Step 2: ask the user (one message, then wait)

1. **What should the display show?** Offer these layouts (number on each side, bar under it):
   * `dual-gpu`: GPU 0 temperature on the left, GPU 1 on the right, each with its load as the bar. Only if `nvidia-smi` listed two or more GPUs.
   * `cpu-gpu`: CPU temperature and load on the left, GPU 0 temperature and load on the right (matches the labels printed on the glass).
   * `loads`: CPU load % on the left, GPU 0 load % on the right.
   * `memory`: RAM used % on the left, GPU 0 VRAM used % on the right.
   * Custom: any two metrics from the list below, and any bar for each.
2. **Vendor app.** If it is installed: stop it and turn off its autostart? (Recommended; the display can only have one owner.)
3. **Install folder.** Default is `C:\Program Files\hwcast-gcb200`.

Metrics for custom layouts: `cpu.temp`, `cpu.load`, `cpu.maxcore` (busiest thread %), `mem.used`, `pagefile.used`, and for each GPU N (as `nvidia-smi` numbers them) `gpuN.temp`, `gpuN.hotspot`, `gpuN.memtemp`, `gpuN.load`, `gpuN.vram`, `gpuN.power`, `gpuN.fan`. Board sensors: `probeN.temp` or `board.<sensor id>` (ids come from `hwcast --once`). `none` leaves a side blank. Numbers show 0 to 99; bars fill by percent, or by temperature between 30 and 90 C.

## Step 3: download and verify

```powershell
$dir = Join-Path $env:TEMP 'hwcast-gcb200-setup'; New-Item -ItemType Directory -Force $dir | Out-Null
$rel = Invoke-RestMethod https://api.github.com/repos/ShreyamMaity/hwcast-gcb200/releases/latest
foreach ($n in 'hwcast-gcb200.exe', 'SHA256SUMS.txt') {
  $a = $rel.assets | Where-Object name -eq $n
  Invoke-WebRequest $a.browser_download_url -OutFile (Join-Path $dir $n) -UseBasicParsing
}
$want = (Select-String -Path "$dir\SHA256SUMS.txt" -Pattern ' hwcast-gcb200\.exe$').Line.Split(' ')[0]
$got = (Get-FileHash "$dir\hwcast-gcb200.exe" -Algorithm SHA256).Hash.ToLower()
if ($want -ne $got) { throw "Checksum mismatch, stopping." } else { "Checksum OK, version $($rel.tag_name)" }
```

If the checksum does not match, stop and tell the user. Do not run the file.

## Step 4: install (after the user says yes)

One UAC prompt. Add `--disable-vendor-app` if they agreed in step 2, and `--dir "<path>"` only for a non-default folder:

```powershell
& "$env:TEMP\hwcast-gcb200-setup\hwcast-gcb200.exe" install --no-pause --disable-vendor-app
```

## Step 5: set what is shown

No admin needed. Use the layout they chose:

```powershell
$exe = "C:\Program Files\hwcast-gcb200\hwcast-gcb200.exe"
& $exe configure --preset dual-gpu
# or a custom layout: --left <number>[:<bar>] --right <number>[:<bar>]
& $exe configure --left cpu.temp:cpu.load --right gpu0.hotspot:gpu0.load
```

## Step 6: verify and hand over

```powershell
& "C:\Program Files\hwcast-gcb200\hwcast-gcb200.exe" status
```

`status` should show the service running, the display on a COM port and hwcast reachable. Ask the user to look at the display: within a few seconds it should switch from its startup screen to live values. Tell them they can change the layout any time from the Start menu entry **hwcast GCB200 display**, or by asking you again.

## Useful commands

| Command | Does |
|---|---|
| `hwcast-gcb200 status` | Service, display port, hwcast connection, current layout |
| `hwcast-gcb200 configure` | Menu to choose what is shown (same as the Start menu entry) |
| `hwcast-gcb200 stop` / `start` | Stops or starts the service (no admin needed) |
| `hwcast-gcb200 test` | Lights every segment (stop the service first) |
| `hwcast-gcb200 uninstall` | Removes everything (admin prompt). Also in Settings > Apps |
