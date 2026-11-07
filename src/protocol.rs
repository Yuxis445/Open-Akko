//! Akko YC3121/YC500 keyboard protocol (reverse engineered from Akko Cloud v4:
//! dist/js/861cc116.js, 6eeb3436.js, a0f70cc2.js).
//!
//! 64-byte feature reports, no report id. Commands padded to >= 9 bytes with a checksum:
//!   Bit7: buf[7] = 0xFF - sum(buf[0..7]);  Bit8: buf[8] = 0xFF - sum(buf[0..8]) (LED set).

use crate::data::{tables, DeviceEntry, Layout, Model};
use crate::hidraw::{self, Hidraw, REPORT_LEN};
use crate::keycodes::{dom_code_to_hid, special_code_actions, Action};
use std::io;
use std::thread::sleep;
use std::time::Duration;

pub mod cmd {
    pub const SET_RESET: u8 = 0x02;
    pub const SET_REPORT: u8 = 0x04;
    pub const SET_PROFILE: u8 = 0x05;
    pub const SET_KBOPTION: u8 = 0x06;
    pub const SET_LEDPARAM: u8 = 0x07;
    pub const SET_SLEDPARAM: u8 = 0x08;
    pub const SET_USERPIC: u8 = 0x0C;
    pub const SET_DEBOUNCE: u8 = 0x11;
    pub const SET_SLEEPTIME: u8 = 0x12;
    pub const SET_KEYMATRIX_SIMPLE: u8 = 0x13;
    pub const SET_FN_SIMPLE: u8 = 0x15;
    pub const SET_MACRO: u8 = 0x16;
    pub const GET_REV: u8 = 0x80;
    pub const GET_REPORT: u8 = 0x84;
    pub const GET_PROFILE: u8 = 0x85;
    pub const GET_KBOPTION: u8 = 0x86;
    pub const GET_LEDPARAM: u8 = 0x87;
    pub const GET_SLEDPARAM: u8 = 0x88;
    pub const GET_KEYMATRIX: u8 = 0x89;
    pub const GET_MACRO: u8 = 0x8B;
    pub const GET_USERPIC: u8 = 0x8C;
    pub const GET_INFO: u8 = 0x8F;
    pub const GET_FN: u8 = 0x90;
    pub const GET_DEBOUNCE: u8 = 0x91;
    pub const GET_SLEEPTIME: u8 = 0x92;
}

#[derive(Clone, Copy, PartialEq)]
enum Ck {
    Bit7,
    Bit8,
}

pub const LIGHT_MODES: &[(u8, &str)] = &[
    (0, "Off"), (1, "Static"), (2, "Breath"), (3, "Neon"), (4, "Wave"), (5, "Ripple"),
    (6, "Raindrop"), (7, "Snake"), (8, "Reactive"), (9, "Converge"), (10, "Sine Wave"),
    (11, "Kaleidoscope"), (12, "Line Wave"), (13, "Custom"), (14, "Laser"), (15, "Circle Wave"),
    (16, "Dazzling"), (17, "Rain Down"), (18, "Meteor"), (19, "Reactive Off"), (23, "Train"),
    (24, "Fireworks"), (20, "Music 1"), (22, "Music 2"), (21, "Screen Sync"),
];
pub const SIDE_LIGHT_MODES: &[(u8, &str)] = &[
    (0, "Off"), (1, "Static"), (2, "Breath"), (3, "Neon"), (4, "Wave"), (5, "Snake"),
    (20, "Music 1"), (22, "Music 2"), (21, "Screen Sync"),
];
pub const REPORT_RATES: &[(u8, u32)] = &[(5, 125), (4, 500), (3, 1000), (2, 2000), (1, 4000), (0, 8000)];
pub const CUSTOM_MODE: u8 = 13;
const WHITE_SUB: u32 = 16449530; // firmware's "white"

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub mode: u8,
    pub speed: u8,      // 0..=4
    pub brightness: u8, // 0..=4
    pub color: u32,
    pub rainbow: bool,
    pub option: u8,     // direction / sub-option
}

