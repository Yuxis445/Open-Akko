//! HID usage tables, layout key codes and the 4-byte key action format
//! (recovered from Akko Cloud v4's keymap tables).

pub type Action = [u8; 4];

/// HID usage -> short label (as printed on keycaps in the official UI)
pub fn hid_label(code: u8) -> String {
    let s = match code {
        0x04..=0x1D => return ((b'A' + code - 0x04) as char).to_string(),
        0x1E..=0x26 => return ((b'1' + code - 0x1E) as char).to_string(),
        0x27 => "0",
        0x28 => "Enter", 0x29 => "Esc", 0x2A => "Backspace", 0x2B => "Tab", 0x2C => "Space",
        0x2D => "-", 0x2E => "=", 0x2F => "[", 0x30 => "]", 0x31 => "\\", 0x32 => "#",
        0x33 => ";", 0x34 => "'", 0x35 => "`", 0x36 => ",", 0x37 => ".", 0x38 => "/",
        0x39 => "Caps",
        0x3A..=0x45 => return format!("F{}", code - 0x3A + 1),
        0x46 => "PrtSc", 0x47 => "ScrLk", 0x48 => "Pause", 0x49 => "Ins", 0x4A => "Home",
        0x4B => "PgUp", 0x4C => "Del", 0x4D => "End", 0x4E => "PgDn", 0x4F => "→",
        0x50 => "←", 0x51 => "↓", 0x52 => "↑", 0x53 => "NumLk", 0x54 => "N/", 0x55 => "N*",
        0x56 => "N-", 0x57 => "N+", 0x58 => "NEnter",
        0x59..=0x61 => return format!("N{}", code - 0x59 + 1),
        0x62 => "N0", 0x63 => "N.", 0x64 => "\\|", 0x65 => "Menu",
        0x68..=0x73 => return format!("F{}", code - 0x68 + 13),
        0x87 => "Ro", 0x89 => "¥", 0x8A => "Henkan", 0x8B => "Muhenkan", 0x90 => "Lang1", 0x91 => "Lang2",
        0xE0 => "Ctrl", 0xE1 => "Shift", 0xE2 => "Alt", 0xE3 => "Win",
        0xE4 => "RCtrl", 0xE5 => "RShift", 0xE6 => "RAlt", 0xE7 => "RWin",
        0 => "",
        _ => return format!("0x{code:02X}"),
    };
    s.to_string()
}

/// KeyboardEvent.code (used by the layout SVGs) -> HID usage
pub fn dom_code_to_hid(code: &str) -> Option<u8> {
    let c = code.split("_index").next().unwrap_or(code);
    if let Some(l) = c.strip_prefix("Key") {
        let b = l.as_bytes();
        if b.len() == 1 && b[0].is_ascii_uppercase() {
            return Some(0x04 + b[0] - b'A');
        }
    }
    if let Some(d) = c.strip_prefix("Digit") {
        return match d { "0" => Some(0x27), _ => d.parse::<u8>().ok().map(|n| 0x1E + n - 1) };
    }
    if let Some(f) = c.strip_prefix('F') {
        if let Ok(n) = f.parse::<u8>() {
            return Some(if n <= 12 { 0x3A + n - 1 } else { 0x68 + n - 13 });
        }
    }
    if let Some(n) = c.strip_prefix("Numpad") {
        if let Ok(d) = n.parse::<u8>() {
            return Some(if d == 0 { 0x62 } else { 0x59 + d - 1 });
        }
    }
    Some(match c {
        "Enter" => 0x28, "Escape" => 0x29, "Backspace" => 0x2A, "Tab" => 0x2B, "Space" => 0x2C,
        "Minus" => 0x2D, "Equal" => 0x2E, "BracketLeft" => 0x2F, "BracketRight" => 0x30,
        "Backslash" => 0x31, "Semicolon" => 0x33, "Quote" => 0x34, "Backquote" => 0x35,
        "Comma" => 0x36, "Period" => 0x37, "Slash" => 0x38, "CapsLock" => 0x39,
        "PrintScreen" => 0x46, "ScrollLock" => 0x47, "Pause" => 0x48, "Insert" => 0x49,
        "Home" => 0x4A, "PageUp" => 0x4B, "Delete" => 0x4C, "End" => 0x4D, "PageDown" => 0x4E,
        "ArrowRight" => 0x4F, "ArrowLeft" => 0x50, "ArrowDown" => 0x51, "ArrowUp" => 0x52,
        "NumLock" => 0x53, "NumpadDivide" => 0x54, "NumpadMultiply" => 0x55,
        "NumpadSubtract" => 0x56, "NumpadAdd" => 0x57, "NumpadEnter" => 0x58,
        "NumpadDecimal" => 0x63, "IntlBackslash" => 0x64, "ContextMenu" => 0x65,
        "IntlRo" => 0x87, "IntlYen" => 0x89, "Convert" => 0x8A, "NonConvert" => 0x8B,
        "KanaMode" => 0x88, "Lang1" => 0x90, "Lang2" => 0x91,
        "ControlLeft" => 0xE0, "ShiftLeft" => 0xE1, "AltLeft" => 0xE2, "MetaLeft" => 0xE3,
        "ControlRight" => 0xE4, "ShiftRight" => 0xE5, "AltRight" => 0xE6, "MetaRight" => 0xE7,
        _ => return None,
    })
}

