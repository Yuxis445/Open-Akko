//! `openakko selftest [--write]` - hardware smoke test.
//!
//! Read-only by default. With --write every check snapshots the state it touches, changes it,
//! reads it back and restores the snapshot (restores also run when a check fails).
//! Never performs a factory reset.

use crate::hidraw;
use crate::keycodes::{action_label, Action};
use crate::protocol::*;
use std::io;
use std::time::Instant;

struct Report {
    pass: u32,
    fail: u32,
    warn: u32,
}

impl Report {
    fn ok(&mut self, name: &str, detail: impl AsRef<str>) {
        self.pass += 1;
        println!("  \x1b[32mPASS\x1b[0m  {name:<38} {}", detail.as_ref());
    }
    fn bad(&mut self, name: &str, detail: impl AsRef<str>) {
        self.fail += 1;
        println!("  \x1b[31mFAIL\x1b[0m  {name:<38} {}", detail.as_ref());
    }
    fn warn(&mut self, name: &str, detail: impl AsRef<str>) {
        self.warn += 1;
        println!("  \x1b[33mWARN\x1b[0m  {name:<38} {}", detail.as_ref());
    }
    fn check(&mut self, name: &str, cond: bool, detail: impl AsRef<str>) {
        if cond { self.ok(name, detail) } else { self.bad(name, detail) }
    }
    fn result<T>(&mut self, name: &str, r: io::Result<T>, f: impl FnOnce(&T) -> (bool, String)) -> Option<T> {
        match r {
            Ok(v) => {
                let (good, d) = f(&v);
                self.check(name, good, d);
                Some(v)
            }
            Err(e) => {
                self.bad(name, format!("I/O error: {e}"));
                None
            }
        }
    }
}

fn section(t: &str) {
    println!("\n\x1b[1m{t}\x1b[0m");
}

