//! Background thread that owns the keyboard: all USB I/O happens here so the UI never blocks.

use crate::keycodes::Action;
use crate::protocol::*;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

pub enum Req {
    LoadState,
    /// base layer of `profile` + Fn layer `fn_index` (Fn layers are NOT per profile)
    LoadKeymaps { profile: u8, fn_index: u8 },
    LoadKeyColors(u8),
    SetKey { fn_layer: bool, profile: u8, index: usize, action: Action },
    SetLight(Light),
    SetSideLight(Light),
    SetKeyColors { profile: u8, colors: Vec<u32> },
    SetProfile(u8),
    SetDebounce(u8),
    SetOptions(u8, Options),
    SetSleep(Sleep),
    SetReportRate(u32),
    /// Pick a macro slot that is free on EVERY profile and Fn layer (or reuse the key's own),
    /// write the macro there and bind the key to it.
    AssignMacro { fn_layer: bool, layer: u8, index: usize, mode: u8, events: Vec<MacroEvent>, repeat: u16, profiles: u8 },
    FactoryReset,
}

#[derive(Clone)]
pub struct Connected {
    pub info: DeviceInfo,
    pub slots: Vec<KeySlot>,
    pub layout_size: (f32, f32),
    pub has_report_rate: bool,
    pub defaults: Vec<Action>,
    pub fn_defaults: Vec<Action>,
}

#[derive(Clone)]
pub struct State {
    pub profile: u8,
    pub light: Light,
    pub side: Option<Light>,
    pub debounce: u8,
    pub options: Options,
    pub sleep: Sleep,
    pub report_rate: Option<u32>,
}

pub enum Resp {
    Connected(Box<Connected>),
    Disconnected(String),
    State(Box<State>),
    Keymaps { profile: u8, fn_index: u8, base: Vec<Action>, fn_layer: Vec<Action> },
    KeyColors { profile: u8, colors: Vec<u32> },
    KeySet { fn_layer: bool, index: usize, action: Action },
    /// battery % over 2.4 GHz
    Battery(u8),
    Busy(bool),
    Ok(String),
    Err(String),
}

pub struct Worker {
    pub tx: Sender<Req>,
    pub rx: Receiver<Resp>,
}

impl Worker {
    pub fn spawn(repaint: impl Fn() + Send + 'static) -> Self {
        let (tx, req_rx) = channel::<Req>();
        let (resp_tx, rx) = channel::<Resp>();
        thread::spawn(move || run(req_rx, resp_tx, repaint));
        Worker { tx, rx }
    }
}