impl Default for Light {
    fn default() -> Self {
        Self { mode: 1, speed: 3, brightness: 4, color: 0xFFFFFF, rainbow: false, option: 0 }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Options {
    pub win_lock: bool,
    pub mac_mode: bool,
    pub wasd_swap: bool,
    pub led_off: bool,
    pub side_led_off: bool,
    pub keyboard_mode: bool,
    pub keyboard_lock: bool,
    pub fn_swap: bool,
    pub power_save: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sleep {
    pub bt: u16,
    pub rf: u16,
    pub deep_bt: u16,
    pub deep_rf: u16,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MacroEvent {
    pub code: u8,
    pub down: bool,
    pub delay_ms: u16,
}

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub path: String,
    pub usb_name: String,
    pub pid: u16,
    pub device_id: i64,
    pub firmware: u16,
    pub model: String,
    pub display_name: String,
    pub supported: bool,
    pub profiles: u8,
    pub layout_name: Option<String>,
    /// "USB" or "2.4 GHz"
    pub connection: &'static str,
}

/// A physical key on screen with its matrix slot.
#[derive(Clone, Debug)]
pub struct KeySlot {
    pub index: usize,
    pub code: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub round: bool,
    pub knob_part: Option<u8>, // 0 = left (ccw), 1 = press, 2 = right (cw)
}

/// How commands reach the keyboard.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Link {
    Usb,
    /// 2.4 GHz receiver: commands are relayed with F7 (status) / F6 (route) / FC (fetch reply)
    Dongle,
}

/// Receivers that relay to a keyboard (Akko Cloud's "dongle" filter entries).
pub const DONGLE_PIDS: &[u16] = &[0x4011, 0x5007, 0x5020, 0x503F, 0x5040, 0x504B];

pub fn is_dongle(pid: u16) -> bool {
    DONGLE_PIDS.contains(&pid)
}

#[derive(Clone, Copy, Debug)]
pub struct DongleStatus {
    pub can_read: bool,
    pub can_send: bool,
    pub online: bool,
    pub battery: u8,
}

pub struct Keyboard {
    dev: Hidraw,
    pub link: Link,
    routed: std::cell::Cell<bool>,
    pub info: DeviceInfo,
    pub model: Model,
    dazzle: u8,
    normal: u8,
    maxspeed: u8,
    pub has_report_rate: bool,
    pub slots: Vec<KeySlot>,
    pub layout_size: (f32, f32),
}

fn checksum(data: &[u8], ck: Ck) -> Vec<u8> {
    let mut b = data.to_vec();
    if b.len() < 9 {
        b.resize(9, 0);
    }
    match ck {
        Ck::Bit7 => b[7] = 0xFF - (b[..7].iter().map(|&x| x as u32).sum::<u32>() & 0xFF) as u8,
        Ck::Bit8 => b[8] = 0xFF - (b[..8].iter().map(|&x| x as u32).sum::<u32>() & 0xFF) as u8,
    }
    b
}

pub fn find_config_interfaces() -> Vec<hidraw::HidInfo> {
    hidraw::enumerate().into_iter().filter(|d| d.is_config()).collect()
}

impl Keyboard {
    pub const SETTLE: Duration = Duration::from_millis(300);

    /// Wired keyboard first (faster); otherwise a 2.4 GHz receiver with the keyboard awake.
    pub fn open_first() -> io::Result<Self> {
        let mut ifs = find_config_interfaces();
        if ifs.is_empty() {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no Akko keyboard found"));
        }
        ifs.sort_by_key(|d| is_dongle(d.pid));
        let mut last = None;
        for d in &ifs {
            match Self::open(d) {
                Ok(k) => return Ok(k),
                Err(e) => last = Some(e),
            }
        }
        Err(last.unwrap())
    }

    pub fn open(h: &hidraw::HidInfo) -> io::Result<Self> {
        let dev = Hidraw::open(&h.path)?;
        let link = if is_dongle(h.pid) { Link::Dongle } else { Link::Usb };
        let mut kb = Keyboard {
            dev,
            link,
            routed: std::cell::Cell::new(false),
            info: DeviceInfo {
                path: h.path.clone(), usb_name: h.name.clone(), pid: h.pid, device_id: -1, firmware: 0,
                model: "unknown".into(), display_name: "Akko Keyboard".into(), supported: false,
                profiles: 4, layout_name: None,
                connection: if link == Link::Dongle { "2.4 GHz" } else { "USB" },
            },
            model: Model::default(),
            dazzle: 8, normal: 7, maxspeed: 5, has_report_rate: false,
            slots: vec![], layout_size: (0.0, 0.0),
        };
        if link == Link::Dongle && !kb.dongle_status()?.online {
            return Err(io::Error::new(io::ErrorKind::NotConnected,
                "2.4 GHz receiver found, but the keyboard is off or asleep: switch it to 2.4 GHz and press a key"));
        }
        kb.identify()?;
        Ok(kb)
    }

    // ------------------------------------------------------------------ io
    fn send(&self, data: &[u8], ck: Ck) -> io::Result<()> {
        sleep(Duration::from_millis(10));
        if self.link == Link::Dongle {
            self.dongle_wait(|s| s.can_send, "keyboard not ready (2.4 GHz)")?;
            if !self.routed.get() {
                self.dev.send_feature(&[0xF6, 10])?; // route to the keyboard
                sleep(Duration::from_millis(10));
                self.routed.set(true);
            }
        }
        self.dev.send_feature(&checksum(data, ck))
    }

    fn query(&self, data: &[u8]) -> io::Result<[u8; REPORT_LEN]> {
        self.send(data, Ck::Bit7)?;
        sleep(Duration::from_millis(10));
        if self.link == Link::Dongle {
            self.dongle_wait(|s| s.can_read, "no reply from keyboard (2.4 GHz)")?;
            self.dev.send_feature(&[0xFC])?; // hand over the reply
            sleep(Duration::from_millis(10));
        }
        self.dev.get_feature()
    }

    /// Receiver status (F7). Only meaningful on the 2.4 GHz link.
    pub fn dongle_status(&self) -> io::Result<DongleStatus> {
        self.dev.send_feature(&[0xF7])?;
        sleep(Duration::from_millis(10));
        let r = self.dev.get_feature()?;
        let kb = r[6] == 1 || r[6] == 3;
        Ok(DongleStatus {
            can_read: kb && r[0] == 1,
            can_send: r[5] == 1,
            online: kb && r[3] == 0,
            battery: r[1],
        })
    }

    fn dongle_wait(&self, ok: impl Fn(&DongleStatus) -> bool, what: &str) -> io::Result<()> {
        for _ in 0..50 {
            let st = self.dongle_status()?;
            if !st.online {
                return Err(io::Error::new(io::ErrorKind::NotConnected, "keyboard went to sleep (2.4 GHz): press a key to wake it"));
            }
            if ok(&st) {
                return Ok(());
            }
            sleep(Duration::from_millis(20));
        }
        Err(io::Error::new(io::ErrorKind::TimedOut, what.to_string()))
    }

    /// Median of `n` quick battery readings (the keyboard's own estimate is noisy).
    pub fn battery_median(&self, n: usize) -> Option<u8> {
        let mut v: Vec<u8> = (0..n.max(1)).filter_map(|i| {
            if i > 0 { sleep(Duration::from_millis(150)); }
            self.battery()
        }).collect();
        if v.is_empty() { return None; }
        v.sort();
        Some(v[v.len() / 2])
    }

    /// Battery % (2.4 GHz only), single raw reading.
    pub fn battery(&self) -> Option<u8> {
        if self.link != Link::Dongle { return None; }
        self.dongle_status().ok().filter(|s| s.online).map(|s| s.battery)
    }

    pub fn raw(&self, data: &[u8]) -> io::Result<[u8; REPORT_LEN]> {
        self.query(data)
    }

    fn settle(&self) {
        sleep(Self::SETTLE);
    }

    // ------------------------------------------------------------ identity
    fn identify(&mut self) -> io::Result<()> {
        let r = self.query(&[cmd::GET_INFO])?;
        let mut id = -1i64;
        if r[0] == cmd::GET_INFO {
            let v = u32::from_le_bytes([r[1], r[2], r[3], r[4]]);
            if v != u32::MAX {
                id = v as i64;
            }
        }
        self.info.device_id = id;
        let t = tables();
        let entry: Option<&DeviceEntry> = t.devices.iter().find(|d| d.id == id);
        let model_name = entry.and_then(|e| e.name.clone()).unwrap_or_default();
        self.model = t.models.get(&model_name).or_else(|| t.models.get("_generic")).cloned().unwrap_or_default();
        if let Some(e) = entry {
            self.info.model = model_name.clone();
            let dn = e.display_name.clone().unwrap_or_default();
            self.info.display_name = if dn.trim().is_empty() { model_name.clone() } else { dn.trim().to_string() };
            self.info.supported = e.supported;
            self.info.profiles = e.layer.unwrap_or(4).clamp(1, 4) as u8;
            self.info.layout_name = e.key_layout.clone();
        }
        self.dazzle = self.model.dazzle.unwrap_or(8);
        self.normal = self.model.normal.unwrap_or(7);
        self.maxspeed = self.model.maxspeed.unwrap_or(5);
        self.has_report_rate = self.model.report_rate;
        let rev = self.model.get_rev.unwrap_or(cmd::GET_REV);
        let r = self.query(&[rev])?;
        if r[0] == rev {
            self.info.firmware = u16::from_le_bytes([r[1], r[2]]);
        }
        self.build_slots();
        Ok(())
    }

    fn matrix_entry(&self, idx: usize) -> Action {
        let m = &self.model.default_matrix;
        if idx * 4 + 3 < m.len() { [m[idx * 4], m[idx * 4 + 1], m[idx * 4 + 2], m[idx * 4 + 3]] } else { [0; 4] }
    }

    pub fn default_action(&self, idx: usize) -> Action {
        self.matrix_entry(idx)
    }

    pub fn default_fn_action(&self, idx: usize) -> Action {
        let m = &self.model.default_fn_matrix;
        if idx * 4 + 3 < m.len() { [m[idx * 4], m[idx * 4 + 1], m[idx * 4 + 2], m[idx * 4 + 3]] } else { [0; 4] }
    }

    fn slot_count(&self) -> usize {
        self.model.default_matrix.len() / 4
    }

    fn find_action(&self, a: Action, nth: usize) -> Option<usize> {
        (0..self.slot_count()).filter(|&i| self.matrix_entry(i) == a).nth(nth)
    }

    fn build_slots(&mut self) {
        let t = tables();
        let layout: Option<&Layout> = self.info.layout_name.as_ref().and_then(|n| t.layouts.get(n));
        let (slots, size) = build_slots(&self.model, layout);
        self.slots = slots;
        self.layout_size = size;
    }

    // ------------------------------------------------------------ lighting
    fn light_params(&self) -> LightParams<'_> {
        LightParams { dazzle: self.dazzle, normal: self.normal, maxspeed: self.maxspeed, common: &self.model.common_color }
    }

    fn encode_light(&self, command: u8, s: &Light, side: bool) -> Vec<u8> {
        encode_light(&self.light_params(), command, s, side)
    }

    fn decode_light(&self, r: &[u8], side: bool) -> Light {
        decode_light(&self.light_params(), r, side)
    }

    pub fn get_light(&self) -> io::Result<Light> {
        Ok(self.decode_light(&self.query(&[cmd::GET_LEDPARAM])?, false))
    }

    pub fn set_light(&self, s: &Light) -> io::Result<()> {
        self.send(&self.encode_light(cmd::SET_LEDPARAM, s, false), Ck::Bit8)?;
        self.settle();
        Ok(())
    }

    pub fn get_side_light(&self) -> io::Result<Option<Light>> {
        let r = self.query(&[cmd::GET_SLEDPARAM])?;
        if r[0] != cmd::GET_SLEDPARAM || !SIDE_LIGHT_MODES.iter().any(|(m, _)| *m == r[1]) {
            return Ok(None);
        }
        Ok(Some(self.decode_light(&r, true)))
    }

    pub fn set_side_light(&self, s: &Light) -> io::Result<()> {
        self.send(&self.encode_light(cmd::SET_SLEDPARAM, s, true), Ck::Bit8)?;
        self.settle();
        Ok(())
    }

    /// Per-key colours for the Custom lighting mode: one RGB triple per matrix slot.
    pub fn get_key_colors(&self, profile: u8) -> io::Result<Vec<u32>> {
        let mut raw = vec![];
        for page in 0..6u8 {
            raw.extend_from_slice(&self.query(&[cmd::GET_USERPIC, profile, page])?);
        }
        let n = self.slot_count();
        Ok((0..n).map(|i| {
            let o = i * 3;
            if o + 2 < raw.len() { ((raw[o] as u32) << 16) | ((raw[o + 1] as u32) << 8) | raw[o + 2] as u32 } else { 0 }
        }).collect())
    }

    pub fn set_key_colors(&self, profile: u8, colors: &[u32]) -> io::Result<()> {
        let n = self.slot_count();
        let mut data = vec![0u8; n * 3];
        for (i, c) in colors.iter().enumerate().take(n) {
            data[i * 3] = (c >> 16) as u8;
            data[i * 3 + 1] = (c >> 8) as u8;
            data[i * 3 + 2] = *c as u8;
        }
        let total = 64 * ((n * 3) / 64);
        for page in 0..7usize {
            let mut pkt = vec![cmd::SET_USERPIC, profile, (total & 255) as u8, (total >> 8) as u8, page as u8, 0, 0, 0];
            let mut chunk: Vec<u8> = data.iter().skip(page * 56).take(56).copied().collect();
            chunk.resize(56, 0);
            pkt.extend(chunk);
            self.send(&pkt, Ck::Bit7)?;
        }
        self.settle();
        Ok(())
    }

    // ----------------------------------------------------------- settings
    pub fn get_profile(&self) -> io::Result<u8> {
        Ok(self.query(&[cmd::GET_PROFILE])?[1])
    }

    pub fn set_profile(&self, p: u8) -> io::Result<()> {
        self.send(&[cmd::SET_PROFILE, p], Ck::Bit7)?;
        self.settle();
        Ok(())
    }

    pub fn get_debounce(&self) -> io::Result<u8> {
        Ok(self.query(&[cmd::GET_DEBOUNCE])?[2])
    }

    pub fn set_debounce(&self, ms: u8) -> io::Result<()> {
        self.send(&[cmd::SET_DEBOUNCE, 0, ms], Ck::Bit7)?;
        self.settle();
        Ok(())
    }

    pub fn get_options(&self, profile: u8) -> io::Result<Options> {
        let r = self.query(&[cmd::GET_KBOPTION, profile])?;
        let b = r[2];
        Ok(Options {
            win_lock: b & 1 != 0, mac_mode: b & 6 != 0, wasd_swap: b & 8 != 0, led_off: b & 16 != 0,
            side_led_off: b & 32 != 0, keyboard_mode: b & 64 != 0, keyboard_lock: b & 128 != 0,
            fn_swap: r[3] & 1 != 0, power_save: r[4] != 0,
        })
    }

    pub fn set_options(&self, profile: u8, o: &Options) -> io::Result<()> {
        // bit layout as written by the official app (mac flag at bit 2)
        let b = (o.win_lock as u8) | (o.mac_mode as u8) << 2 | (o.wasd_swap as u8) << 3 | (o.led_off as u8) << 4
            | (o.side_led_off as u8) << 5 | (o.keyboard_mode as u8) << 6 | (o.keyboard_lock as u8) << 7;
        self.send(&[cmd::SET_KBOPTION, profile, b, o.fn_swap as u8, o.power_save as u8], Ck::Bit7)?;
        self.settle();
        Ok(())
    }

    pub fn get_sleep(&self) -> io::Result<Sleep> {
        let r = self.query(&[cmd::GET_SLEEPTIME])?;
        let u = |i: usize| u16::from_le_bytes([r[i], r[i + 1]]);
        Ok(Sleep { bt: u(1), rf: u(3), deep_bt: u(5), deep_rf: u(7) })
    }

    pub fn set_sleep(&self, s: &Sleep) -> io::Result<()> {
        let mut e = vec![0u8; 64];
        e[0] = cmd::SET_SLEEPTIME;
        for (off, v) in [(8, s.bt), (10, s.rf), (12, s.deep_bt), (14, s.deep_rf)] {
            e[off..off + 2].copy_from_slice(&v.to_le_bytes());
        }
        self.send(&e, Ck::Bit7)?;
        self.settle();
        Ok(())
    }

    pub fn get_report_rate(&self) -> io::Result<Option<u32>> {
        let r = self.query(&[cmd::GET_REPORT, 0])?;
        Ok(if r[0] == cmd::GET_REPORT { REPORT_RATES.iter().find(|(c, _)| *c == r[2]).map(|x| x.1) } else { None })
    }

    pub fn set_report_rate(&self, hz: u32) -> io::Result<()> {
        if let Some((c, _)) = REPORT_RATES.iter().find(|(_, h)| *h == hz) {
            self.query(&[cmd::SET_REPORT, 0, *c])?;
            self.settle();
        }
        Ok(())
    }

    pub fn factory_reset(&self) -> io::Result<()> {
        self.send(&[cmd::SET_RESET], Ck::Bit7)?;
        sleep(Duration::from_secs(4));
        Ok(())
    }

    // ------------------------------------------------------------- keymap
    fn read_matrix(&self, command: u8, profile: u8) -> io::Result<Vec<Action>> {
        let mut raw = vec![];
        for page in 0..8u8 {
            raw.extend_from_slice(&self.query(&[command, profile, page])?);
        }
        Ok((0..self.slot_count()).map(|i| [raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]]).collect())
    }

