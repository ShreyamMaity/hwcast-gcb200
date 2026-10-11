//! The GCB200 display: a WCH CH552 behind USB serial (VID 1A86, PID 5722) driving a
//! Holtek HT1621B segment LCD. Protocol reverse engineered from the vendor app:
//!
//!   6-byte header (Turing Smart Screen rev A layout) with command 169, then 14 bytes:
//!   [left tens, left ones, right tens, right ones, left bar 0-7, right bar 0-7, 0 x 8]
//!
//! Digits above 9 and bars above 7 read past the firmware's lookup table, so values are clamped.

use serialport::{SerialPort, SerialPortType};
use std::io::Write;
use std::time::Duration;

pub const VID: u16 = 0x1A86;
pub const PID: u16 = 0x5722;
const CMD_DATA: u8 = 169;
const CMD_HELLO: u8 = 255;

fn header(cmd: u8, x: u16, y: u16, ex: u16, ey: u16) -> [u8; 6] {
    [
        (x >> 2) as u8,
        (((x & 3) << 6) + (y >> 4)) as u8,
        (((y & 15) << 4) + (ex >> 6)) as u8,
        (((ex & 63) << 2) + (ey >> 8)) as u8,
        (ey & 255) as u8,
        cmd,
    ]
}

/// What one side of the display shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Side {
    /// 0-99, shown as two digits.
    pub value: u8,
    /// 0-7 lit bar blocks.
    pub bar: u8,
}

pub fn frame(left: Side, right: Side) -> [u8; 20] {
    let l = left.value.min(99);
    let r = right.value.min(99);
    let mut f = [0u8; 20];
    f[..6].copy_from_slice(&header(CMD_DATA, 0, 0, 0, 0));
    f[6] = l / 10;
    f[7] = l % 10;
    f[8] = r / 10;
    f[9] = r % 10;
    f[10] = left.bar.min(7);
    f[11] = right.bar.min(7);
    f
}

/// Finds the display's COM port by its USB ID, unless one is configured.
pub fn find_port(configured: Option<&str>) -> Option<String> {
    if let Some(p) = configured {
        return Some(p.to_string());
    }
    serialport::available_ports().ok()?.into_iter().find_map(|p| match p.port_type {
        SerialPortType::UsbPort(u) if u.vid == VID && u.pid == PID => Some(p.port_name),
        _ => None,
    })
}

pub struct Display {
    port: Box<dyn SerialPort>,
    pub name: String,
}

impl Display {
    pub fn open(name: &str) -> Result<Self, String> {
        let mut port = serialport::new(name, 115_200)
            .timeout(Duration::from_millis(2000))
            .open()
            .map_err(|e| format!("cannot open {name}: {e}"))?;
        // The vendor app enables both; the board expects them.
        let _ = port.write_data_terminal_ready(true);
        let _ = port.write_request_to_send(true);
        std::thread::sleep(Duration::from_millis(300));
        let mut d = Display { port, name: name.to_string() };
        d.write(&header(CMD_HELLO, 0, 0, 0, 0))?;
        Ok(d)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.port.write_all(bytes).and_then(|_| self.port.flush()).map_err(|e| format!("write to {} failed: {e}", self.name))
    }

    pub fn show(&mut self, left: Side, right: Side) -> Result<(), String> {
        self.write(&frame(left, right))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_matches_captured_vendor_layout() {
        // Captured from the vendor app: CPU 69, GPU 46, bars 2/2 gives body 06 09 04 06 02 02
        let f = frame(Side { value: 69, bar: 2 }, Side { value: 46, bar: 2 });
        assert_eq!(&f[..6], &[0, 0, 0, 0, 0, 169]);
        assert_eq!(&f[6..12], &[6, 9, 4, 6, 2, 2]);
        assert!(f[12..].iter().all(|b| *b == 0));
    }

    #[test]
    fn values_are_clamped_to_the_firmware_tables() {
        let f = frame(Side { value: 150, bar: 12 }, Side { value: 5, bar: 0 });
        assert_eq!(&f[6..12], &[9, 9, 0, 5, 7, 0]);
    }
}
