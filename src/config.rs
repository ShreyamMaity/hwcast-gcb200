//! config.json next to the exe (all keys optional):
//!
//! {
//!   "hwcast": "127.0.0.1:8340",
//!   "port": null,                                    // COM port; null = find by USB ID
//!   "left":  { "value": "gpu0.temp", "bar": "gpu0.load" },
//!   "right": { "value": "gpu1.temp", "bar": "gpu1.load" },
//!   "bar_temp_range": [30, 90]                       // degrees C mapped to an empty..full bar
//! }
//!
//! Metrics: gpuN.temp gpuN.hotspot gpuN.memtemp gpuN.load gpuN.vram gpuN.power gpuN.fan
//! (N = nvidia-smi index), cpu.temp cpu.load cpu.maxcore, mem.used, pagefile.used,
//! probeN.temp (board probes in order), board.<sensor id>, none.
//! `hwcast-gcb200 configure` picks these from a menu.

use serde_json::Value;

#[derive(Clone, Debug)]
pub struct SideConfig {
    pub value: String,
    pub bar: String,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub hwcast: String,
    pub port: Option<String>,
    pub left: SideConfig,
    pub right: SideConfig,
    pub bar_temp_range: (f64, f64),
}

impl Default for Config {
    fn default() -> Self {
        Config {
            hwcast: "127.0.0.1:8340".into(),
            port: None,
            left: SideConfig { value: "gpu0.temp".into(), bar: "gpu0.load".into() },
            right: SideConfig { value: "gpu1.temp".into(), bar: "gpu1.load".into() },
            bar_temp_range: (30.0, 90.0),
        }
    }
}

pub fn path() -> Option<std::path::PathBuf> {
    std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("config.json")))
}

pub fn load() -> Config {
    let mut c = Config::default();
    let Some(text) = path().and_then(|p| std::fs::read_to_string(p).ok()) else { return c };
    let Ok(v) = serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) else {
        eprintln!("config.json is not valid JSON; using defaults");
        return c;
    };
    if let Some(s) = v["hwcast"].as_str() {
        c.hwcast = s.to_string();
    }
    c.port = v["port"].as_str().map(String::from);
    for (key, side) in [("left", &mut c.left), ("right", &mut c.right)] {
        if let Some(s) = v[key]["value"].as_str() {
            side.value = s.to_string();
        }
        if let Some(s) = v[key]["bar"].as_str() {
            side.bar = s.to_string();
        }
    }
    if let (Some(lo), Some(hi)) = (v["bar_temp_range"][0].as_f64(), v["bar_temp_range"][1].as_f64()) {
        if hi > lo {
            c.bar_temp_range = (lo, hi);
        }
    }
    c
}

pub fn to_json(c: &Config) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "hwcast": c.hwcast,
        "port": c.port,
        "left": { "value": c.left.value, "bar": c.left.bar },
        "right": { "value": c.right.value, "bar": c.right.bar },
        "bar_temp_range": [c.bar_temp_range.0, c.bar_temp_range.1]
    }))
    .unwrap()
}

pub fn save(c: &Config) -> Result<(), String> {
    let p = path().ok_or("cannot find the install folder")?;
    std::fs::write(&p, to_json(c)).map_err(|e| format!("cannot write {}: {e}", p.display()))
}

/// Ready-made layouts: (id, description, left value, left bar, right value, right bar).
pub const PRESETS: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("dual-gpu", "Two GPUs: GPU 0 on the left, GPU 1 on the right, temperature with a load bar", "gpu0.temp", "gpu0.load", "gpu1.temp", "gpu1.load"),
    ("cpu-gpu", "CPU and GPU: temperatures with load bars, the way the box prints it", "cpu.temp", "cpu.load", "gpu0.temp", "gpu0.load"),
    ("loads", "Load in %: CPU load on the left, GPU 0 load on the right", "cpu.load", "cpu.load", "gpu0.load", "gpu0.load"),
    ("memory", "Memory in %: RAM used on the left, GPU 0 VRAM used on the right", "mem.used", "mem.used", "gpu0.vram", "gpu0.vram"),
];