    pub fn get_keymap(&self, profile: u8) -> io::Result<Vec<Action>> {
        self.read_matrix(cmd::GET_KEYMATRIX, profile)
    }

    pub fn get_fn_keymap(&self, profile: u8) -> io::Result<Vec<Action>> {
        self.read_matrix(cmd::GET_FN, profile)
    }

    /// Note: firmware writes to the ACTIVE profile; `profile` must equal it (verified on 5075B Plus fw 0x0104).
    pub fn set_key(&self, profile: u8, index: usize, a: Action) -> io::Result<()> {
        let mut e = vec![0u8; 64];
        e[0] = cmd::SET_KEYMATRIX_SIMPLE;
        e[1] = profile;
        e[2] = index as u8;
        e[8..12].copy_from_slice(&a);
        self.send(&e, Ck::Bit7)?;
        self.settle();
        Ok(())
    }

    pub fn set_fn_key(&self, profile: u8, index: usize, a: Action) -> io::Result<()> {
        let mut e = vec![0u8; 64];
        e[0] = cmd::SET_FN_SIMPLE;
        e[1] = profile;
        e[2] = index as u8;
        e[8..12].copy_from_slice(&a);
        self.send(&e, Ck::Bit7)?;
        self.settle();
        Ok(())
    }

    // ------------------------------------------------------------- macros
    pub fn set_macro(&self, slot: u8, events: &[MacroEvent], repeat: u16) -> io::Result<()> {
        let buf = encode_macro(events, repeat).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let used = (0..5).filter(|p| buf[p * 56..(p + 1) * 56].iter().any(|&b| b != 0)).count().max(1);
        for p in 0..used {
            let mut pkt = vec![cmd::SET_MACRO, slot, p as u8, 56, (p == used - 1) as u8, 0, 0, 0];
            pkt.extend_from_slice(&buf[p * 56..(p + 1) * 56]);
            self.send(&pkt, Ck::Bit7)?;
        }
        self.settle();
        Ok(())
    }

