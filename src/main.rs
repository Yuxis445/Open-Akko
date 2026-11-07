//! Open Akko: native Linux configurator for Akko keyboards.
//!   openakko            -> GUI
//!   openakko <command>  -> CLI (run `openakko help`)

mod app;
mod data;
mod firmware;
mod hidraw;
mod keycodes;
mod keyrec;
mod protocol;
mod selftest;
mod worker;

use keycodes::{action_label, CATEGORIES};
use protocol::{Keyboard, LIGHT_MODES, SIDE_LIGHT_MODES};

const HELP: &str = "\
Open Akko - Akko keyboard configurator for Linux (offline, no telemetry)

USAGE: openakko [command]

  (no command)                 open the GUI
  list                         list Akko hidraw interfaces
  info                         device info and current settings
  light <mode> [#rrggbb|rainbow] [brightness 0-4] [speed 0-4]
  sidelight <mode> [#rrggbb|rainbow] [brightness] [speed]
  modes                        list lighting modes
  profile [n]                  get/set active profile (1-4)
  debounce [ms]                get/set debounce
  keymap [profile]             show base layer of a profile (default: active)
  keymap --fn [1|2]            show Fn layer 1 or 2 (shared by all profiles; default 1)
  remap <index> <action>       remap a key on the ACTIVE profile (firmware writes there)
  remap <index> <action> --fn [1|2]   remap on Fn layer 1 (default) or 2
                               action: name from the remap lists (e.g. Mute), or raw a,b,c,d
  fwcheck                      check Akko's server for newer firmware (network; sends model ID only)
  record [seconds]             print key events read straight from the keyboard (default 5 s)
  --dongle                     (any command) talk through the 2.4 GHz receiver
  selftest [--write]           smoke test (read-only; --write adds reversible write checks)
  raw <byte> [byte...]         send a raw command and print the reply
";

fn mode_from(s: &str, table: &[(u8, &str)]) -> Option<u8> {
    s.parse().ok().or_else(|| table.iter().find(|(_, n)| n.to_lowercase().replace(' ', "") == s.to_lowercase().replace(['-', ' '], "")).map(|x| x.0))
}

fn parse_action(s: &str) -> Option<keycodes::Action> {
    if s.contains(',') {
        let v: Vec<u8> = s.split(',').filter_map(|x| x.trim().parse().ok()).collect();
        return (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]]);
    }
    if s.eq_ignore_ascii_case("disabled") {
        return Some([0; 4]);
    }
    CATEGORIES.iter().flat_map(|c| c.items.iter()).find(|(n, _)| n.eq_ignore_ascii_case(s)).map(|x| x.1)
}