pub fn apply_preset(c: &mut Config, id: &str) -> bool {
    let Some(p) = PRESETS.iter().find(|p| p.0 == id) else { return false };
    c.left = SideConfig { value: p.2.into(), bar: p.3.into() };
    c.right = SideConfig { value: p.4.into(), bar: p.5.into() };
    true
}

/// Every metric this PC offers right now, as (key, description), for the chooser.
pub fn catalog(snap: Option<&Value>) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = vec![
        ("cpu.temp".into(), "CPU temperature".into()),
        ("cpu.load".into(), "CPU load %".into()),
        ("cpu.maxcore".into(), "Busiest CPU thread %".into()),
        ("mem.used".into(), "RAM used %".into()),
        ("pagefile.used".into(), "Page file used %".into()),
    ];
    let gpus = snap.and_then(|s| s["gpus"].as_array()).cloned().unwrap_or_default();
    for g in &gpus {
        let (Some(i), name) = (g["index"].as_u64(), g["name"].as_str().unwrap_or("GPU")) else { continue };
        let mut add = |field: &str, what: &str| out.push((format!("gpu{i}.{field}"), format!("GPU {i} ({name}) {what}")));
        add("temp", "temperature");
        if !g["hotspot"].is_null() {
            add("hotspot", "hotspot temperature");
        }
        if !g["mem_temp"].is_null() {
            add("memtemp", "memory temperature");
        }
        add("load", "load %");
        add("vram", "VRAM used %");
        add("power", "power % of limit");
        add("fan", "fan %");
    }
    if gpus.is_empty() {
        for (k, d) in [("gpu0.temp", "GPU 0 temperature"), ("gpu0.load", "GPU 0 load %"), ("gpu1.temp", "GPU 1 temperature"), ("gpu1.load", "GPU 1 load %")] {
            out.push((k.into(), d.into()));
        }
    }
    if let Some(temps) = snap.and_then(|s| s["sensors"]["temps"].as_array()) {
        for t in temps.iter().filter(|t| !t["v"].is_null()) {
            if let (Some(id), Some(label)) = (t["id"].as_str(), t["label"].as_str()) {
                out.push((format!("board.{id}"), format!("Board: {label}")));
            }
        }
    }
    out.push(("none".into(), "Nothing (blank)".into()));
    out
}

/// A metric value and whether it is a temperature (bars scale those by `bar_temp_range`).
pub fn metric(snap: &Value, key: &str) -> Option<(f64, bool)> {
    let (head, field) = key.split_once('.').unwrap_or((key, ""));
    let pct = |used: Option<f64>, total: Option<f64>| match (used, total) {
        (Some(u), Some(t)) if t > 0.0 => Some(u / t * 100.0),
        _ => None,
    };
    if let Some(n) = head.strip_prefix("gpu").and_then(|n| n.parse::<u64>().ok()) {
        let g = snap["gpus"].as_array()?.iter().find(|g| g["index"].as_u64() == Some(n))?;
        return match field {
            "temp" => g["temp"].as_f64().map(|t| (t, true)),
            "load" => g["util"].as_f64().map(|v| (v, false)),
            "vram" => pct(g["vram_used_gb"].as_f64(), g["vram_total_gb"].as_f64()).map(|v| (v, false)),
            "power" => pct(g["power_w"].as_f64(), g["power_limit_w"].as_f64()).map(|v| (v, false)),
            "fan" => g["fan_pct"].as_f64().map(|v| (v, false)),
            "hotspot" => g["hotspot"].as_f64().map(|t| (t, true)),
            "memtemp" => g["mem_temp"].as_f64().map(|t| (t, true)),
            _ => None,
        };
    }
    // Any board temperature by its hwcast sensor id, e.g. board.it8696e.t3
    if let Some(id) = key.strip_prefix("board.") {
        let t = snap["sensors"]["temps"].as_array()?.iter().find(|t| t["id"].as_str() == Some(id))?;
        return t["v"].as_f64().map(|v| (v, true));
    }
    if let Some(n) = head.strip_prefix("probe").and_then(|n| n.parse::<usize>().ok()) {
        let probes: Vec<&Value> = snap["sensors"]["temps"].as_array()?.iter().filter(|t| t["probe"].as_bool() == Some(true)).collect();
        return probes.get(n)?["v"].as_f64().map(|t| (t, true));
    }
    match key {
        "cpu.temp" => snap["cpu"]["tctl"].as_f64().map(|t| (t, true)),
        "cpu.load" => snap["cpu"]["load"].as_f64().map(|v| (v, false)),
        "cpu.maxcore" => snap["cpu"]["cores"].as_array()?.iter().filter_map(Value::as_f64).reduce(f64::max).map(|v| (v, false)),
        "mem.used" => pct(snap["mem"]["used_gb"].as_f64(), snap["mem"]["total_gb"].as_f64()).map(|v| (v, false)),
        "pagefile.used" => pct(snap["mem"]["pagefile_used_gb"].as_f64(), snap["mem"]["pagefile_size_gb"].as_f64()).map(|v| (v, false)),
        _ => None,
    }
}

