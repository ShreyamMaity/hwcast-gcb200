//! `hwcast-gcb200 configure`: choose what each side of the display shows. Runs after the
//! first install and from the Start menu entry; scripts and agents can pass flags instead:
//!   configure --preset dual-gpu
//!   configure --left gpu0.temp:gpu0.load --right cpu.temp:cpu.load

use crate::config::{self, Config, SideConfig, PRESETS};
use std::io::{BufRead, Write};

pub fn run(args: &[String]) -> Result<(), String> {
    let mut cfg = config::load();
    let flagged = args.iter().any(|a| a == "--preset" || a == "--left" || a == "--right");
    if flagged {
        if let Some(p) = arg(args, "--preset") {
            if !config::apply_preset(&mut cfg, p) {
                let ids: Vec<&str> = PRESETS.iter().map(|p| p.0).collect();
                return Err(format!("unknown preset '{p}' (choose from: {})", ids.join(", ")));
            }
        }
        for (flag, side) in [("--left", &mut cfg.left), ("--right", &mut cfg.right)] {
            if let Some(v) = arg(args, flag) {
                let (value, bar) = v.split_once(':').unwrap_or((v, v));
                *side = SideConfig { value: value.into(), bar: bar.into() };
            }
        }
        return finish(&cfg, args);
    }

    let snap = crate::feed::snapshot(&cfg.hwcast);
    println!("hwcast-gcb200 {}: choose what the display shows\n", env!("CARGO_PKG_VERSION"));
    if snap.is_none() {
        println!("(hwcast is not reachable at {}, so live values and your GPU names are missing.)\n", cfg.hwcast);
    }
    let cat = config::catalog(snap.as_ref());
    let name = |k: &str| cat.iter().find(|c| c.0 == k).map(|c| c.1.clone()).unwrap_or_else(|| k.to_string());
    println!("Now:  left (CPU label)  {} with a {} bar", name(&cfg.left.value), name(&cfg.left.bar));
    println!("      right (GPU label) {} with a {} bar\n", name(&cfg.right.value), name(&cfg.right.bar));

    let gpus = snap.as_ref().and_then(|s| s["gpus"].as_array()).map_or(0, Vec::len);
    let presets: Vec<_> = PRESETS.iter().filter(|p| !(p.0 == "dual-gpu" && snap.is_some() && gpus < 2)).collect();
    for (i, p) in presets.iter().enumerate() {
        println!("  {}. {}", i + 1, p.1);
    }
    let custom = presets.len() + 1;
    println!("  {custom}. Custom: pick any number and any bar for each side");
    println!("  0. Keep it as it is\n");

    let pick = ask_number("Choose", custom)?;
    match pick {
        0 => {
            println!("Nothing changed.");
            return Ok(());
        }
        n if n == custom => {
            println!();
            for (i, (k, d)) in cat.iter().enumerate() {
                let live = snap.as_ref().and_then(|s| config::metric(s, k)).map(|(v, t)| if t { format!("{v:.0} C") } else { format!("{v:.0}%") });
                println!("  {:>2}. {:<44} {}", i + 1, d, live.unwrap_or_default());
            }
            println!("\nNumbers show 0 to 99. Bars fill with % values, or by temperature between {} and {} C.", cfg.bar_temp_range.0, cfg.bar_temp_range.1);
            println!("Press Enter to keep the current choice.\n");
            cfg.left.value = ask_metric("Left number (CPU label)", &cat, &cfg.left.value)?;
            cfg.left.bar = ask_metric("Left bar", &cat, &cfg.left.bar)?;
            cfg.right.value = ask_metric("Right number (GPU label)", &cat, &cfg.right.value)?;
            cfg.right.bar = ask_metric("Right bar", &cat, &cfg.right.bar)?;
        }
        n => {
            config::apply_preset(&mut cfg, presets[n - 1].0);
        }
    }
    finish(&cfg, args)
}

fn finish(cfg: &Config, args: &[String]) -> Result<(), String> {
    config::save(cfg).map_err(|e| format!("{e}\nRun this from an admin terminal, or reinstall to fix the permissions."))?;
    println!("\nSaved: left {} / bar {}, right {} / bar {}", cfg.left.value, cfg.left.bar, cfg.right.value, cfg.right.bar);
    if args.iter().any(|a| a == "--no-restart") {
        return Ok(());
    }
    match crate::manage::restart_if_installed() {
        Ok(true) => println!("The display updates in a few seconds."),
        Ok(false) => println!("Takes effect the next time hwcast-gcb200 starts."),
        Err(e) => println!("Saved, but the service did not restart ({e}). Run `hwcast-gcb200 restart`."),
    }
    Ok(())
}

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn read_line(prompt: &str) -> Result<String, String> {
    print!("{prompt}: ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    std::io::stdin().lock().read_line(&mut s).map_err(|e| e.to_string())?;
    Ok(s.trim().to_string())
}

fn ask_number(prompt: &str, max: usize) -> Result<usize, String> {
    loop {
        let s = read_line(prompt)?;
        match s.parse::<usize>() {
            Ok(n) if n <= max => return Ok(n),
            _ => println!("Type a number from 0 to {max}."),
        }
    }
}

fn ask_metric(prompt: &str, cat: &[(String, String)], current: &str) -> Result<String, String> {
    loop {
        let s = read_line(&format!("{prompt} [{current}]"))?;
        if s.is_empty() {
            return Ok(current.to_string());
        }
        if let Ok(n) = s.parse::<usize>() {
            if (1..=cat.len()).contains(&n) {
                return Ok(cat[n - 1].0.clone());
            }
        }
        // Metric names typed directly are accepted too (e.g. probe0.temp).
        if s.contains('.') || s == "none" {
            return Ok(s);
        }
        println!("Type a number from the list, or press Enter to keep {current}.");
    }
}