/// Special (non-HID) layout codes -> the action that identifies them in the default matrix.
pub fn special_code_actions(code: &str) -> &'static [Action] {
    match code {
        "Fn" => &[[10, 1, 0, 0]],
        "AudioVolumeDown" => &[[3, 0, 234, 0]],
        "AudioVolumeUp" => &[[3, 0, 233, 0]],
        "AudioVolumeMute" => &[[3, 0, 226, 0]],
        "Volume_Brightness" | "Volume_Brightness1" => &[[10, 14, 0, 0], [10, 13, 0, 0], [10, 15, 0, 0]],
        "MediaPlayPause" => &[[3, 0, 205, 0]],
        "LaunchApp2" | "Calculator" => &[[3, 0, 146, 1]],
        _ => &[],
    }
}

pub const MOD_NAMES: [&str; 8] = ["Ctrl", "Shift", "Alt", "Win", "RCtrl", "RShift", "RAlt", "RWin"];

pub struct Category {
    pub name: &'static str,
    pub items: &'static [(&'static str, Action)],
}

macro_rules! k { ($l:expr, $c:expr) => { ($l, [0, 0, $c, 0]) }; }

pub const CAT_LETTERS: &[(&str, Action)] = &[
    k!("A", 0x04), k!("B", 0x05), k!("C", 0x06), k!("D", 0x07), k!("E", 0x08), k!("F", 0x09),
    k!("G", 0x0A), k!("H", 0x0B), k!("I", 0x0C), k!("J", 0x0D), k!("K", 0x0E), k!("L", 0x0F),
    k!("M", 0x10), k!("N", 0x11), k!("O", 0x12), k!("P", 0x13), k!("Q", 0x14), k!("R", 0x15),
    k!("S", 0x16), k!("T", 0x17), k!("U", 0x18), k!("V", 0x19), k!("W", 0x1A), k!("X", 0x1B),
    k!("Y", 0x1C), k!("Z", 0x1D), k!("1!", 0x1E), k!("2@", 0x1F), k!("3#", 0x20), k!("4$", 0x21),
    k!("5%", 0x22), k!("6^", 0x23), k!("7&", 0x24), k!("8*", 0x25), k!("9(", 0x26), k!("0)", 0x27),
    k!("-_", 0x2D), k!("=+", 0x2E), k!("[{", 0x2F), k!("]}", 0x30), k!("\\|", 0x31), k!(";:", 0x33),
    k!("'\"", 0x34), k!("`~", 0x35), k!(",<", 0x36), k!(".>", 0x37), k!("/?", 0x38),
];
pub const CAT_CONTROL: &[(&str, Action)] = &[
    k!("Esc", 0x29), k!("Tab", 0x2B), k!("Caps", 0x39), k!("Space", 0x2C), k!("Enter", 0x28),
    k!("Backspace", 0x2A), k!("Ins", 0x49), k!("Del", 0x4C), k!("Home", 0x4A), k!("End", 0x4D),
    k!("PgUp", 0x4B), k!("PgDn", 0x4E), k!("↑", 0x52), k!("↓", 0x51), k!("←", 0x50), k!("→", 0x4F),
    k!("PrtSc", 0x46), k!("ScrLk", 0x47), k!("Pause", 0x48), k!("Menu", 0x65),
    k!("L-Ctrl", 0xE0), k!("L-Shift", 0xE1), k!("L-Alt", 0xE2), k!("L-Win", 0xE3),
    k!("R-Ctrl", 0xE4), k!("R-Shift", 0xE5), k!("R-Alt", 0xE6), k!("R-Win", 0xE7),
    k!("F1", 0x3A), k!("F2", 0x3B), k!("F3", 0x3C), k!("F4", 0x3D), k!("F5", 0x3E), k!("F6", 0x3F),
    k!("F7", 0x40), k!("F8", 0x41), k!("F9", 0x42), k!("F10", 0x43), k!("F11", 0x44), k!("F12", 0x45),
    k!("F13", 0x68), k!("F14", 0x69), k!("F15", 0x6A), k!("F16", 0x6B), k!("F17", 0x6C), k!("F18", 0x6D),
    k!("F19", 0x6E), k!("F20", 0x6F), k!("F21", 0x70), k!("F22", 0x71), k!("F23", 0x72), k!("F24", 0x73),
    k!("NumLk", 0x53), k!("N/", 0x54), k!("N*", 0x55), k!("N-", 0x56), k!("N+", 0x57), k!("NEnter", 0x58),
    k!("N1", 0x59), k!("N2", 0x5A), k!("N3", 0x5B), k!("N4", 0x5C), k!("N5", 0x5D), k!("N6", 0x5E),
    k!("N7", 0x5F), k!("N8", 0x60), k!("N9", 0x61), k!("N0", 0x62), k!("N.", 0x63),
];
pub const CAT_MEDIA: &[(&str, Action)] = &[
    ("Play/Pause", [3, 0, 205, 0]), ("Previous", [3, 0, 182, 0]), ("Next", [3, 0, 181, 0]),
    ("Stop", [3, 0, 183, 0]), ("Mute", [3, 0, 226, 0]), ("Volume -", [3, 0, 234, 0]),
    ("Volume +", [3, 0, 233, 0]), ("Media Player", [3, 0, 131, 1]), ("Calculator", [3, 0, 146, 1]),
    ("Mail", [3, 0, 138, 1]), ("My Computer", [3, 0, 148, 1]), ("Search", [3, 0, 33, 2]),
    ("Browser Home", [3, 0, 35, 2]), ("Browser Back", [3, 0, 36, 2]),
    ("Screen Bright -", [3, 0, 112, 0]), ("Screen Bright +", [3, 0, 111, 0]),
    ("Mic Mute", [6, 128, 0, 0]),
];
pub const CAT_MOUSE: &[(&str, Action)] = &[
    ("Left Click", [1, 0, 240, 0]), ("Right Click", [1, 0, 241, 0]), ("Middle Click", [1, 0, 242, 0]),
    ("Back", [1, 0, 243, 0]), ("Forward", [1, 0, 244, 0]),
    ("Scroll Up", [1, 0, 245, 1]), ("Scroll Down", [1, 0, 245, 255]),
];
pub const CAT_FUNCTION: &[(&str, Action)] = &[
    ("Fn", [10, 1, 0, 0]), ("Vol ⇄ LED", [10, 14, 0, 0]),
    ("LED Bright +", [13, 2, 1, 0]), ("LED Bright -", [13, 2, 2, 0]),
    ("Profile +", [8, 0, 1, 0]), ("Profile -", [8, 0, 2, 0]), ("Profile Cycle", [8, 0, 3, 0]),
    ("Power", [2, 0, 129, 0]), ("Sleep", [2, 0, 130, 0]), ("Wake", [2, 0, 131, 0]),
    ("Lock (Win+L)", [0, 0x08, 0x0F, 0]), ("Show Desktop", [0, 0x08, 0x07, 0]),
    ("Copy", [0, 0x01, 0x06, 0]), ("Paste", [0, 0x01, 0x19, 0]), ("Cut", [0, 0x01, 0x1B, 0]),
    ("Undo", [0, 0x01, 0x1D, 0]), ("Task Manager", [0, 0x03, 0x29, 0]),
];