    pub fn get_macro(&self, slot: u8) -> io::Result<(u16, Vec<MacroEvent>)> {
        let mut raw = vec![];
        for page in 0..4u8 {
            let r = self.query(&[cmd::GET_MACRO, slot, page])?;
            raw.extend_from_slice(&r);
            if r.windows(4).any(|w| w == [0, 0, 0, 0]) {
                break;
            }
        }
        Ok(decode_macro(&raw))
    }
}

// ======================================================================= pure helpers
// (no device access; unit tested below)

pub struct LightParams<'a> {
    pub dazzle: u8,
    pub normal: u8,
    pub maxspeed: u8,
    pub common: &'a std::collections::HashMap<String, u32>,
}

pub fn encode_light(p: &LightParams, command: u8, s: &Light, side: bool) -> Vec<u8> {
    let mut e = vec![0u8; 64];
    e[0] = command;
    let mut sub = (s.option << 4) | if s.rainbow { p.dazzle } else { p.normal };
    if s.mode == 20 || s.mode == 22 {
        sub = (s.option << 4) | if s.rainbow { 0 } else { 4 };
    }
    if s.mode == 21 {
        sub = 0;
    }
    if !side && s.mode == CUSTOM_MODE {
        sub = s.option << 4;
    }
    if side && s.mode == 3 {
        sub = 8;
    }
    e[1] = s.mode;
    e[2] = if side { s.speed } else { p.maxspeed.saturating_sub(s.speed) };
    e[3] = s.brightness;
    e[4] = sub;
    let rgb = if s.color == 0xFFFFFF { WHITE_SUB } else { s.color };
    e[5] = (rgb >> 16) as u8;
    e[6] = (rgb >> 8) as u8;
    e[7] = rgb as u8;
    if !side && s.mode == CUSTOM_MODE {
        e[5..8].copy_from_slice(&[0, 200, 200]);
    }
    e
}