fn run(rx: Receiver<Req>, tx: Sender<Resp>, repaint: impl Fn()) {
    let mut kb: Option<Keyboard> = None;
    let mut last_try = Instant::now() - Duration::from_secs(10);
    let mut last_battery = Instant::now() - Duration::from_secs(60);
    let mut battery_reads = 0u32; // poll faster until the UI has enough samples to smooth
    let send = |r: Resp| {
        let _ = tx.send(r);
        repaint();
    };
    loop {
        if kb.is_none() && last_try.elapsed() > Duration::from_secs(2) {
            last_try = Instant::now();
            match Keyboard::open_first() {
                Ok(k) => {
                    let n = k.model.default_matrix.len() / 4;
                    send(Resp::Connected(Box::new(Connected {
                        info: k.info.clone(),
                        slots: k.slots.clone(),
                        layout_size: k.layout_size,
                        has_report_rate: k.has_report_rate,
                        defaults: (0..n).map(|i| k.default_action(i)).collect(),
                        fn_defaults: (0..n).map(|i| k.default_fn_action(i)).collect(),
                    })));
                    kb = Some(k);
                    battery_reads = 0;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotConnected => send(Resp::Disconnected(e.to_string())),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    send(Resp::Disconnected("Permission denied: install 70-akko.rules (see README) and replug".into()))
                }
                Err(_) => send(Resp::Disconnected("Connect your Akko keyboard with a USB cable".into())),
            }
        }
        let req = match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(r) => r,
            Err(RecvTimeoutError::Timeout) => {
                // detect unplug / wireless sleep, refresh battery
                if let Some(k) = &kb {
                    if !std::path::Path::new(&k.info.path).exists() {
                        kb = None;
                        send(Resp::Disconnected("Keyboard disconnected".into()));
                    } else if k.link == Link::Dongle && last_battery.elapsed() > Duration::from_secs(if battery_reads < 3 { 3 } else { 15 }) {
                        last_battery = Instant::now();
                        battery_reads += 1;
                        match k.battery() {
                            Some(b) => send(Resp::Battery(b)),
                            None => {
                                kb = None;
                                send(Resp::Disconnected("Keyboard asleep or off (2.4 GHz): press a key to wake it".into()));
                            }
                        }
                    }
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        let Some(k) = kb.as_ref() else {
            send(Resp::Err("No keyboard connected".into()));
            continue;
        };
        send(Resp::Busy(true));
        let result: std::io::Result<Option<Resp>> = (|| {
            Ok(match req {
                Req::LoadState => {
                    let profile = k.get_profile()?;
                    Some(Resp::State(Box::new(State {
                        profile,
                        light: k.get_light()?,
                        side: k.get_side_light()?,
                        debounce: k.get_debounce()?,
                        options: k.get_options(profile)?,
                        sleep: k.get_sleep()?,
                        report_rate: if k.has_report_rate { k.get_report_rate()? } else { None },
                    })))
                }
                Req::LoadKeymaps { profile, fn_index } => Some(Resp::Keymaps { profile, fn_index, base: k.get_keymap(profile)?, fn_layer: k.get_fn_keymap(fn_index)? }),
                Req::LoadKeyColors(p) => Some(Resp::KeyColors { profile: p, colors: k.get_key_colors(p)? }),
                Req::SetKey { fn_layer, profile, index, action } => {
                    if fn_layer { k.set_fn_key(profile, index, action)? } else { k.set_key(profile, index, action)? }
                    Some(Resp::Ok("Key saved".into()))
                }
                Req::SetLight(l) => { k.set_light(&l)?; Some(Resp::Ok("Lighting saved".into())) }
                Req::SetSideLight(l) => { k.set_side_light(&l)?; Some(Resp::Ok("Side lighting saved".into())) }
                Req::SetKeyColors { profile, colors } => { k.set_key_colors(profile, &colors)?; Some(Resp::Ok("Key colours saved".into())) }
                Req::SetProfile(p) => { k.set_profile(p)?; Some(Resp::Ok(format!("Switched to profile {}", p + 1))) }
                Req::SetDebounce(ms) => { k.set_debounce(ms)?; Some(Resp::Ok("Debounce saved".into())) }
                Req::SetOptions(p, o) => { k.set_options(p, &o)?; Some(Resp::Ok("Options saved".into())) }
                Req::SetSleep(s) => { k.set_sleep(&s)?; Some(Resp::Ok("Sleep timers saved".into())) }
                Req::SetReportRate(hz) => { k.set_report_rate(hz)?; Some(Resp::Ok(format!("Polling rate {hz} Hz"))) }
                Req::AssignMacro { fn_layer, layer, index, mode, events, repeat, profiles } => {
                    let current = if fn_layer { k.get_fn_keymap(layer)? } else { k.get_keymap(layer)? };
                    let own = current.get(index).filter(|a| a[0] == 9).map(|a| a[2]);
                    let mut used = std::collections::HashSet::new();
                    for p in 0..profiles.max(1) {
                        for (i, a) in k.get_keymap(p)?.iter().enumerate() {
                            if a[0] == 9 && !(!fn_layer && p == layer && i == index) { used.insert(a[2]); }
                        }
                    }
                    for l in 0..2u8 {
                        for (i, a) in k.get_fn_keymap(l)?.iter().enumerate() {
                            if a[0] == 9 && !(fn_layer && l == layer && i == index) { used.insert(a[2]); }
                        }
                    }
                    let slot = own.filter(|s| !used.contains(s)).or_else(|| (0u8..50).find(|s| !used.contains(s)));
                    match slot {
                        None => Some(Resp::Err("All 50 macro slots are in use".into())),
                        Some(slot) => {
                            k.set_macro(slot, &events, repeat)?;
                            let action = [9, mode, slot, 0];
                            if fn_layer { k.set_fn_key(layer, index, action)? } else { k.set_key(layer, index, action)? }
                            let _ = tx.send(Resp::KeySet { fn_layer, index, action });
                            Some(Resp::Ok(format!("Macro assigned (slot {})", slot + 1)))
                        }
                    }
                }
                Req::FactoryReset => { k.factory_reset()?; Some(Resp::Ok("Factory reset done".into())) }
            })
        })();
        match result {
            Ok(Some(r)) => send(r),
            Ok(None) => {}
            Err(e) => {
                let gone = matches!(e.raw_os_error(), Some(libc::ENODEV) | Some(libc::EIO) | Some(libc::EPIPE))
                    || matches!(e.kind(), std::io::ErrorKind::NotConnected);
                if gone {
                    let msg = if e.kind() == std::io::ErrorKind::NotConnected { e.to_string() } else { "Keyboard disconnected".to_string() };
                    kb = None;
                    send(Resp::Disconnected(msg));
                } else {
                    send(Resp::Err(format!("Keyboard error: {e}")));
                }
            }
        }
        send(Resp::Busy(false));
    }
}