pub const CATEGORIES: &[Category] = &[
    Category { name: "Basic", items: CAT_LETTERS },
    Category { name: "Control & F-keys", items: CAT_CONTROL },
    Category { name: "Media", items: CAT_MEDIA },
    Category { name: "Mouse", items: CAT_MOUSE },
    Category { name: "Function", items: CAT_FUNCTION },
];

/// Human readable text for a 4-byte action.
pub fn action_label(a: Action) -> String {
    if a == [0, 0, 0, 0] {
        return "Disabled".into();
    }
    for cat in CATEGORIES {
        for (n, v) in cat.items {
            // named entries: everything except plain single keys (those use the key label)
            if *v == a && (a[0] != 0 || a[1] != 0) {
                return n.to_string();
            }
        }
    }
    match a[0] {
        0 => {
            let mut parts: Vec<String> = (0..8).filter(|i| a[1] & (1 << i) != 0).map(|i| MOD_NAMES[i].to_string()).collect();
            for c in [a[2], a[3]] {
                if c != 0 {
                    parts.push(hid_label(c));
                }
            }
            parts.join("+")
        }
        9 => format!("Macro {}", a[2] + 1),
        8 if a[2] == 4 => format!("Profile {}", a[3] + 1),
        10 if a[1] == 13 || a[1] == 14 || a[1] == 15 => "Vol ⇄ LED".into(),
        _ => format!("Factory function [{},{},{},{}]", a[0], a[1], a[2], a[3]),
    }
}

