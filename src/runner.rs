//! The mod loop: hwcast stream in, display frames out. Reconnects to both ends on its own,
//! so unplugging the display or restarting hwcast never needs a manual restart.

use crate::config::{self, Config};
use crate::display::{self, Display, Side};
use crate::feed;
use std::thread::sleep;
use std::time::Duration;

pub fn side(snap: &serde_json::Value, value: &str, bar: &str, cfg: &Config) -> Side {
    let v = config::metric(snap, value).map(|(x, _)| x.round().clamp(0.0, 99.0) as u8).unwrap_or(0);
    let b = config::metric(snap, bar).map(|m| config::bar_level(m, cfg.bar_temp_range)).unwrap_or(0);
    Side { value: v, bar: b }
}

pub fn run(verbose: bool) {
    let cfg = config::load();
    let log = |m: &str| {
        if verbose {
            println!("{m}");
        }
    };
    log(&format!(
        "hwcast-gcb200 {}: left = {} / bar {}, right = {} / bar {}",
        env!("CARGO_PKG_VERSION"),
        cfg.left.value,
        cfg.left.bar,
        cfg.right.value,
        cfg.right.bar
    ));
    loop {
        let Some(port) = display::find_port(cfg.port.as_deref()) else {
            log("display not found (USB 1A86:5722); retrying in 5 s");
            sleep(Duration::from_secs(5));
            continue;
        };
        let mut disp = match Display::open(&port) {
            Ok(d) => d,
            Err(e) => {
                log(&format!("{e}; retrying in 5 s"));
                sleep(Duration::from_secs(5));
                continue;
            }
        };
        log(&format!("display on {port}"));
        let mut last: Option<(Side, Side)> = None;
        let mut display_failed = false;
        let why = feed::subscribe(&cfg.hwcast, |snap| {
            let l = side(snap, &cfg.left.value, &cfg.left.bar, &cfg);
            let r = side(snap, &cfg.right.value, &cfg.right.bar, &cfg);
            if let Err(e) = disp.show(l, r) {
                log(&e);
                display_failed = true;
                return false;
            }
            if verbose && last != Some((l, r)) {
                println!("left {:>2} bar {}  |  right {:>2} bar {}", l.value, l.bar, r.value, r.bar);
            }
            last = Some((l, r));
            true
        });
        log(&format!("{why}; reconnecting in 3 s"));
        drop(disp);
        sleep(Duration::from_secs(if display_failed { 2 } else { 3 }));
    }
}