pub fn run(write: bool) -> i32 {
    let t0 = Instant::now();
    let mut r = Report { pass: 0, fail: 0, warn: 0 };
    println!("Open Akko smoke test ({})", if write { "read + reversible writes" } else { "read-only" });

    // ------------------------------------------------------------ transport
    section("1. Transport");
    let all = hidraw::enumerate();
    r.check("ROYUAN hidraw interfaces present", !all.is_empty(), format!("{} interface(s)", all.len()));
    let cfg: Vec<_> = all.iter().filter(|d| d.is_config()).collect();
    r.check("vendor config interface found", cfg.len() == 1, cfg.iter().map(|d| d.path.as_str()).collect::<Vec<_>>().join(", "));
    let kb = match Keyboard::open_first() {
        Ok(k) => { r.ok("open + identify", &k.info.path); k }
        Err(e) => {
            r.bad("open + identify", e.to_string());
            if e.kind() == io::ErrorKind::PermissionDenied { println!("        -> install 70-akko.rules and replug"); }
            return summary(&r, t0);
        }
    };
    let echo = kb.raw(&[cmd::GET_INFO]);
    r.result("command echo + checksum accepted", echo, |b| (b[0] == cmd::GET_INFO, format!("reply starts {:02x}", b[0])));

    // ------------------------------------------------------------- identity
    section("2. Identity");
    let i = &kb.info;
    match kb.link {
        Link::Usb => r.ok("connection", "USB cable"),
        Link::Dongle => match kb.battery_median(5) {
            Some(b) => r.ok("connection", format!("2.4 GHz receiver, keyboard battery ~{b}%")),
            None => r.bad("connection", "2.4 GHz receiver, keyboard not responding"),
        },
    }
    r.check("device id read", i.device_id > 0, format!("{}", i.device_id));
    r.check("model in Akko device table", i.model != "unknown", format!("{} / {}", i.display_name, i.model));
    r.check("model supported by protocol", i.supported, if i.supported { "YC3121/YC500 family".into() } else { "generic fallback".to_string() });
    r.check("firmware version read", i.firmware != 0, format!("0x{:04x}", i.firmware));
    r.check("default key matrix loaded", kb.model.default_matrix.len() >= 256, format!("{} slots", kb.model.default_matrix.len() / 4));
    let real_layout = kb.slots.iter().any(|s| !s.code.starts_with('#'));
    if real_layout {
        r.ok("keyboard drawing mapped to matrix", format!("{} keys/knob parts", kb.slots.len()));
    } else {
        r.warn("keyboard drawing mapped to matrix", "no bundled layout, using grid fallback");
    }
    let mut dup = kb.slots.iter().filter(|s| s.knob_part.is_none()).map(|s| s.index).collect::<Vec<_>>();
    dup.sort();
    let before = dup.len();
    dup.dedup();
    r.check("no two keys share a matrix slot", before == dup.len(), format!("{} unique", dup.len()));

    // ---------------------------------------------------------------- reads
    if !wait_awake(&kb) { r.bad("keyboard awake", "asleep: press a key and rerun"); return summary(&r, t0); }
    section("3. Read settings");
    let profile = r.result("active profile", kb.get_profile(), |p| (*p < 4, format!("profile {}", p + 1))).unwrap_or(0);
    let light = r.result("main lighting", kb.get_light(), |l| (
        LIGHT_MODES.iter().any(|(m, _)| *m == l.mode) && l.brightness <= 4,
        format!("{} br={} sp={} #{:06x}{}", mode_name(l.mode, LIGHT_MODES), l.brightness, l.speed, l.color, if l.rainbow { " rainbow" } else { "" })));
    let side = match kb.get_side_light() {
        Ok(Some(s)) => { r.ok("side lighting", format!("{} br={} sp={}", mode_name(s.mode, SIDE_LIGHT_MODES), s.brightness, s.speed)); Some(s) }
        Ok(None) => { r.warn("side lighting", "not present on this model"); None }
        Err(e) => { r.bad("side lighting", e.to_string()); None }
    };
    let debounce = r.result("debounce", kb.get_debounce(), |d| (*d <= 50, format!("{d} ms")));
    let options = r.result("keyboard options", kb.get_options(profile), |o| (true, format!(
        "winlock={} mac={} wasd={} fnswap={} powersave={}", o.win_lock, o.mac_mode, o.wasd_swap, o.fn_swap, o.power_save)));
    let sleep_t = r.result("sleep timers", kb.get_sleep(), |s| (s.rf > 0 && s.rf < 36000, format!("2.4G {}s / BT {}s, deep {}s / {}s", s.rf, s.bt, s.deep_rf, s.deep_bt)));
    if kb.has_report_rate {
        r.result("polling rate", kb.get_report_rate(), |h| (h.is_some(), format!("{h:?} Hz")));
    }

    section("4. Read keymaps");
    let mut base_maps = vec![];
    for p in 0..i.profiles {
        let m = r.result(&format!("profile {} base layer", p + 1), kb.get_keymap(p), |m| {
            let changed = kb.slots.iter().filter(|s| m[s.index] != kb.default_action(s.index)).count();
            (m.len() * 4 == kb.model.default_matrix.len(), format!("{changed} key(s) changed from default"))
        });
        base_maps.push(m.unwrap_or_default());
    }
    let mut fn_maps = vec![];
    for l in 0..2u8 {
        let m = r.result(&format!("Fn layer {}", l + 1), kb.get_fn_keymap(l), |m| {
            let changed: Vec<String> = kb.slots.iter().filter(|s| m[s.index] != kb.default_fn_action(s.index))
                .map(|s| format!("{}={}", s.code, action_label(m[s.index]))).collect();
            (true, if changed.is_empty() { "factory default".into() } else { changed.join(", ") })
        });
        fn_maps.push(m.unwrap_or_default());
    }
    let again = kb.get_keymap(profile);
    r.result("repeat read is identical", again, |m| (base_maps.get(profile as usize).map(|b| b == m).unwrap_or(false), "consistent".into()));
    r.result("per-key colours (Custom mode)", kb.get_key_colors(profile), |c| (c.len() * 4 == kb.model.default_matrix.len(), format!("{} entries", c.len())));
    let used_macros: Vec<u8> = base_maps.iter().chain(fn_maps.iter()).flatten().filter(|a| a[0] == 9).map(|a| a[2]).collect();
    r.result("macro slot read", kb.get_macro(used_macros.first().copied().unwrap_or(0)), |(rep, ev)| (true, format!("repeat {rep}, {} events", ev.len())));

    if !write {
        println!("\n(read-only run; use `openakko selftest --write` for reversible write checks)");
        return summary(&r, t0);
    }

    // --------------------------------------------------------------- writes
    section("5. Reversible writes (state is restored after each check)");
    let complete = base_maps.len() == i.profiles as usize && base_maps.iter().all(|m| !m.is_empty())
        && fn_maps.len() == 2 && fn_maps.iter().all(|m| !m.is_empty());
    if !complete {
        r.bad("snapshot", "reads were incomplete (keyboard asleep?): writes skipped so nothing can be left changed");
        return summary(&r, t0);
    }
    if kb.link == Link::Dongle {
        println!("        (2.4 GHz: tap a key now and then so the keyboard does not fall asleep)");
    }

    // side light
    if let Some(orig) = side {
        let test = Light { mode: 1, color: if orig.color == 0x00FF00 { 0xFF0000 } else { 0x00FF00 }, rainbow: false, ..orig };
        roundtrip(&mut r, &kb, "side light write/readback", || kb.set_side_light(&test), || kb.get_side_light().map(|x| x == Some(test)),
            || kb.set_side_light(&orig), || kb.get_side_light().map(|x| x == Some(orig)));
    }
    // main light: change only brightness, keep the mode (Custom stays Custom)
    if let Some(orig) = light {
        let test = Light { brightness: if orig.brightness == 4 { 3 } else { orig.brightness + 1 }, ..orig };
        roundtrip(&mut r, &kb, "main light brightness write", || kb.set_light(&test), || kb.get_light().map(|x| x.brightness == test.brightness && x.mode == orig.mode),
            || kb.set_light(&orig), || kb.get_light().map(|x| x == orig));
    }
    // debounce
    if let Some(d) = debounce {
        let t = if d >= 10 { d - 1 } else { d + 1 };
        roundtrip(&mut r, &kb, "debounce write", || kb.set_debounce(t), || kb.get_debounce().map(|x| x == t),
            || kb.set_debounce(d), || kb.get_debounce().map(|x| x == d));
    }
    // options: toggle win lock
    if let Some(o) = options {
        let t = Options { win_lock: !o.win_lock, ..o };
        roundtrip(&mut r, &kb, "options write (Win-lock toggle)", || kb.set_options(profile, &t), || kb.get_options(profile).map(|x| x.win_lock == t.win_lock),
            || kb.set_options(profile, &o), || kb.get_options(profile).map(|x| x == o));
    }
    // sleep timers
    if let Some(s) = sleep_t {
        let t = Sleep { rf: s.rf + 30, ..s };
        roundtrip(&mut r, &kb, "sleep timer write", || kb.set_sleep(&t), || kb.get_sleep().map(|x| x == t),
            || kb.set_sleep(&s), || kb.get_sleep().map(|x| x == s));
    }
    // per-key colours: write back identical data (no visible change)
    match kb.get_key_colors(profile) {
        Ok(c) => {
            let ok = kb.set_key_colors(profile, &c).and_then(|_| kb.get_key_colors(profile)).map(|n| n == c);
            match ok { Ok(true) => r.ok("per-key colour write (same data)", "identical after write"), Ok(false) => r.bad("per-key colour write (same data)", "readback differs"), Err(e) => r.bad("per-key colour write (same data)", e.to_string()) }
        }
        Err(e) => r.bad("per-key colour write (same data)", e.to_string()),
    }
    // base-layer key: pick Q, remap to its own default+1 code, restore
    let base = base_maps.get(profile as usize).cloned().unwrap_or_default();
    if let Some(q) = kb.slots.iter().find(|s| s.code == "KeyQ").map(|s| s.index) {
        let Some(&orig) = base.get(q) else { return summary(&r, t0) };
        let test: Action = [0, 0, 0x1D, 0]; // Z
        roundtrip(&mut r, &kb, "base key remap (Q->Z, active profile)", || kb.set_key(profile, q, test), || kb.get_keymap(profile).map(|m| m[q] == test),
            || kb.set_key(profile, q, orig), || kb.get_keymap(profile).map(|m| m[q] == orig));
    }
    // Fn layer 2: use a key that is unset in BOTH Fn layers so nothing of yours can be overwritten
    if fn_maps.len() == 2 && wait_awake(&kb) {
        let cand = kb.slots.iter().filter(|s| s.knob_part.is_none() && s.code != "Fn" && !s.code.starts_with("Shift") && !s.code.starts_with("Control"))
            .find(|s| fn_maps[0][s.index] == [0; 4] && fn_maps[1][s.index] == [0; 4]).map(|s| (s.index, s.code.clone()));
        if let Some((idx, code)) = cand {
            let test: Action = [3, 0, 226, 0]; // Mute
            let name = format!("Fn layer 2 write ({code}, unset key)");
            let landed = kb.set_fn_key(1, idx, test).and_then(|_| Ok((kb.get_fn_keymap(0)?[idx], kb.get_fn_keymap(1)?[idx])));
            let restore = kb.set_fn_key(1, idx, [0; 4]).and_then(|_| kb.set_fn_key(0, idx, [0; 4]))
                .and_then(|_| Ok(kb.get_fn_keymap(0)? == fn_maps[0] && kb.get_fn_keymap(1)? == fn_maps[1]));
            match landed {
                Ok((a0, a1)) if a1 == test && a0 == [0; 4] => r.ok(&name, "landed on Fn layer 2 as requested"),
                Ok((a0, _)) if a0 == test => r.warn(&name, "firmware wrote Fn layer 1 instead (layer byte ignored)"),
                Ok(_) => r.bad(&name, "write did not appear"),
                Err(e) => r.bad(&name, e.to_string()),
            }
            r.check("  restored both Fn layers", matches!(restore, Ok(true)), "identical to snapshot");
        } else {
            r.warn("Fn layer write", "no key unset in both layers; skipped");
        }
    }
    // macro: unused slot, write, read back, restore previous content
    if let Some(slot) = (0u8..50).rev().find(|s| !used_macros.contains(s)).filter(|_| wait_awake(&kb)) {
        let ev = vec![MacroEvent { code: 0x0B, down: true, delay_ms: 20 }, MacroEvent { code: 0x0B, down: false, delay_ms: 20 },
                      MacroEvent { code: 0x0C, down: true, delay_ms: 300 }, MacroEvent { code: 0x0C, down: false, delay_ms: 20 }];
        match kb.get_macro(slot) {
            Ok((orep, oev)) => {
                let wrote = kb.set_macro(slot, &ev, 2).and_then(|_| kb.get_macro(slot));
                let ok = matches!(&wrote, Ok((rep, e)) if *rep == 2 && e.len() == ev.len() && e.iter().zip(&ev).all(|(a, b)| a.code == b.code && a.down == b.down));
                r.check(&format!("macro write/readback (slot {}, unused)", slot + 1), ok,
                    match &wrote { Ok((rep, e)) => format!("repeat {rep}, {} events (incl. 300 ms long delay)", e.len()), Err(x) => x.to_string() });
                let restored = kb.set_macro(slot, &oev, orep).is_ok();
                r.check("  macro slot restored", restored, "previous content written back");
            }
            Err(e) => r.bad("macro write/readback", e.to_string()),
        }
    }
    // profile switch
    let other = if profile == 0 { 1 } else { 0 };
    roundtrip(&mut r, &kb, &format!("profile switch ({} -> {} -> {})", profile + 1, other + 1, profile + 1),
        || kb.set_profile(other), || kb.get_profile().map(|p| p == other),
        || kb.set_profile(profile), || kb.get_profile().map(|p| p == profile));

    summary(&r, t0)
}