pub fn is_factory_function(a: Action) -> bool {
    action_label(a).starts_with("Factory function")
}

/// Short label for drawing on a keycap.
pub fn action_cap(a: Action) -> String {
    match a[0] {
        0 if a[1] == 0 && a[3] == 0 => hid_label(a[2]),
        0 if a != [0; 4] => {
            let named = action_label(a);
            if !named.contains('+') {
                return short_name(&named); // e.g. Copy, Task Manager -> TaskMgr
            }
            // compact combo for a keycap: C+S+B (full name in the tooltip)
            const M: [&str; 8] = ["C", "S", "A", "W", "RC", "RS", "RA", "RW"];
            let mut parts: Vec<String> = (0..8).filter(|i| a[1] & (1 << i) != 0).map(|i| M[i].to_string()).collect();
            parts.extend([a[2], a[3]].iter().filter(|&&c| c != 0).map(|&c| hid_label(c)));
            parts.join("+")
        }
        9 => format!("M{}", a[2] + 1),
        _ => short_name(&action_label(a)),
    }
}

/// Compact keycap names for long action labels (full name is shown on hover).
fn short_name(l: &str) -> String {
    let s = match l {
        "My Computer" => "PC", "Media Player" => "Player", "Play/Pause" => "Play ⏯", "Previous" => "Prev",
        "Calculator" => "Calc", "Browser Home" => "WWW", "Browser Back" => "Back", "Search" => "Search",
        "Screen Bright -" => "Scr −", "Screen Bright +" => "Scr +", "Volume -" => "Vol −", "Volume +" => "Vol +",
        "LED Bright +" => "LED +", "LED Bright -" => "LED −", "Mic Mute" => "Mic", "Vol ⇄ LED" => "V⇄L",
        "Lock (Win+L)" => "Lock", "Show Desktop" => "Desk", "Task Manager" => "TaskMgr",
        "Left Click" => "LMB", "Right Click" => "RMB", "Middle Click" => "MMB",
        "Scroll Up" => "Whl ↑", "Scroll Down" => "Whl ↓", "Profile +" => "Prof +", "Profile -" => "Prof −",
        "Profile Cycle" => "Prof ⟳", "Disabled" => "✕",
        _ => {
            if let Some(n) = l.strip_prefix("Profile ") { return format!("Prof {n}"); }
            if l.starts_with("Factory function") { return "★".into(); }
            l
        }
    };
    s.to_string()
}