pub fn decode_light(p: &LightParams, r: &[u8], side: bool) -> Light {
    let sub = r[4];
    let low = sub & 15;
    let mut color = ((r[5] as u32) << 16) | ((r[6] as u32) << 8) | r[7] as u32;
    if low != p.dazzle && low != p.normal {
        if let Some(c) = p.common.get(&low.to_string()) {
            color = *c;
        }
    }
    if color == WHITE_SUB {
        color = 0xFFFFFF;
    }
    let rainbow = if r[1] == 20 || r[1] == 22 { low == 0 } else { low == p.dazzle };
    Light {
        mode: r[1],
        speed: if side { r[2] } else { p.maxspeed.saturating_sub(r[2]) }.min(4),
        brightness: r[3].min(4),
        color,
        rainbow,
        option: sub >> 4,
    }
}

/// Max macro payload the firmware stores (repeat + events) - official app uses a 256-byte buffer.
pub const MACRO_MAX_BYTES: usize = 256;

/// Macro wire format: [repeat u16 LE] then per event either
///   [code, delay(1..=127) | 0x80 if down]                 (short)
///   [code, 0x80 if down else 0, delay lo, delay hi]       (long, delay >= 128)
/// A zero delay would be read back as the long form, so delays are stored as >= 1 ms.
pub fn encode_macro(events: &[MacroEvent], repeat: u16) -> Result<Vec<u8>, String> {
    let mut buf = repeat.to_le_bytes().to_vec();
    for e in events {
        let d = e.delay_ms.max(1);
        if d <= 127 {
            buf.push(e.code);
            buf.push(if e.down { d as u8 + 128 } else { d as u8 });
        } else {
            buf.extend([e.code, if e.down { 128 } else { 0 }, (d & 255) as u8, (d >> 8) as u8]);
        }
    }
    if buf.len() + 4 > MACRO_MAX_BYTES {
        return Err(format!("macro too long: {} bytes, the keyboard stores at most {}", buf.len(), MACRO_MAX_BYTES - 4));
    }
    buf.resize(5 * 56, 0);
    Ok(buf)
}