fn mode_name(m: u8, t: &[(u8, &str)]) -> String {
    t.iter().find(|(x, _)| *x == m).map(|x| x.1.to_string()).unwrap_or(format!("mode {m}"))
}

/// Over 2.4 GHz the keyboard sleeps after its idle timer; only a key press wakes it.
/// Returns false if it stayed asleep for 60 s.
fn wait_awake(kb: &Keyboard) -> bool {
    if kb.link != Link::Dongle || kb.battery().is_some() {
        return true;
    }
    println!("        \x1b[33m… keyboard is asleep: press any key on it (waiting up to 60 s)\x1b[0m");
    for _ in 0..120 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        if kb.battery().is_some() {
            return true;
        }
    }
    false
}

fn asleep(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::NotConnected
}

/// write -> verify -> restore -> verify-restore. Restore always runs, and is retried after a sleep.
fn roundtrip(
    r: &mut Report, kb: &Keyboard, name: &str,
    write: impl Fn() -> io::Result<()>, verify: impl Fn() -> io::Result<bool>,
    restore: impl Fn() -> io::Result<()>, verify_restore: impl Fn() -> io::Result<bool>,
) {
    if !wait_awake(kb) {
        r.bad(name, "skipped: keyboard asleep");
        return;
    }
    let res = write().and_then(|_| verify());
    let mut rest = restore().and_then(|_| verify_restore());
    if matches!(&rest, Err(e) if asleep(e)) && wait_awake(kb) {
        rest = restore().and_then(|_| verify_restore());
    }
    match res {
        Ok(true) => r.ok(name, "changed and read back"),
        Ok(false) => r.bad(name, "readback did not match"),
        Err(e) => r.bad(name, e.to_string()),
    }
    match rest {
        Ok(true) => r.ok("  restored", "matches snapshot"),
        Ok(false) => r.bad("  restored", "STATE DIFFERS FROM SNAPSHOT - check this setting"),
        Err(e) => r.bad("  restored", format!("{e}  - STATE MAY DIFFER, rerun the selftest")),
    }
}

fn summary(r: &Report, t0: Instant) -> i32 {
    println!("\n{} passed, {} failed, {} warnings  ({:.1}s)", r.pass, r.fail, r.warn, t0.elapsed().as_secs_f32());
    if r.fail == 0 { 0 } else { 1 }
}
