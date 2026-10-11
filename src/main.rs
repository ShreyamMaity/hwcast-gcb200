//! hwcast-gcb200: a hwcast mod that shows live hardware data on the Ant Esports GCB200
//! segment display (USB serial 1A86:5722).
//!
//!   hwcast-gcb200                 (double-click) install
//!   hwcast-gcb200 install|uninstall|configure|start|stop|restart|status|test
//!   hwcast-gcb200 run             run in the foreground
//!   hwcast-gcb200 service         entry point used by the Windows service manager

mod config;
mod configure;
mod display;
mod feed;
mod manage;
mod runner;
mod service;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    match args.first().map(String::as_str) {
        None => manage::double_click(),
        Some("--version" | "-V" | "version") => println!("hwcast-gcb200 {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h" | "help" | "/?") => manage::help(),
        Some("run") => runner::run(true),
        Some("service") => service::run(),
        Some(cmd) => manage::dispatch(cmd, rest),
    }
}
