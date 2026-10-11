//! hwcast mod API client: subscribes to hwcast's Server-Sent Events stream (`/events`),
//! which pushes one JSON snapshot per second. Plain HTTP over localhost, no dependencies.

use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Calls `on_snapshot` for every snapshot until the connection drops or the callback
/// returns false; returns the reason it stopped.
pub fn subscribe(addr: &str, mut on_snapshot: impl FnMut(&Value) -> bool) -> String {
    let stream = match TcpStream::connect(addr) {
        Ok(s) => s,
        Err(e) => return format!("cannot reach hwcast at {addr}: {e}"),
    };
    // hwcast sends a comment ping every 15 s, so 40 s of silence means the stream is dead.
    let _ = stream.set_read_timeout(Some(Duration::from_secs(40)));
    let mut w = match stream.try_clone() {
        Ok(s) => s,
        Err(e) => return e.to_string(),
    };
    let req = format!("GET /events HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\nUser-Agent: hwcast-gcb200\r\n\r\n");
    if let Err(e) = w.write_all(req.as_bytes()) {
        return e.to_string();
    }
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    let mut in_body = false;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => return "hwcast closed the stream".into(),
            Ok(_) => {}
            Err(e) => return format!("stream error: {e}"),
        }
        let l = line.trim_end();
        if !in_body {
            if l.starts_with("HTTP/") && !l.contains(" 200") {
                return format!("hwcast answered: {l}");
            }
            if l.is_empty() {
                in_body = true;
            }
            continue;
        }
        if let Some(data) = l.strip_prefix("data: ") {
            if let Ok(v) = serde_json::from_str::<Value>(data) {
                if !on_snapshot(&v) {
                    return "stopped by the display side".into();
                }
            }
        }
    }
}

/// One snapshot, for menus and status (the first event on the stream).
pub fn snapshot(addr: &str) -> Option<Value> {
    let mut out = None;
    subscribe(addr, |v| {
        out = Some(v.clone());
        false
    });
    out
}