/// 0-7 bar blocks for a metric.
pub fn bar_level(v: (f64, bool), range: (f64, f64)) -> u8 {
    let frac = if v.1 { (v.0 - range.0) / (range.1 - range.0) } else { v.0 / 100.0 };
    (frac.clamp(0.0, 1.0) * 7.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Value {
        serde_json::json!({
            "cpu": { "tctl": 71.4, "load": 23.0, "cores": [10.0, 88.0, 5.0] },
            "mem": { "used_gb": 48.0, "total_gb": 96.0, "pagefile_used_gb": 1.0, "pagefile_size_gb": 29.0 },
            "gpus": [
                { "index": 0, "temp": 61, "util": 100, "vram_used_gb": 8.0, "vram_total_gb": 16.0, "power_w": 150.0, "power_limit_w": 300.0, "fan_pct": 40 },
                { "index": 1, "temp": 44, "util": 0, "vram_used_gb": 0.0, "vram_total_gb": 20.0, "power_w": 20.0, "power_limit_w": 320.0, "fan_pct": 0 }
            ],
            "sensors": { "temps": [ { "id": "a", "v": 40, "probe": false }, { "id": "b", "v": 47, "probe": true } ] }
        })
    }

    #[test]
    fn resolves_metrics() {
        let s = snap();
        assert_eq!(metric(&s, "gpu0.temp"), Some((61.0, true)));
        assert_eq!(metric(&s, "gpu1.load"), Some((0.0, false)));
        assert_eq!(metric(&s, "gpu0.vram"), Some((50.0, false)));
        assert_eq!(metric(&s, "gpu0.power"), Some((50.0, false)));
        assert_eq!(metric(&s, "cpu.temp"), Some((71.4, true)));
        assert_eq!(metric(&s, "mem.used"), Some((50.0, false)));
        assert_eq!(metric(&s, "probe0.temp"), Some((47.0, true)));
        assert_eq!(metric(&s, "board.a"), Some((40.0, true)));
        assert_eq!(metric(&s, "cpu.maxcore"), Some((88.0, false)));
        assert_eq!(metric(&s, "gpu2.temp"), None);
        assert_eq!(metric(&s, "none"), None);
    }

    #[test]
    fn bars() {
        assert_eq!(bar_level((100.0, false), (30.0, 90.0)), 7);
        assert_eq!(bar_level((0.0, false), (30.0, 90.0)), 0);
        assert_eq!(bar_level((50.0, false), (30.0, 90.0)), 4);
        assert_eq!(bar_level((60.0, true), (30.0, 90.0)), 4);
        assert_eq!(bar_level((95.0, true), (30.0, 90.0)), 7);
    }
}
