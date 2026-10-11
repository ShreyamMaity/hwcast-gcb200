//! Install / uninstall / control the hwcast-gcb200 service. Only install and uninstall
//! need admin; interactive users may start and stop the installed service.

use crate::service::NAME;
use std::ffi::OsStr;
use std::io::{BufRead, Write};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};
use windows_service::service::{
    ServiceAccess, ServiceAction, ServiceActionType, ServiceDependency, ServiceErrorControl, ServiceFailureActions,
    ServiceFailureResetPeriod, ServiceInfo, ServiceStartType, ServiceState, ServiceType,
};
use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};

const SERVICE_SDDL: &str =
    "D:(A;;CCLCSWRPWPDTLOCRRC;;;SY)(A;;CCDCLCSWRPWPDTLOCRSDRCWDWO;;;BA)(A;;CCLCSWRPWPDTLOCRRC;;;IU)(A;;CCLCSWLOCRRC;;;SU)";
const UNINSTALL_KEY: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\hwcast-gcb200";
const VENDOR_TASK: &str = "UsbMonitor";
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn help() {
    println!(
        "hwcast-gcb200 {} - hwcast mod for the Ant Esports GCB200 segment display

Shows any two hwcast metrics (temperatures or percentages) with a bar each.
Needs hwcast running (https://github.com/ShreyamMaity/hwcast).

Usage:
  hwcast-gcb200                    Install (asks once for admin)
  hwcast-gcb200 install [options]  Install or update the background service
      --dir <path>                 Install folder (default: Program Files\\hwcast-gcb200)
      --disable-vendor-app         Stop the GCB200 app and turn off its autostart
      --no-pause                   Close the admin window when done (for scripts)
  hwcast-gcb200 uninstall [--keep-config]
  hwcast-gcb200 configure        Choose what the display shows (also in the Start menu)
      --preset <id>                dual-gpu, cpu-gpu, loads or memory
      --left <value>[:<bar>]       e.g. --left gpu0.temp:gpu0.load (no bar given = same metric)
      --right <value>[:<bar>]
  hwcast-gcb200 start | stop | restart | status
  hwcast-gcb200 run                Run in the foreground and print what is shown
  hwcast-gcb200 test               Light every segment for 3 seconds (service must be stopped)
  hwcast-gcb200 --version

Config: config.json in the install folder (see README for metrics like gpu0.temp, cpu.load).",
        env!("CARGO_PKG_VERSION")
    );
}

pub fn dispatch(cmd: &str, rest: &[String]) {
    let r = match cmd {
        "install" => install(rest),
        "uninstall" | "remove" => uninstall(rest),
        "start" => control(true),
        "stop" => control(false),
        "restart" => control(false).and_then(|_| control(true)),
        "status" => status(),
        "test" => test(),
        "configure" | "config" => crate::configure::run(rest).map(|_| pause_if(rest)),
        _ => {
            eprintln!("Unknown command: {cmd}\n");
            help();
            std::process::exit(2);
        }
    };
    if let Err(e) = r {
        eprintln!("\nError: {e}");
        if !rest.iter().any(|a| a == "--pause") && rest.iter().any(|a| a == "--pause-on-error") {
            wait_enter();
        }
        pause_if(rest);
        std::process::exit(1);
    }
}

pub fn double_click() {
    if installed_dir().is_some() {
        if let Err(e) = crate::configure::run(&[]) {
            eprintln!("\nError: {e}");
        }
        wait_enter();
        return;
    }
    println!("hwcast-gcb200 {}\n", env!("CARGO_PKG_VERSION"));
    println!("Drives the GCB200 segment display with live data from hwcast.");
    println!("Installing makes these changes (undone by `hwcast-gcb200 uninstall` or Settings > Apps):");
    println!("  - copies hwcast-gcb200.exe to {}", default_dir().display());
    println!("  - registers the 'hwcast-gcb200' Windows service (low-privilege LocalService account, starts at boot)");
    println!("  - adds a Start menu entry to change what the display shows, and a Settings > Apps entry");
    println!("Windows will ask for administrator permission once.\n");
    if !ask_yes("Install now?") {
        return;
    }
    let mut args = vec!["--no-pause".to_string(), "--pause-on-error".to_string()];
    if vendor_task_enabled() || vendor_running() {
        println!("\nThe GCB200 app is installed and would compete for the display.");
        if ask_yes("Stop it and turn off its autostart (task 'UsbMonitor')?") {
            args.push("--disable-vendor-app".to_string());
        }
    }
    if let Err(e) = install(&args) {
        eprintln!("\nError: {e}");
        wait_enter();
        return;
    }
    println!("\nInstalled and running. Now choose what the display shows.\n");
    if let Err(e) = crate::configure::run(&[]) {
        eprintln!("\nError: {e}");
    }
    println!("\nOpen \"hwcast GCB200 display\" from the Start menu any time to change it.");
    wait_enter();
}

fn default_dir() -> PathBuf {
    PathBuf::from(std::env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".into())).join("hwcast-gcb200")
}

fn install(args: &[String]) -> Result<(), String> {
    if !is_admin() {
        let mut elevated = vec!["install".to_string()];
        elevated.extend(args.iter().cloned());
        if !args.iter().any(|a| a == "--no-pause") {
            elevated.push("--pause".into());
        }
        let code = run_elevated(&elevated)?;
        return if code == 0 { Ok(()) } else { Err("installation did not finish (see the other window)".into()) };
    }
    let dir = arg_value(args, "--dir").map(PathBuf::from).or_else(installed_dir).unwrap_or_else(default_dir);
    let exe = dir.join("hwcast-gcb200.exe");
    println!("Installing hwcast-gcb200 {} to {}", env!("CARGO_PKG_VERSION"), dir.display());

    if args.iter().any(|a| a == "--disable-vendor-app") {
        disable_vendor_app();
    }

    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE)
        .map_err(|e| format!("cannot open the service manager: {e}"))?;
    if let Ok(svc) = mgr.open_service(NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP) {
        let _ = svc.stop();
        wait_state(&svc, ServiceState::Stopped, 15);
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if !same_file(&me, &exe) {
        copy_with_retry(&me, &exe)?;
    }
    let cfg = dir.join("config.json");
    if !cfg.exists() {
        // Two GPUs: one per side. Otherwise the CPU and GPU layout printed on the glass.
        let mut c = crate::config::Config::default();
        let gpus = crate::feed::snapshot(&c.hwcast).and_then(|s| s["gpus"].as_array().map(Vec::len)).unwrap_or(0);
        crate::config::apply_preset(&mut c, if gpus >= 2 { "dual-gpu" } else { "cpu-gpu" });
        std::fs::write(&cfg, crate::config::to_json(&c)).map_err(|e| format!("cannot write config.json: {e}"))?;
    }
    // Let signed-in users change what is shown without an admin prompt (Users: modify).
    run_quiet("icacls", &[&cfg.display().to_string(), "/grant", "*S-1-5-32-545:M"]);

    let info = ServiceInfo {
        name: NAME.into(),
        display_name: "hwcast GCB200 display".into(),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe.clone(),
        launch_arguments: vec!["service".into()],
        // Start after hwcast when it is installed; the mod reconnects on its own anyway.
        dependencies: if service_exists("hwcast") { vec![ServiceDependency::Service("hwcast".into())] } else { vec![] },
        account_name: Some(r"NT AUTHORITY\LocalService".into()),
        account_password: None,
    };
    let svc = match mgr.open_service(NAME, ServiceAccess::all()) {
        Ok(s) => {
            s.change_config(&info).map_err(|e| format!("cannot update the service: {e}"))?;
            s
        }
        Err(_) => mgr.create_service(&info, ServiceAccess::all()).map_err(|e| format!("cannot create the service: {e}"))?,
    };
    let _ = svc.set_description("hwcast mod: shows live hardware data on the Ant Esports GCB200 segment display.");
    let _ = svc.update_failure_actions(ServiceFailureActions {
        reset_period: ServiceFailureResetPeriod::After(Duration::from_secs(24 * 3600)),
        reboot_msg: None,
        command: None,
        actions: Some(vec![ServiceAction { action_type: ServiceActionType::Restart, delay: Duration::from_secs(10) }; 3]),
    });
    run_quiet("sc.exe", &["sdset", NAME, SERVICE_SDDL]);
    write_uninstall_entry(&dir, &exe);
    write_start_menu(&exe);
    println!("  service registered (LocalService account, starts at boot)");

    svc.start::<&OsStr>(&[]).or_else(|e| if is_running() { Ok(()) } else { Err(e) }).map_err(|e| format!("installed, but the service did not start: {e}"))?;
    wait_state(&svc, ServiceState::Running, 10);
    println!("\nhwcast-gcb200 is installed and running.");
    println!("Config: {}", cfg.display());
    pause_if(args);
    Ok(())
}

fn uninstall(args: &[String]) -> Result<(), String> {
    if !is_admin() {
        let mut elevated = vec!["uninstall".to_string()];
        elevated.extend(args.iter().filter(|a| *a != "--pause").cloned());
        if !args.iter().any(|a| a == "--no-pause") {
            elevated.push("--pause".into());
        }
        run_elevated(&elevated)?;
        return Ok(());
    }
    let keep = args.iter().any(|a| a == "--keep-config");
    let dir = installed_dir();
    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(|e| e.to_string())?;
    if let Ok(svc) = mgr.open_service(NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE) {
        let _ = svc.stop();
        wait_state(&svc, ServiceState::Stopped, 15);
        svc.delete().map_err(|e| format!("cannot remove the service: {e}"))?;
        println!("  service removed");
    }
    reg::delete_tree(UNINSTALL_KEY);
    if let Some(sm) = start_menu() {
        let _ = std::fs::remove_file(sm.join(SHORTCUT));
    }
    if let Some(dir) = dir {
        let exe = dir.join("hwcast-gcb200.exe");
        let cfg = dir.join("config.json");
        let me = std::env::current_exe().unwrap_or_default();
        if same_file(&me, &exe) {
            let mut script = format!(r#"/c ping 127.0.0.1 -n 3 >nul & del /f /q "{}""#, exe.display());
            if !keep {
                script.push_str(&format!(r#" & del /f /q "{}""#, cfg.display()));
            }
            script.push_str(&format!(r#" & rmdir "{}""#, dir.display()));
            let _ = Command::new("cmd").raw_arg(script).creation_flags(CREATE_NO_WINDOW).spawn();
        } else {
            let _ = std::fs::remove_file(&exe);
            if !keep {
                let _ = std::fs::remove_file(&cfg);
            }
            let _ = std::fs::remove_dir(&dir);
        }
        println!("  files removed from {}", dir.display());
    }
    println!("\nhwcast-gcb200 is uninstalled. The GCB200 app's autostart was left as it is.");
    pause_if(args);
    Ok(())
}

fn control(start: bool) -> Result<(), String> {
    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).map_err(|e| e.to_string())?;
    let svc = mgr
        .open_service(NAME, ServiceAccess::QUERY_STATUS | ServiceAccess::START | ServiceAccess::STOP)
        .map_err(|_| "hwcast-gcb200 is not installed. Run `hwcast-gcb200 install` first.".to_string())?;
    let r = if start { svc.start::<&OsStr>(&[]) } else { svc.stop().map(|_| ()) };
    if let Err(e) = r {
        let state = svc.query_status().map(|s| s.current_state).ok();
        let already = (start && state == Some(ServiceState::Running)) || (!start && state == Some(ServiceState::Stopped));
        if !already {
            return Err(format!("could not {} the service: {e}", if start { "start" } else { "stop" }));
        }
    }
    wait_state(&svc, if start { ServiceState::Running } else { ServiceState::Stopped }, 15);
    println!("hwcast-gcb200 is {}.", if start { "running" } else { "stopped (it starts again at the next boot)" });
    Ok(())
}

/// Restarts the service so a new config.json takes effect. Ok(false) when not installed.
pub fn restart_if_installed() -> Result<bool, String> {
    if installed_dir().is_none() {
        return Ok(false);
    }
    control(false).and_then(|_| control(true)).map(|_| true)
}

fn status() -> Result<(), String> {
    let cfg = crate::config::load();
    println!("hwcast-gcb200 {}", env!("CARGO_PKG_VERSION"));
    match installed_dir() {
        Some(d) => println!("  Service:  {} ({})", if is_running() { "running" } else { "stopped" }, d.display()),
        None => println!("  Service:  not installed"),
    }
    println!("  Display:  {}", crate::display::find_port(cfg.port.as_deref()).unwrap_or_else(|| "not found (USB 1A86:5722)".into()));
    let hw = std::net::TcpStream::connect_timeout(&cfg.hwcast.parse().map_err(|_| "bad hwcast address".to_string())?, Duration::from_secs(2)).is_ok();
    println!("  hwcast:   {} ({})", if hw { "reachable" } else { "not reachable" }, cfg.hwcast);
    println!("  Left:     {} with bar {}", cfg.left.value, cfg.left.bar);
    println!("  Right:    {} with bar {}", cfg.right.value, cfg.right.bar);
    if vendor_running() || vendor_task_enabled() {
        println!("  Warning:  the GCB200 app {} and may take the display", if vendor_running() { "is running" } else { "autostarts" });
    }
    Ok(())
}

fn test() -> Result<(), String> {
    use crate::display::{Display, Side};
    let port = crate::display::find_port(crate::config::load().port.as_deref()).ok_or("display not found")?;
    let mut d = Display::open(&port).map_err(|e| format!("{e} (stop the service first: hwcast-gcb200 stop)"))?;
    println!("Lighting every segment on {port} for 3 seconds...");
    for _ in 0..3 {
        d.show(Side { value: 88, bar: 7 }, Side { value: 88, bar: 7 })?;
        sleep(Duration::from_secs(1));
    }
    d.show(Side { value: 0, bar: 0 }, Side { value: 0, bar: 0 })?;
    println!("Done.");
    Ok(())
}

// ------------------------------------------------------------- vendor app

fn vendor_running() -> bool {
    Command::new("tasklist").args(["/fi", "imagename eq GCB200.exe", "/nh"]).creation_flags(CREATE_NO_WINDOW).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_ascii_lowercase().contains("gcb200.exe"))
        .unwrap_or(false)
}

fn vendor_task_enabled() -> bool {
    Command::new("schtasks").args(["/query", "/tn", VENDOR_TASK, "/xml"]).creation_flags(CREATE_NO_WINDOW).output()
        .map(|o| {
            let x = String::from_utf8_lossy(&o.stdout).to_ascii_lowercase();
            o.status.success() && x.contains("gcb200") && !x.contains("<enabled>false</enabled>")
        })
        .unwrap_or(false)
}

fn disable_vendor_app() {
    if vendor_task_enabled() {
        let ok = run_quiet("schtasks", &["/change", "/tn", VENDOR_TASK, "/disable"]);
        println!("  GCB200 autostart task '{VENDOR_TASK}': {}", if ok { "disabled" } else { "could not disable" });
    }
    if vendor_running() {
        run_quiet("taskkill", &["/f", "/im", "GCB200.exe"]);
        println!("  GCB200 app stopped");
    }
}

// ------------------------------------------------------------- helpers

fn service_exists(name: &str) -> bool {
    ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .ok()
        .and_then(|m| m.open_service(name, ServiceAccess::QUERY_STATUS).ok())
        .is_some()
}

fn installed_dir() -> Option<PathBuf> {
    let mgr = ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).ok()?;
    let svc = mgr.open_service(NAME, ServiceAccess::QUERY_CONFIG).ok()?;
    let raw = svc.query_config().ok()?.executable_path.display().to_string();
    let exe = if let Some(s) = raw.strip_prefix('"') {
        s.split('"').next().unwrap_or("").to_string()
    } else {
        let lower = raw.to_ascii_lowercase();
        lower.find(".exe").map(|i| raw[..i + 4].to_string()).unwrap_or(raw)
    };
    PathBuf::from(exe).parent().map(Path::to_path_buf)
}

fn is_running() -> bool {
    ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT)
        .ok()
        .and_then(|m| m.open_service(NAME, ServiceAccess::QUERY_STATUS).ok())
        .and_then(|s| s.query_status().ok())
        .map_or(false, |s| s.current_state == ServiceState::Running)
}

fn wait_state(svc: &windows_service::service::Service, want: ServiceState, secs: u64) {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        if svc.query_status().map(|s| s.current_state == want).unwrap_or(false) {
            return;
        }
        sleep(Duration::from_millis(250));
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

fn copy_with_retry(from: &Path, to: &Path) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..10 {
        match std::fs::copy(from, to) {
            Ok(_) => return Ok(()),
            Err(e) => last = e.to_string(),
        }
        sleep(Duration::from_millis(500));
    }
    Err(format!("cannot copy to {}: {last}", to.display()))
}

const SHORTCUT: &str = "hwcast GCB200 display.lnk";

fn start_menu() -> Option<PathBuf> {
    std::env::var("ProgramData").ok().map(|pd| PathBuf::from(pd).join(r"Microsoft\Windows\Start Menu\Programs"))
}

/// Start menu entry that opens the display chooser (`configure --pause`).
fn write_start_menu(exe: &Path) {
    let Some(sm) = start_menu() else { return };
    let lnk = sm.join(SHORTCUT);
    let ps = format!(
        "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.Arguments='configure --pause';$s.Description='Choose what the GCB200 display shows';$s.IconLocation='{},0';$s.Save()",
        lnk.display(),
        exe.display(),
        exe.display()
    );
    if run_quiet("powershell", &["-NoProfile", "-NonInteractive", "-Command", &ps]) {
        println!("  Start menu entry: hwcast GCB200 display");
    }
}

fn write_uninstall_entry(dir: &Path, exe: &Path) {
    let e = exe.display().to_string();
    reg::set_str(UNINSTALL_KEY, "DisplayName", "hwcast GCB200 display (hwcast mod)");
    reg::set_str(UNINSTALL_KEY, "DisplayVersion", env!("CARGO_PKG_VERSION"));
    reg::set_str(UNINSTALL_KEY, "Publisher", "hwcast");
    reg::set_str(UNINSTALL_KEY, "DisplayIcon", &e);
    reg::set_str(UNINSTALL_KEY, "InstallLocation", &dir.display().to_string());
    reg::set_str(UNINSTALL_KEY, "UninstallString", &format!("\"{e}\" uninstall --pause"));
    reg::set_str(UNINSTALL_KEY, "QuietUninstallString", &format!("\"{e}\" uninstall"));
    reg::set_dword(UNINSTALL_KEY, "NoModify", 1);
    reg::set_dword(UNINSTALL_KEY, "NoRepair", 1);
}

fn arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn run_quiet(prog: &str, args: &[&str]) -> bool {
    Command::new(prog).args(args).creation_flags(CREATE_NO_WINDOW).output().map_or(false, |o| o.status.success())
}

fn ask_yes(q: &str) -> bool {
    print!("{q} [Y/n] ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    if std::io::stdin().lock().read_line(&mut s).is_err() {
        return false;
    }
    let s = s.trim().to_ascii_lowercase();
    s.is_empty() || s == "y" || s == "yes"
}

fn pause_if(args: &[String]) {
    if args.iter().any(|a| a == "--pause") {
        wait_enter();
    }
}

fn wait_enter() {
    print!("\nPress Enter to close this window.");
    let _ = std::io::stdout().flush();
    let _ = std::io::stdin().lock().read_line(&mut String::new());
}

fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

fn run_elevated(args: &[String]) -> Result<u32, String> {
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let w = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let file = w(&exe.display().to_string());
    let params = w(&args.iter().map(|a| if a.contains(' ') { format!("\"{a}\"") } else { a.clone() }).collect::<Vec<_>>().join(" "));
    let verb = w("runas");
    unsafe {
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<SHELLEXECUTEINFOW>() as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = params.as_ptr();
        info.nShow = windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        if ShellExecuteExW(&mut info) == 0 || info.hProcess.is_null() {
            return Err("administrator permission was not granted".into());
        }
        WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        GetExitCodeProcess(info.hProcess, &mut code);
        windows_sys::Win32::Foundation::CloseHandle(info.hProcess);
        Ok(code)
    }
}

mod reg {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_WRITE, REG_DWORD,
        REG_OPTION_NON_VOLATILE, REG_SZ,
    };

    fn w(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    fn open(key: &str) -> Option<HKEY> {
        let mut h: HKEY = std::ptr::null_mut();
        let r = unsafe {
            RegCreateKeyExW(HKEY_LOCAL_MACHINE, w(key).as_ptr(), 0, std::ptr::null(), REG_OPTION_NON_VOLATILE, KEY_WRITE, std::ptr::null(), &mut h, std::ptr::null_mut())
        };
        (r == 0).then_some(h)
    }

    pub fn set_str(key: &str, name: &str, value: &str) {
        if let Some(h) = open(key) {
            let v = w(value);
            unsafe {
                RegSetValueExW(h, w(name).as_ptr(), 0, REG_SZ, v.as_ptr() as _, (v.len() * 2) as u32);
                RegCloseKey(h);
            }
        }
    }

    pub fn set_dword(key: &str, name: &str, value: u32) {
        if let Some(h) = open(key) {
            unsafe {
                RegSetValueExW(h, w(name).as_ptr(), 0, REG_DWORD, &value as *const u32 as _, 4);
                RegCloseKey(h);
            }
        }
    }

    pub fn delete_tree(key: &str) {
        unsafe {
            RegDeleteTreeW(HKEY_LOCAL_MACHINE, w(key).as_ptr());
        }
    }
}