fn cli(args: &[String]) -> Result<(), String> {
    let cmd = args[0].as_str();
    if cmd == "help" || cmd == "-h" || cmd == "--help" {
        print!("{HELP}");
        return Ok(());
    }
    if cmd == "selftest" {
        std::process::exit(selftest::run(args.iter().any(|a| a == "--write")));
    }
    if cmd == "list" {
        for d in hidraw::enumerate() {
            println!("{}  {:04x}:{:04x}  if{}  {}  {}", d.path, d.vid, d.pid, d.interface, if d.is_config() { "[config]" } else { "        " }, d.name);
        }
        return Ok(());
    }
    if cmd == "modes" {
        println!("main: {}", LIGHT_MODES.iter().map(|(m, n)| format!("{m}={n}")).collect::<Vec<_>>().join(", "));
        println!("side: {}", SIDE_LIGHT_MODES.iter().map(|(m, n)| format!("{m}={n}")).collect::<Vec<_>>().join(", "));
        return Ok(());
    }
    // --dongle: use the 2.4 GHz receiver even if a cable is plugged in
    let opened = if args.iter().any(|a| a == "--dongle") {
        protocol::find_config_interfaces().into_iter().find(|d| protocol::is_dongle(d.pid))
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no 2.4 GHz receiver found"))
            .and_then(|d| Keyboard::open(&d))
    } else { Keyboard::open_first() };
    let kb = opened.map_err(|e| {
        if e.kind() == std::io::ErrorKind::PermissionDenied { "permission denied: install 70-akko.rules".to_string() } else { e.to_string() }
    })?;
    let e = |x: std::io::Error| x.to_string();
    let fn_flag = args.iter().any(|a| a == "--fn");
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    match cmd {
        "info" => {
            let i = &kb.info;
            println!("device     {} ({})", i.display_name, i.model);
            println!("id         {}   firmware 0x{:04x}   {}", i.device_id, i.firmware, i.path);
            println!("connection {}{}", i.connection, kb.battery_median(5).map(|b| format!("   battery ~{b}% (median of 5)")).unwrap_or_default());
            let p = kb.get_profile().map_err(e)?;
            println!("profile    {}", p + 1);
            println!("light      {:?}", kb.get_light().map_err(e)?);
            println!("side light {:?}", kb.get_side_light().map_err(e)?);
            println!("debounce   {} ms", kb.get_debounce().map_err(e)?);
            println!("options    {:?}", kb.get_options(p).map_err(e)?);
            println!("sleep      {:?}", kb.get_sleep().map_err(e)?);
            if kb.has_report_rate { println!("polling    {:?} Hz", kb.get_report_rate().map_err(e)?); }
        }
        "light" | "sidelight" => {
            let side = cmd == "sidelight";
            let table = if side { SIDE_LIGHT_MODES } else { LIGHT_MODES };
            let mut l = if side { kb.get_side_light().map_err(e)?.ok_or("no side light")? } else { kb.get_light().map_err(e)? };
            l.mode = mode_from(pos.get(1).ok_or("mode required")?, table).ok_or("unknown mode (see `openakko modes`)")?;
            if let Some(c) = pos.get(2) {
                if c.as_str() == "rainbow" { l.rainbow = true } else {
                    l.color = u32::from_str_radix(c.trim_start_matches('#'), 16).map_err(|_| "bad colour")?;
                    l.rainbow = false;
                }
            }
            if let Some(b) = pos.get(3) { l.brightness = b.parse::<u8>().map_err(|_| "bad brightness")?.min(4); }
            if let Some(s) = pos.get(4) { l.speed = s.parse::<u8>().map_err(|_| "bad speed")?.min(4); }
            if side { kb.set_side_light(&l).map_err(e)? } else { kb.set_light(&l).map_err(e)? }
            println!("{l:?}");
        }
        "profile" => {
            if let Some(n) = pos.get(1) { kb.set_profile(n.parse::<u8>().map_err(|_| "bad profile")?.saturating_sub(1)).map_err(e)?; }
            println!("profile {}", kb.get_profile().map_err(e)? + 1);
        }
        "debounce" => {
            if let Some(n) = pos.get(1) { kb.set_debounce(n.parse().map_err(|_| "bad value")?).map_err(e)?; }
            println!("debounce {} ms", kb.get_debounce().map_err(e)?);
        }
        "keymap" => {
            let arg = pos.get(1).and_then(|x| x.parse::<u8>().ok()).map(|x| x.saturating_sub(1));
            let km = if fn_flag { kb.get_fn_keymap(arg.unwrap_or(0)) } else { kb.get_keymap(arg.unwrap_or(kb.get_profile().map_err(e)?)) }.map_err(e)?;
            for s in &kb.slots {
                let a = km.get(s.index).copied().unwrap_or([0; 4]);
                let d = if fn_flag { kb.default_fn_action(s.index) } else { kb.default_action(s.index) };
                println!("{:3}  {:18} -> {}{}", s.index, s.code, action_label(a), if a != d { "  *" } else { "" });
            }
        }
        "remap" => {
            let idx: usize = pos.get(1).ok_or("index required")?.parse().map_err(|_| "bad index")?;
            let a = parse_action(pos.get(2).ok_or("action required")?).ok_or("unknown action")?;
            // NB: the firmware applies single-key writes to the active profile regardless of the profile byte
            let p = if fn_flag { pos.get(3).and_then(|x| x.parse::<u8>().ok()).map(|x| x.saturating_sub(1)).unwrap_or(0) } else { kb.get_profile().map_err(e)? };
            if fn_flag { kb.set_fn_key(p, idx, a) } else { kb.set_key(p, idx, a) }.map_err(e)?;
            println!("#{idx} -> {}", action_label(a));
        }
        "record" => {
            let secs: u64 = pos.get(1).and_then(|x| x.parse().ok()).unwrap_or(5);
            let rec = keyrec::start(&kb.info.path)?;
            println!("type on the Akko keyboard for {secs} s…");
            let t0 = std::time::Instant::now();
            let mut last = t0;
            while t0.elapsed().as_secs() < secs {
                if let Ok((code, down, at)) = rec.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    println!("  +{:5} ms  {:<8} {}", at.duration_since(last).as_millis(), keycodes::hid_label(code), if down { "down" } else { "up" });
                    last = at;
                }
            }
        }
        "fwcheck" => {
            println!("installed  v{:x}", kb.info.firmware);
            let c = firmware::check(kb.info.device_id, kb.info.firmware)?;
            println!("latest     {}", c.latest);
            println!("{}", if c.update_available { "update available (install with the official Akko app; Open Akko does not flash)" } else { "up to date" });
        }
        "raw" => {
            let bytes: Vec<u8> = pos[1..].iter().filter_map(|b| {
                let b = b.trim_start_matches("0x");
                u8::from_str_radix(b, if b.len() <= 2 && pos.iter().any(|x| x.starts_with("0x")) { 16 } else { 10 }).ok()
            }).collect();
            let r = kb.raw(&bytes).map_err(e)?;
            println!("{}", r.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
        }
        _ => return Err(format!("unknown command '{cmd}'\n\n{HELP}")),
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if let Err(m) = cli(&args) {
            eprintln!("error: {m}");
            std::process::exit(1);
        }
        return;
    }
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_title("Open Akko")
        .with_app_id("openakko")
        .with_inner_size([1480.0, 900.0])
        .with_min_inner_size([1100.0, 700.0]);
    if let Ok(img) = image::load_from_memory(include_bytes!("../packaging/openakko-256.png")) {
        let img = img.to_rgba8();
        viewport = viewport.with_icon(eframe::egui::IconData { width: img.width(), height: img.height(), rgba: img.into_raw() });
    }
    let opts = eframe::NativeOptions { viewport, ..Default::default() };
    if let Err(e) = eframe::run_native("Open Akko", opts, Box::new(|cc| Ok(Box::new(app::App::new(cc))))) {
        eprintln!("GUI error: {e}");
        std::process::exit(1);
    }
}
