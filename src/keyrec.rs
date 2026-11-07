//! Macro recorder that reads the keyboard's own HID input reports (hidraw), so every key is
//! captured with exact timing, including Win/Super and keys the GUI toolkit never sees.
//! hidraw reads are copies: typing keeps working normally in every application.
//!
//! Sources on the YC3121 boards (VID 3151):
//!   interface 0 - boot keyboard report, no report id: [mods, 0, k1..k6]
//!   interface 1 - report id 1 = N-key-rollover bitmap (bit n = HID usage n, 0x00..0x77)

use crate::hidraw;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use std::time::Instant;

pub struct Recorder {
    pub rx: Receiver<(u8, bool, Instant)>,
    stop: Arc<AtomicBool>,
}

impl Drop for Recorder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

struct Source {
    file: File,
    has_ids: bool,
    keys: BTreeSet<u8>,
}

/// Start recording from the keyboard whose config interface lives at `config_path`.
pub fn start(config_path: &str) -> Result<Recorder, String> {
    let all = hidraw::enumerate();
    let cfg = all.iter().find(|d| d.path == config_path).ok_or("keyboard not found")?;
    let mut sources = vec![];
    for d in all.iter().filter(|d| d.vid == cfg.vid && d.pid == cfg.pid && !d.is_config()) {
        // only interfaces that contain a keyboard collection (usage page 1, usage 6)
        if !d.descriptor.windows(4).any(|w| w == [0x05, 0x01, 0x09, 0x06]) {
            continue;
        }
        match File::open(&d.path) {
            Ok(file) => sources.push(Source { file, has_ids: d.descriptor.contains(&0x85), keys: BTreeSet::new() }),
            Err(e) => return Err(format!("{}: {e}", d.path)),
        }
    }
    if sources.is_empty() {
        return Err("no keyboard input interface".into());
    }
    let (tx, rx) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    std::thread::spawn(move || {
        let mut pressed: BTreeSet<u8> = BTreeSet::new();
        let mut buf = [0u8; 64];
        while !stop2.load(Ordering::SeqCst) {
            let mut fds: Vec<libc::pollfd> = sources.iter()
                .map(|s| libc::pollfd { fd: s.file.as_raw_fd(), events: libc::POLLIN, revents: 0 }).collect();
            let r = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, 100) };
            if r < 0 { break; }
            if r == 0 { continue; }
            for (i, f) in fds.iter().enumerate() {
                if f.revents & libc::POLLIN == 0 { continue; }
                let src = &mut sources[i];
                let Ok(n) = src.file.read(&mut buf) else { continue };
                if let Some(keys) = decode(&buf[..n], src.has_ids) { src.keys = keys; }
            }
            let now = Instant::now();
            let union: BTreeSet<u8> = sources.iter().flat_map(|s| s.keys.iter().copied()).collect();
            for k in union.difference(&pressed) { let _ = tx.send((*k, true, now)); }
            for k in pressed.difference(&union) { let _ = tx.send((*k, false, now)); }
            pressed = union;
        }
    });
    Ok(Recorder { rx, stop })
}

/// Pressed HID usages in one input report, or None if it is not a keyboard report.
fn decode(r: &[u8], has_ids: bool) -> Option<BTreeSet<u8>> {
    let mut keys = BTreeSet::new();
    if !has_ids {
        if r.len() < 8 { return None; }
        for bit in 0..8 { if r[0] & (1 << bit) != 0 { keys.insert(0xE0 + bit); } }
        for &k in &r[2..8] { if k > 3 { keys.insert(k); } }
        return Some(keys);
    }
    if r.first() != Some(&1) || r.len() < 2 { return None; }
    for (byte_i, &b) in r[1..].iter().enumerate() {
        for bit in 0..8 {
            if b & (1 << bit) != 0 {
                let usage = byte_i * 8 + bit;
                if usage > 3 && usage <= 0xFF { keys.insert(usage as u8); }
            }
        }
    }
    Some(keys)
}

#[cfg(test)]
mod tests {
    use super::decode;
    #[test]
    fn boot_report_with_win() {
        // LWin (bit 3) + 'R' (0x15)
        let k = decode(&[0x08, 0, 0x15, 0, 0, 0, 0, 0], false).unwrap();
        assert!(k.contains(&0xE3) && k.contains(&0x15) && k.len() == 2);
    }
    #[test]
    fn nkro_bitmap() {
        let mut r = vec![0u8; 16];
        r[0] = 1;
        r[1 + 0x04 / 8] |= 1 << (0x04 % 8); // A
        r[1 + 0x2C / 8] |= 1 << (0x2C % 8); // Space
        let k = decode(&r, true).unwrap();
        assert_eq!(k.into_iter().collect::<Vec<_>>(), vec![0x04, 0x2C]);
    }
}