/// egui key -> HID usage (macro recording).
pub fn egui_key_to_hid(k: eframe::egui::Key) -> Option<u8> {
    use eframe::egui::Key::*;
    let name = format!("{k:?}");
    if name.len() == 1 {
        let b = name.as_bytes()[0];
        if b.is_ascii_uppercase() {
            return Some(0x04 + b - b'A');
        }
    }
    Some(match k {
        Num1 => 0x1E, Num2 => 0x1F, Num3 => 0x20, Num4 => 0x21, Num5 => 0x22, Num6 => 0x23,
        Num7 => 0x24, Num8 => 0x25, Num9 => 0x26, Num0 => 0x27,
        Enter => 0x28, Escape => 0x29, Backspace => 0x2A, Tab => 0x2B, Space => 0x2C,
        Minus => 0x2D, Equals => 0x2E, OpenBracket => 0x2F, CloseBracket => 0x30,
        Backslash => 0x31, Semicolon => 0x33, Quote => 0x34, Backtick => 0x35, Comma => 0x36,
        Period => 0x37, Slash => 0x38,
        F1 => 0x3A, F2 => 0x3B, F3 => 0x3C, F4 => 0x3D, F5 => 0x3E, F6 => 0x3F, F7 => 0x40,
        F8 => 0x41, F9 => 0x42, F10 => 0x43, F11 => 0x44, F12 => 0x45,
        Insert => 0x49, Home => 0x4A, PageUp => 0x4B, Delete => 0x4C, End => 0x4D, PageDown => 0x4E,
        ArrowRight => 0x4F, ArrowLeft => 0x50, ArrowDown => 0x51, ArrowUp => 0x52,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dom_codes_map_to_hid() {
        assert_eq!(dom_code_to_hid("KeyA"), Some(0x04));
        assert_eq!(dom_code_to_hid("KeyZ"), Some(0x1D));
        assert_eq!(dom_code_to_hid("Digit1"), Some(0x1E));
        assert_eq!(dom_code_to_hid("Digit0"), Some(0x27));
        assert_eq!(dom_code_to_hid("F1"), Some(0x3A));
        assert_eq!(dom_code_to_hid("F12"), Some(0x45));
        assert_eq!(dom_code_to_hid("F13"), Some(0x68));
        assert_eq!(dom_code_to_hid("MetaLeft"), Some(0xE3));
        assert_eq!(dom_code_to_hid("Space_index1"), Some(0x2C));
        assert_eq!(dom_code_to_hid("Fn"), None);
        assert_eq!(dom_code_to_hid("Volume_Brightness"), None);
    }

    #[test]
    fn labels_for_common_actions() {
        assert_eq!(action_label([0, 0, 0x04, 0]), "A");
        assert_eq!(action_label([0, 0x01, 0x06, 0]), "Copy"); // Ctrl+C is a named function
        assert_eq!(action_label([0, 0x03, 0x05, 0]), "Ctrl+Shift+B");
        assert_eq!(action_label([3, 0, 226, 0]), "Mute");
        assert_eq!(action_label([9, 0, 4, 0]), "Macro 5");
        assert_eq!(action_label([8, 0, 4, 2]), "Profile 3");
        assert_eq!(action_label([0, 0, 0, 0]), "Disabled");
        assert!(is_factory_function([13, 1, 4, 0]));
        assert!(!is_factory_function([3, 0, 226, 0]));
    }

    #[test]
    fn keycaps_stay_short() {
        for cat in CATEGORIES {
            for (name, a) in cat.items {
                let cap = action_cap(*a);
                assert!(cap.chars().count() <= 9, "{name} -> {cap:?} too long for a keycap");
            }
        }
        assert_eq!(action_cap([3, 0, 148, 1]), "PC");
        assert_eq!(action_cap([3, 0, 112, 0]), "Scr −");
    }

    #[test]
    fn picker_entries_are_unique_and_valid() {
        for cat in CATEGORIES {
            let mut names: Vec<_> = cat.items.iter().map(|(n, _)| *n).collect();
            let n = names.len();
            names.sort();
            names.dedup();
            assert_eq!(n, names.len(), "duplicate name in {}", cat.name);
            for (name, a) in cat.items {
                assert!(*a != [0, 0, 0, 0], "{name} would disable the key");
                if a[0] == 0 && a[1] == 0 { assert!(a[2] >= 4, "{name}: invalid HID code"); }
            }
        }
    }

    #[test]
    fn hid_labels_cover_basic_range() {
        for code in 0x04..=0x65u8 {
            assert!(!hid_label(code).starts_with("0x"), "no label for 0x{code:02X}");
        }
        for code in 0xE0..=0xE7u8 {
            assert!(!hid_label(code).starts_with("0x"));
        }
    }
}