pub fn decode_macro(raw: &[u8]) -> (u16, Vec<MacroEvent>) {
    if raw.len() < 2 {
        return (0, vec![]);
    }
    let repeat = u16::from_le_bytes([raw[0], raw[1]]);
    let mut ev = vec![];
    let mut i = 2;
    while i + 1 < raw.len() {
        let (code, b) = (raw[i], raw[i + 1]);
        if code == 0 && b == 0 {
            break;
        }
        let down = b >= 128;
        let delay = if b & 127 != 0 {
            i += 2;
            (b & 127) as u16
        } else {
            let d = u16::from_le_bytes([*raw.get(i + 2).unwrap_or(&0), *raw.get(i + 3).unwrap_or(&0)]);
            i += 4;
            d
        };
        ev.push(MacroEvent { code, down, delay_ms: delay });
    }
    (repeat, ev)
}

fn matrix_at(m: &[u8], idx: usize) -> Action {
    if idx * 4 + 3 < m.len() { [m[idx * 4], m[idx * 4 + 1], m[idx * 4 + 2], m[idx * 4 + 3]] } else { [0; 4] }
}

/// Map a layout drawing onto matrix slots (falls back to a grid when no drawing exists).
pub fn build_slots(model: &Model, layout: Option<&Layout>) -> (Vec<KeySlot>, (f32, f32)) {
    let m = &model.default_matrix;
    let n = m.len() / 4;
    let find = |a: Action, nth: usize| (0..n).filter(|&i| matrix_at(m, i) == a).nth(nth);
    let mut slots = vec![];
    let mut size = (0.0, 0.0);
    if let Some(l) = layout {
        let mut seen: std::collections::HashMap<String, usize> = Default::default();
        for k in &l.keys {
            let base = k.code.split("_index").next().unwrap_or(&k.code).to_string();
            let nth = { let e = seen.entry(base).or_insert(0); let v = *e; *e += 1; v };
            let idx = if let Some(h) = dom_code_to_hid(&k.code) {
                find([0, 0, h, 0], nth)
            } else {
                special_code_actions(&k.code).iter().find_map(|a| find(*a, 0))
            };
            let Some(index) = idx else { continue };
            let knob_part = match k.code.as_str() {
                "AudioVolumeDown" if k.round => Some(0),
                "AudioVolumeUp" if k.round => Some(2),
                _ if k.round => Some(1),
                _ => None,
            };
            slots.push(KeySlot { index, code: k.code.clone(), x: k.x, y: k.y, w: k.w, h: k.h, round: k.round, knob_part });
        }
        size = (l.width, l.height);
    }
    if slots.is_empty() {
        // matrix is column-major with 6 rows per column
        for i in 0..n {
            if matrix_at(m, i) == [0; 4] {
                continue;
            }
            let (col, row) = ((i / 6) as f32, (i % 6) as f32);
            slots.push(KeySlot { index: i, code: format!("#{i}"), x: col * 44.0, y: row * 44.0, w: 40.0, h: 40.0, round: false, knob_part: None });
        }
        let w = slots.iter().map(|s| s.x + s.w).fold(0.0, f32::max);
        let h = slots.iter().map(|s| s.y + s.h).fold(0.0, f32::max);
        size = (w, h);
    }
    (slots, size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tables;

    #[test]
    fn checksum_matches_official_formula() {
        let b = checksum(&[0x8F], Ck::Bit7);
        assert_eq!(b.len(), 9);
        assert_eq!(b[7], 0x70); // 0xFF - 0x8F
        let b = checksum(&[0x07, 4, 1, 4, 0x47, 0xFF, 0, 0], Ck::Bit8);
        assert_eq!(b[..8].iter().map(|&x| x as u32).sum::<u32>() + b[8] as u32 & 0xFF, 0xFF);
        // long commands keep their length
        assert_eq!(checksum(&[0u8; 64], Ck::Bit7).len(), 64);
    }

    fn params(common: &std::collections::HashMap<String, u32>) -> LightParams<'_> {
        LightParams { dazzle: 7, normal: 8, maxspeed: 5, common }
    }

    #[test]
    fn light_roundtrip_all_main_modes() {
        let common = Default::default();
        let p = params(&common);
        for (mode, _) in LIGHT_MODES {
            if *mode == CUSTOM_MODE || *mode == 21 { continue; } // colour fixed by firmware
            for &(color, rainbow) in &[(0xFF0000u32, false), (0x12AB34, false), (0xFFFFFF, false), (0x00FF00, true)] {
                for speed in 0..=4 {
                    let l = Light { mode: *mode, speed, brightness: 3, color, rainbow, option: 1 };
                    let e = encode_light(&p, cmd::SET_LEDPARAM, &l, false);
                    let d = decode_light(&p, &e, false);
                    assert_eq!((d.mode, d.speed, d.brightness, d.rainbow), (l.mode, l.speed, l.brightness, l.rainbow), "mode {mode}");
                    if !rainbow { assert_eq!(d.color, color, "mode {mode}"); }
                }
            }
        }
    }

    #[test]
    fn light_speed_is_inverted_on_main_not_side() {
        let common = Default::default();
        let p = params(&common);
        let l = Light { mode: 4, speed: 4, brightness: 2, color: 0, rainbow: false, option: 0 };
        assert_eq!(encode_light(&p, cmd::SET_LEDPARAM, &l, false)[2], 1); // 5 - 4
        assert_eq!(encode_light(&p, cmd::SET_SLEDPARAM, &l, true)[2], 4);
    }

    #[test]
    fn white_uses_firmware_white() {
        let common = Default::default();
        let p = params(&common);
        let l = Light { color: 0xFFFFFF, ..Light::default() };
        let e = encode_light(&p, cmd::SET_LEDPARAM, &l, false);
        assert_eq!(&e[5..8], &[0xFA, 0xFF, 0xFA]);
        assert_eq!(decode_light(&p, &e, false).color, 0xFFFFFF);
    }

    fn ev(code: u8, down: bool, delay_ms: u16) -> MacroEvent { MacroEvent { code, down, delay_ms } }

    #[test]
    fn macro_roundtrip_short_and_long_delays() {
        let events = vec![ev(0xE0, true, 20), ev(0x06, true, 127), ev(0x06, false, 128), ev(0xE0, false, 5000), ev(0x04, true, 65535)];
        let buf = encode_macro(&events, 3).unwrap();
        let (rep, back) = decode_macro(&buf);
        assert_eq!(rep, 3);
        assert_eq!(back, events);
    }

    #[test]
    fn macro_zero_delay_is_stored_as_1ms() {
        let buf = encode_macro(&[ev(0x04, true, 0), ev(0x04, false, 0), ev(0x05, true, 10)], 1).unwrap();
        let (_, back) = decode_macro(&buf);
        assert_eq!(back, vec![ev(0x04, true, 1), ev(0x04, false, 1), ev(0x05, true, 10)]);
    }

    #[test]
    fn macro_too_long_is_rejected() {
        let many: Vec<_> = (0..200).map(|i| ev(0x04, i % 2 == 0, 10)).collect();
        assert!(encode_macro(&many, 1).is_err());
        let ok: Vec<_> = (0..100).map(|i| ev(0x04, i % 2 == 0, 10)).collect();
        assert!(encode_macro(&ok, 1).is_ok());
    }

    #[test]
    fn every_supported_akko_model_maps_its_layout() {
        let t = tables();
        let mut checked = 0;
        for d in t.devices.iter().filter(|d| d.supported) {
            let name = d.name.clone().unwrap();
            let model = &t.models[&name];
            assert_eq!(model.default_matrix.len(), 512, "{name}: matrix size");
            let layout = d.key_layout.as_ref().and_then(|l| t.layouts.get(l));
            let (slots, (w, h)) = build_slots(model, layout);
            assert!(w > 0.0 && h > 0.0, "{name}: empty layout");
            assert!(slots.len() >= 60, "{name}: only {} keys mapped", slots.len());
            let mut idx: Vec<usize> = slots.iter().filter(|s| s.knob_part.is_none()).map(|s| s.index).collect();
            let n = idx.len();
            idx.sort();
            idx.dedup();
            assert_eq!(n, idx.len(), "{name}: two keys share a slot");
            assert!(slots.iter().all(|s| s.index < 128), "{name}: slot out of range");
            if let Some(l) = layout {
                let mapped = slots.len() as f32 / l.keys.len() as f32;
                assert!(mapped > 0.9, "{name}: only {:.0}% of drawn keys mapped", mapped * 100.0);
            }
            checked += 1;
        }
        assert!(checked >= 15, "only {checked} models checked");
    }

    #[test]
    fn fivethousand75b_layout_is_exact() {
        let t = tables();
        let model = &t.models["yc500_5075bplus_soc"];
        let (slots, _) = build_slots(model, t.layouts.get("Common82_5075B"));
        assert_eq!(slots.len(), 85); // 82 keys + 3 knob parts
        let find = |c: &str| slots.iter().find(|s| s.code == c).map(|s| s.index);
        assert_eq!(find("Escape"), Some(0));
        assert_eq!(find("CapsLock"), Some(3));
        assert_eq!(find("KeyP"), Some(62));
        assert_eq!(find("Fn"), Some(59));
        assert_eq!(find("AudioVolumeDown"), Some(90));
        assert_eq!(find("AudioVolumeUp"), Some(91));
    }

    #[test]
    fn dongle_pids() {
        assert!(is_dongle(0x4011));
        assert!(!is_dongle(0x4015));
    }
}
