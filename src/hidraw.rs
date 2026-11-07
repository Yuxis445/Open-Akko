//! Linux hidraw access: enumeration via sysfs and feature-report ioctls.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::path::Path;

pub const VENDOR_ROYUAN: u16 = 0x3151;
pub const REPORT_LEN: usize = 64;

#[derive(Clone, Debug)]
pub struct HidInfo {
    pub path: String,
    pub vid: u16,
    pub pid: u16,
    pub name: String,
    pub interface: i32,
    pub descriptor: Vec<u8>,
}

impl HidInfo {
    /// Vendor configuration channel: usage page 0xFFFF with a feature report.
    pub fn is_config(&self) -> bool {
        self.descriptor.starts_with(&[0x06, 0xFF, 0xFF]) && self.descriptor.contains(&0xB1)
    }
}

pub fn enumerate() -> Vec<HidInfo> {
    let mut out = vec![];
    let Ok(rd) = fs::read_dir("/sys/class/hidraw") else { return out };
    let mut names: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    for n in names {
        let dev = Path::new("/sys/class/hidraw").join(&n).join("device");
        let Ok(uevent) = fs::read_to_string(dev.join("uevent")) else { continue };
        let kv: HashMap<&str, &str> = uevent.lines().filter_map(|l| l.split_once('=')).collect();
        let id: Vec<&str> = kv.get("HID_ID").copied().unwrap_or("").split(':').collect();
        if id.len() != 3 {
            continue;
        }
        let vid = u32::from_str_radix(id[1], 16).unwrap_or(0) as u16;
        let pid = u32::from_str_radix(id[2], 16).unwrap_or(0) as u16;
        if vid != VENDOR_ROYUAN {
            continue;
        }
        let phys = kv.get("HID_PHYS").copied().unwrap_or("");
        let interface = phys.rsplit_once("input").and_then(|(_, i)| i.parse().ok()).unwrap_or(-1);
        out.push(HidInfo {
            path: format!("/dev/{n}"),
            vid,
            pid,
            name: kv.get("HID_NAME").copied().unwrap_or("").to_string(),
            interface,
            descriptor: fs::read(dev.join("report_descriptor")).unwrap_or_default(),
        });
    }
    out
}

fn ioc(nr: u64, len: usize) -> u64 {
    (3u64 << 30) | ((len as u64) << 16) | ((b'H' as u64) << 8) | nr
}

pub struct Hidraw {
    file: File,
}

impl Hidraw {
    pub fn open(path: &str) -> io::Result<Self> {
        Ok(Self { file: OpenOptions::new().read(true).write(true).open(path)? })
    }

    pub fn send_feature(&self, data: &[u8]) -> io::Result<()> {
        let mut buf = vec![0u8; REPORT_LEN + 1];
        let n = data.len().min(REPORT_LEN);
        buf[1..1 + n].copy_from_slice(&data[..n]);
        let r = unsafe { libc::ioctl(self.file.as_raw_fd(), ioc(0x06, buf.len()) as _, buf.as_mut_ptr()) };
        if r < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    pub fn get_feature(&self) -> io::Result<[u8; REPORT_LEN]> {
        let mut buf = vec![0u8; REPORT_LEN + 1];
        let r = unsafe { libc::ioctl(self.file.as_raw_fd(), ioc(0x07, buf.len()) as _, buf.as_mut_ptr()) };
        if r < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut out = [0u8; REPORT_LEN];
        out.copy_from_slice(&buf[1..]);
        Ok(out)
    }
}
