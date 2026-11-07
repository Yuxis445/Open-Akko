//! Open Akko GUI (egui), styled after Akko Cloud Driver v4.

use crate::keycodes::*;
use crate::protocol::{Light, MacroEvent, Options, Sleep, CUSTOM_MODE, LIGHT_MODES, REPORT_RATES, SIDE_LIGHT_MODES};
use crate::worker::{Connected, Req, Resp, State, Worker};
use eframe::egui::{self, Color32, CornerRadius, FontId, Margin, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use serde::{Deserialize, Serialize};
use std::time::Instant;

// ---- palettes (light sampled from the official UI; dark is our own)
pub struct Palette {
    pub purple: Color32, pub purple_light: Color32, pub bg: Color32, pub sidebar: Color32,
    pub card: Color32, pub card2: Color32, pub hover: Color32, pub text: Color32, pub muted: Color32,
    pub key: Color32, pub key_border: Color32, pub modified: Color32, pub disabled: Color32,
    pub factory: Color32, pub chip: Color32, pub unset: Color32, pub toast: Color32, pub ok: Color32,
    pub devcard: Color32,
}
const LIGHT: Palette = Palette {
    purple: Color32::from_rgb(0x65, 0x2C, 0xB4), purple_light: Color32::from_rgb(0xEE, 0xE6, 0xF9),
    bg: Color32::from_rgb(0xF7, 0xF7, 0xF8), sidebar: Color32::from_rgb(0xF0, 0xF0, 0xF2),
    card: Color32::WHITE, card2: Color32::from_rgb(0xF2, 0xF2, 0xF4), hover: Color32::from_rgb(0xE6, 0xE6, 0xEA),
    text: Color32::from_rgb(0x2B, 0x2B, 0x2B), muted: Color32::from_rgb(0x8C, 0x8C, 0x94),
    key: Color32::from_rgb(0x3E, 0x3E, 0x3E), key_border: Color32::from_rgb(0xCF, 0xCF, 0xCF),
    modified: Color32::from_rgb(0xFF, 0x8A, 0x3D), disabled: Color32::from_rgb(0xFE, 0x45, 0x45),
    factory: Color32::from_rgb(0x5A, 0x4E, 0x6E), chip: Color32::from_rgb(0xEC, 0xEC, 0xEE),
    unset: Color32::from_gray(0x9A), toast: Color32::from_rgb(0x2B, 0x2B, 0x33), ok: Color32::from_rgb(0x2E, 0xA0, 0x4F),
    devcard: Color32::WHITE,
};
const DARK: Palette = Palette {
    purple: Color32::from_rgb(0x8A, 0x52, 0xE6), purple_light: Color32::from_rgb(0x36, 0x27, 0x52),
    bg: Color32::from_rgb(0x16, 0x16, 0x1A), sidebar: Color32::from_rgb(0x1E, 0x1E, 0x24),
    card: Color32::from_rgb(0x23, 0x23, 0x2A), card2: Color32::from_rgb(0x2B, 0x2B, 0x33), hover: Color32::from_rgb(0x2C, 0x2C, 0x35),
    text: Color32::from_rgb(0xE8, 0xE8, 0xEE), muted: Color32::from_rgb(0x9A, 0x9A, 0xA6),
    key: Color32::from_rgb(0x38, 0x38, 0x40), key_border: Color32::from_rgb(0x5E, 0x5E, 0x6A),
    modified: Color32::from_rgb(0xFF, 0x96, 0x4F), disabled: Color32::from_rgb(0xFF, 0x5C, 0x5C),
    factory: Color32::from_rgb(0x4C, 0x40, 0x66), chip: Color32::from_rgb(0x30, 0x30, 0x39),
    unset: Color32::from_gray(0x5A), toast: Color32::from_rgb(0x3A, 0x3A, 0x46), ok: Color32::from_rgb(0x4C, 0xC0, 0x6C),
    devcard: Color32::from_rgb(0x2B, 0x2B, 0x33),
};
static IS_DARK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn pal() -> &'static Palette {
    if IS_DARK.load(std::sync::atomic::Ordering::Relaxed) { &DARK } else { &LIGHT }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
enum ThemeChoice { #[default] System, Light, Dark }

#[derive(Serialize, Deserialize, Default)]
struct Settings { #[serde(default)] theme: ThemeChoice }

fn load_settings() -> Settings {
    std::fs::read_to_string(config_dir().join("settings.json")).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_settings(st: &Settings) {
    let d = config_dir();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("settings.json"), serde_json::to_string_pretty(st).unwrap_or_default());
}

fn apply_theme(ctx: &egui::Context, t: ThemeChoice) {
    ctx.options_mut(|o| o.theme_preference = match t {
        ThemeChoice::System => egui::ThemePreference::System,
        ThemeChoice::Light => egui::ThemePreference::Light,
        ThemeChoice::Dark => egui::ThemePreference::Dark,
    });
}

fn style_for(ctx: &egui::Context, theme: egui::Theme) {
    let pal = if theme == egui::Theme::Dark { &DARK } else { &LIGHT };
    ctx.style_mut_of(theme, |style| {
        style.visuals = if theme == egui::Theme::Dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        let v = &mut style.visuals;
        v.panel_fill = pal.bg;
        v.window_fill = pal.card;
        v.extreme_bg_color = pal.card2;
        v.faint_bg_color = pal.card2;
        v.override_text_color = Some(pal.text);
        v.selection.bg_fill = pal.purple;
        v.selection.stroke = Stroke::new(1.0_f32, Color32::WHITE);
        v.hyperlink_color = pal.purple;
        v.widgets.inactive.weak_bg_fill = pal.chip;
        v.widgets.inactive.bg_fill = pal.chip;
        v.widgets.active.bg_fill = pal.purple;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, pal.purple);
        v.slider_trailing_fill = true;
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 5.0);
        style.text_styles.insert(egui::TextStyle::Body, FontId::proportional(14.0));
        style.text_styles.insert(egui::TextStyle::Button, FontId::proportional(13.5));
        style.text_styles.insert(egui::TextStyle::Heading, FontId::proportional(20.0));
    });
}

#[derive(PartialEq, Clone, Copy)]
enum Page { Config, Fn, Macro, Light, Settings, About, Help }

#[derive(PartialEq, Clone, Copy)]
enum PickTab { Cat(usize), Combo, Macro, Disabled }

#[derive(Serialize, Deserialize, Clone, Default)]
struct StoredMacro {
    name: String,
    repeat: u16,
    events: Vec<(u8, bool, u16)>,
}

fn config_dir() -> std::path::PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME").map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    let dir = base.join("openakko");
    let old = base.join("akkohub"); // migrate data from the pre-rename name
    if !dir.exists() && old.exists() { let _ = std::fs::rename(&old, &dir); }
    dir
}

fn load_macros() -> Vec<StoredMacro> {
    std::fs::read_to_string(config_dir().join("macros.json")).ok()
        .and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn save_macros(m: &[StoredMacro]) {
    let d = config_dir();
    let _ = std::fs::create_dir_all(&d);
    let _ = std::fs::write(d.join("macros.json"), serde_json::to_string_pretty(m).unwrap_or_default());
}

fn to_c32(c: u32) -> Color32 { Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8) }
fn from_rgb(c: [u8; 3]) -> u32 { (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32 }

pub struct App {
    worker: Worker,
    dev: Option<Connected>,
    state: Option<State>,
    status: String,
    toast: Option<(String, bool, Instant)>,
    busy: bool,
    page: Page,
    profile: u8,
    base: Vec<Action>,
    fn_layer: Vec<Action>,
    keymap_key: Option<(u8, u8)>,
    fn_index: u8,
    selected: Option<usize>,
    pick_tab: PickTab,
    pending: Option<Action>,
    combo_mods: u8,
    combo_key: u8,
    macro_mode: u8,
    // lighting
    light_edit: Light,
    side_edit: Option<Light>,
    key_colors: Vec<u32>,
    /// colours as last read from / saved to the keyboard (click-again reverts to these)
    key_colors_saved: Vec<u32>,
    key_colors_profile: Option<u8>,
    paint_color: [u8; 3],
    // macros
    macros: Vec<StoredMacro>,
    macro_sel: Option<usize>,
    recording: Option<(Instant, egui::Modifiers)>,
    /// hardware recorder (reads the keyboard's own key reports; None = in-window fallback)
    hw_rec: Option<crate::keyrec::Recorder>,
    default_delay: u16,
    // settings
    options_edit: Options,
    sleep_edit: Sleep,
    debounce_edit: u8,
    confirm_reset: bool,
    logo: Option<egui::TextureHandle>,
    logo_tried: bool,
    settings: Settings,
    /// recent raw battery readings (2.4 GHz); shown as a smoothed median
    battery: std::collections::VecDeque<u8>,
    fw_rx: Option<std::sync::mpsc::Receiver<Result<crate::firmware::FwCheck, String>>>,
    fw_result: Option<Result<crate::firmware::FwCheck, String>>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext) -> Self {
        style_for(&cc.egui_ctx, egui::Theme::Light);
        style_for(&cc.egui_ctx, egui::Theme::Dark);
        let settings = load_settings();
        apply_theme(&cc.egui_ctx, settings.theme);
        install_fonts(&cc.egui_ctx);
        let ctx = cc.egui_ctx.clone();
        // optional start page: AKKOHUB_PAGE=config|fn|macro|light|settings|about
        let page_env = std::env::var("OPENAKKO_PAGE").or_else(|_| std::env::var("AKKOHUB_PAGE")).unwrap_or_default();
        let start = match page_env.as_str() {
            "fn" => Page::Fn, "macro" => Page::Macro, "light" => Page::Light,
            "settings" => Page::Settings, "about" => Page::About, "help" => Page::Help, _ => Page::Config,
        };
        // screenshot/debug hooks (no effect unless set)
        let env_num = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<usize>().ok());
        let preselect = env_num("OPENAKKO_SELECT");
        let premacro = env_num("OPENAKKO_MACRO");
        App {
            worker: Worker::spawn(move || ctx.request_repaint()),
            dev: None, state: None,
            status: "Looking for keyboard…".into(), toast: None, busy: false,
            page: start, profile: 0, base: vec![], fn_layer: vec![], keymap_key: None, fn_index: 0,
            selected: preselect, pick_tab: PickTab::Cat(0), pending: None, combo_mods: 0, combo_key: 0, macro_mode: 0,
            light_edit: Light::default(), side_edit: None, key_colors: vec![], key_colors_saved: vec![], key_colors_profile: None,
            paint_color: [255, 0, 0],
            macros: load_macros(), macro_sel: premacro, recording: None, hw_rec: None, default_delay: 20,
            options_edit: Options::default(), sleep_edit: Sleep { bt: 120, rf: 120, deep_bt: 1680, deep_rf: 1680 },
            debounce_edit: 1, confirm_reset: false, logo: None, logo_tried: false, settings, battery: Default::default(), fw_rx: None, fw_result: None,
        }
    }

    fn set_theme(&mut self, ctx: &egui::Context, t: ThemeChoice) {
        self.settings.theme = t;
        apply_theme(ctx, t);
        save_settings(&self.settings);
    }

    fn req(&self, r: Req) {
        let _ = self.worker.tx.send(r);
    }

    fn toast(&mut self, msg: impl Into<String>, err: bool) {
        self.toast = Some((msg.into(), err, Instant::now()));
    }

    fn poll(&mut self) {
        while let Ok(r) = self.worker.rx.try_recv() {
            match r {
                Resp::Connected(c) => {
                    self.status = format!("{} connected", c.info.display_name);
                    self.battery.clear();
                    self.dev = Some(*c);
                    self.keymap_key = None;
                    self.key_colors_profile = None;
                    self.req(Req::LoadState);
                }
                Resp::Disconnected(m) => {
                    if self.dev.is_some() { self.toast(m.clone(), true); }
                    self.dev = None; self.state = None; self.status = m;
                }
                Resp::State(s) => {
                    self.profile = s.profile.min(3);
                    self.light_edit = s.light;
                    self.side_edit = s.side;
                    self.options_edit = s.options;
                    self.sleep_edit = s.sleep;
                    self.debounce_edit = s.debounce;
                    self.state = Some(*s);
                    self.keymap_key = None;
                }
                Resp::Keymaps { profile, fn_index, base, fn_layer } => {
                    self.base = base; self.fn_layer = fn_layer; self.keymap_key = Some((profile, fn_index));
                }
                Resp::KeyColors { profile, colors } => { self.key_colors_saved = colors.clone(); self.key_colors = colors; self.key_colors_profile = Some(profile); }
                Resp::KeySet { fn_layer, index, action } => {
                    let v = if fn_layer { &mut self.fn_layer } else { &mut self.base };
                    if index < v.len() { v[index] = action; }
                }
                Resp::Battery(b) => {
                    self.battery.push_back(b);
                    while self.battery.len() > 7 { self.battery.pop_front(); }
                }
                Resp::Busy(b) => self.busy = b,
                Resp::Ok(m) => self.toast(m, false),
                Resp::Err(m) => { self.toast(m, true); self.keymap_key = None; }
            }
        }
    }

    fn load_logo(&mut self, ctx: &egui::Context) {
        if self.logo_tried { return; }
        self.logo_tried = true;
        let home = std::env::var("HOME").unwrap_or_default();
        for p in [
            format!("{home}/.local/share/openakko/logo.png"),
            format!("{home}/akko-re/extracted/app/resources/app/company/company_akko/default_logo.png"),
        ] {
            if let Ok(img) = image::open(&p) {
                let img = img.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                self.logo = Some(ctx.load_texture("logo", ci, egui::TextureOptions::LINEAR));
                return;
            }
        }
    }

    // ------------------------------------------------------------------ chrome
    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(36.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("OPEN").size(13.0).strong().color(pal().muted).extra_letter_spacing(6.0));
            if let Some(t) = &self.logo {
                let w = 120.0;
                let h = w * t.size()[1] as f32 / t.size()[0] as f32;
                ui.image((t.id(), Vec2::new(w, h)));
            } else {
                ui.label(RichText::new("Akko").size(38.0).strong().color(pal().purple));
            }
        });
        ui.add_space(48.0);
        let items = [
            (Page::Config, "⌨", "Configuration"),
            (Page::Fn, "Fn", "FnSetting"),
            (Page::Macro, "⏺", "Macro"),
            (Page::Light, "💡", "Light"),
            (Page::Settings, "⚙", "Settings"),
            (Page::About, "ℹ", "About"),
            (Page::Help, "?", "Help"),
        ];
        for (p, icon, label) in items {
            let active = self.page == p;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::click());
            if resp.hovered() || active {
                ui.painter().rect_filled(rect.shrink2(Vec2::new(14.0, 2.0)), 10.0,
                    if active { pal().purple_light } else { pal().hover });
            }
            let col = if active { pal().purple } else { pal().text };
            ui.painter().text(Pos2::new(rect.left() + 42.0, rect.center().y), egui::Align2::CENTER_CENTER,
                icon, FontId::proportional(17.0), col);
            ui.painter().text(Pos2::new(rect.left() + 66.0, rect.center().y), egui::Align2::LEFT_CENTER,
                label, FontId::proportional(16.0), col);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && self.page != p {
                // selection belongs to the page it was made on
                self.page = p; self.selected = None; self.pending = None;
                self.recording = None; self.hw_rec = None; // leaving the page stops recording
            }
        }
        // device card: fixed-size box pinned to the bottom of the sidebar
        let full = ui.max_rect();
        let card_h = 112.0;
        let rect = Rect::from_min_size(
            Pos2::new(full.left() + 16.0, full.bottom() - card_h - 18.0),
            Vec2::new(full.width() - 32.0, card_h),
        );
        if rect.top() > ui.cursor().top() {
            ui.painter().rect(rect, 14.0, pal().devcard, Stroke::new(1.0_f32, pal().hover), StrokeKind::Inside);
            let (name, icon_col, status, status_col) = match &self.dev {
                Some(d) => {
                    let s = match (d.info.connection, smoothed_battery(&self.battery)) {
                        ("USB", _) => "● USB connected".to_string(),
                        (c, Some(b)) => format!("● {c}  ·  🔋 {b}%"),
                        (c, None) => format!("● {c}  ·  🔋 …"),
                    };
                    (d.info.display_name.clone(), pal().purple, s, pal().ok)
                }
                None => ("No device".to_string(), pal().muted, self.status.clone(), pal().muted),
            };
            let c = rect.center().x;
            let painter = ui.painter();
            painter.text(Pos2::new(c, rect.top() + 24.0), egui::Align2::CENTER_CENTER, name,
                FontId::proportional(17.0), if self.dev.is_some() { pal().text } else { pal().muted });
            painter.text(Pos2::new(c, rect.top() + 58.0), egui::Align2::CENTER_CENTER, "⌨",
                FontId::proportional(32.0), icon_col);
            let galley = painter.layout(status, FontId::proportional(12.0), status_col, rect.width() - 20.0);
            let pos = Pos2::new(c - galley.size().x / 2.0, rect.top() + 84.0);
            painter.galley(pos, galley, status_col);
        }
    }

    fn topbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(24.0);
            let title = match self.page {
                Page::Config => "Key Remapping", Page::Fn => "Fn Layer", Page::Macro => "Macro Editor",
                Page::Light => "Lighting", Page::Settings => "Settings", Page::About => "About", Page::Help => "Help & Guide",
            };
            ui.label(RichText::new(title).size(20.0).strong().color(pal().text));
            if self.busy { ui.spinner(); }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(24.0);
                let (icon, tip, next) = match self.settings.theme {
                    ThemeChoice::System => ("◐", "Theme: follow system (click for Light)", ThemeChoice::Light),
                    ThemeChoice::Light => ("☀", "Theme: light (click for Dark)", ThemeChoice::Dark),
                    ThemeChoice::Dark => ("🌙", "Theme: dark (click for System)", ThemeChoice::System),
                };
                if ui.add(egui::Button::new(RichText::new(icon).size(16.0)).fill(pal().chip).corner_radius(8.0)
                    .min_size(Vec2::new(36.0, 30.0))).on_hover_text(tip).clicked() {
                    self.set_theme(ui.ctx(), next);
                }
                if self.dev.is_some() {
                    if ui.add(purple_button("Retrieve Current Setting")).clicked() {
                        self.keymap_key = None; self.key_colors_profile = None;
                        self.req(Req::LoadState);
                    }
                    let profiles = self.dev.as_ref().map(|d| d.info.profiles).unwrap_or(4);
                    let mut p = self.profile;
                    egui::ComboBox::from_id_salt("profile").width(120.0)
                        .selected_text(format!("Profile {}", p + 1))
                        .show_ui(ui, |ui| {
                            for i in 0..profiles { ui.selectable_value(&mut p, i, format!("Profile {}", i + 1)); }
                        });
                    if p != self.profile {
                        self.profile = p;
                        self.selected = None;
                        self.req(Req::SetProfile(p));
                        self.req(Req::LoadState);
                    }
                }
            });
        });
    }

    // --------------------------------------------------------------- keyboard
    /// Draws the keyboard; returns the clicked matrix index.
    fn keyboard(&self, ui: &mut egui::Ui, max_w: f32, label: &dyn Fn(usize) -> (String, Color32, Option<Color32>)) -> Option<usize> {
        let tooltip = |i: usize| -> String {
            let a = match self.page { Page::Fn => self.fn_layer.get(i), _ => self.base.get(i) }.copied().unwrap_or([0; 4]);
            if self.page == Page::Fn && a == [0; 4] { "Not set: Fn + this key sends the normal key".into() } else { action_label(a) }
        };
        let dev = self.dev.as_ref()?;
        let (lw, lh) = dev.layout_size;
        if lw <= 0.0 { return None; }
        // fixed design size like the official app; only shrink when the window is too narrow
        let scale = (max_w / lw).min(1.15);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(lw * scale, lh * scale + 4.0), Sense::hover());
        let painter = ui.painter_at(rect.expand(4.0));
        let mut clicked = None;
        let mut knob_drawn = false;
        for s in &dev.slots {
            let r = Rect::from_min_size(rect.min + Vec2::new(s.x, s.y) * scale, Vec2::new(s.w, s.h) * scale);
            let (text, mut fill, outline) = label(s.index);
            let id = ui.id().with(("key", s.index, s.knob_part));
            let hit = if let Some(part) = s.knob_part {
                let third = r.width() / 3.0;
                Rect::from_min_size(Pos2::new(r.left() + third * part as f32, r.top()), Vec2::new(third, r.height()))
            } else { r };
            let resp = ui.interact(hit, id, Sense::click());
            let selected = matches!(self.page, Page::Config | Page::Fn) && self.selected == Some(s.index);
            if selected { fill = pal().purple; } else if resp.hovered() { fill = fill.gamma_multiply(0.85); }
            if s.round {
                if !knob_drawn {
                    knob_drawn = true;
                    painter.circle_filled(r.center(), r.width() / 2.0, pal().key);
                    painter.circle_stroke(r.center(), r.width() / 2.0, Stroke::new(2.0_f32, pal().key_border));
                }
                let third = r.width() / 3.0;
                let cx = r.left() + third * (s.knob_part.unwrap_or(1) as f32 + 0.5);
                if selected || resp.hovered() {
                    painter.circle_filled(Pos2::new(cx, r.center().y), third * 0.45, if selected { pal().purple } else { Color32::from_gray(90) });
                }
                let glyph = match s.knob_part { Some(0) => "⟲", Some(2) => "⟳", _ => "◉" };
                painter.text(Pos2::new(cx, r.center().y), egui::Align2::CENTER_CENTER, glyph,
                    FontId::proportional(11.0 * scale.max(0.8)), outline.unwrap_or(Color32::WHITE));
                resp.clone().on_hover_text(match s.knob_part { Some(0) => format!("Knob ⟲: {text}"),
                    Some(2) => format!("Knob ⟳: {text}"), _ => format!("Knob press: {text}") });
            } else {
                let cr = CornerRadius::same((5.0 * scale) as u8);
                let lum_b = 0.299 * fill.r() as f32 + 0.587 * fill.g() as f32 + 0.114 * fill.b() as f32;
                let border = outline.unwrap_or(if lum_b > 200.0 { Color32::from_gray(0x70) } else { pal().key_border });
                painter.rect(r, cr, fill, Stroke::new(1.5_f32, border), StrokeKind::Inside);
                // fit the label inside the key: shrink the font, then truncate with "…"
                let max_w = r.width() - 6.0 * scale.max(0.8);
                let mut fs = 11.5 * scale.clamp(0.8, 1.4);
                let min_fs = 8.5 * scale.clamp(0.8, 1.2);
                let width_of = |t: &str, f: f32| painter.layout_no_wrap(t.to_string(), FontId::proportional(f), Color32::WHITE).size().x;
                while fs > min_fs && width_of(&text, fs) > max_w { fs -= 0.5; }
                let mut text = text;
                if width_of(&text, fs) > max_w {
                    let chars: Vec<char> = text.chars().collect();
                    let mut n = chars.len();
                    while n > 1 && width_of(&(chars[..n].iter().collect::<String>() + "…"), fs) > max_w { n -= 1; }
                    text = chars[..n].iter().collect::<String>() + "…";
                }
                // keycap text depends only on the key colour (perceived luminance), never on the theme
                let lum = 0.299 * fill.r() as f32 + 0.587 * fill.g() as f32 + 0.114 * fill.b() as f32;
                let tc = if lum > 150.0 { Color32::from_rgb(0x10, 0x10, 0x10) } else { Color32::WHITE };
                painter.text(r.center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(fs), tc);
            }
            let resp = if s.round { resp } else { resp.on_hover_text(format!("{} (slot {})", tooltip(s.index), s.index)) };
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() { clicked = Some(s.index); }
        }
        clicked
    }

    fn ensure_keymaps(&mut self) {
        let want = (self.profile, self.fn_index);
        if self.dev.is_some() && self.state.is_some() && self.keymap_key != Some(want) {
            self.keymap_key = Some(want);
            self.req(Req::LoadKeymaps { profile: want.0, fn_index: want.1 });
        }
    }

    // ------------------------------------------------------------- remap pages
    fn remap_page(&mut self, ui: &mut egui::Ui, fn_layer: bool) {
        self.ensure_keymaps();
        let Some(dev) = self.dev.clone() else { return };
        let current = if fn_layer { self.fn_layer.clone() } else { self.base.clone() };
        let defaults = if fn_layer { dev.fn_defaults.clone() } else { dev.defaults.clone() };
        let get = |i: usize| current.get(i).copied().unwrap_or([0; 4]);
        let def = |i: usize| defaults.get(i).copied().unwrap_or([0; 4]);

        if fn_layer {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Fn layer").color(pal().muted));
                for i in 0..2u8 {
                    if chip(ui, &format!("Fn layer {}", i + 1), self.fn_index == i).clicked() && self.fn_index != i {
                        self.fn_index = i; self.selected = None;
                    }
                }
                ui.label(RichText::new("Fn layers are shared by all profiles. Fn layer 1 is what Fn + key uses.").color(pal().muted).size(12.5));
            });
            ui.add_space(6.0);
        }
        card(ui, |ui| {
            ui.vertical_centered(|ui| {
                let w = ui.available_width() * 0.92;
                let label = |i: usize| {
                    let a = get(i);
                    let base_def = dev.defaults.get(i).copied().unwrap_or([0; 4]);
                    let modified = a != def(i);
                    let factory = fn_layer && is_factory_function(a);
                    let text = if fn_layer && a == [0, 0, 0, 0] { action_cap(base_def) }
                        else if factory { format!("★ {}", action_cap(base_def)) } else { action_cap(a) };
                    let fill = if fn_layer && a == [0, 0, 0, 0] { pal().unset }
                        else if factory { pal().factory } else if a == [0; 4] { pal().disabled } else { pal().key };
                    (text, fill, modified.then_some(pal().modified))
                };
                if let Some(i) = self.keyboard(ui, w, &label) {
                    self.selected = Some(i);
                    self.pending = None;
                }
            });
            ui.horizontal(|ui| {
                legend(ui, pal().modified, "Modified key");
                legend(ui, pal().disabled, "Disabled key");
                if fn_layer {
                    legend(ui, pal().unset, "Not set on Fn layer");
                    legend(ui, pal().factory, "★ Factory Fn function (hover the key for its code)");
                }
            });
        });
        ui.add_space(10.0);

        card(ui, |ui| {
            ui.horizontal_top(|ui| {
                // category tabs
                egui::Frame::new().fill(pal().card2).corner_radius(10.0)
                    .inner_margin(Margin::symmetric(8, 10)).show(ui, |ui| {
                        ui.set_width(150.0);
                        ui.set_min_height(230.0);
                        ui.vertical(|ui| {
                        let mut tabs: Vec<(PickTab, &str)> = CATEGORIES.iter().enumerate().map(|(i, c)| (PickTab::Cat(i), c.name)).collect();
                        tabs.push((PickTab::Combo, "Combination"));
                        tabs.push((PickTab::Macro, "Macro"));
                        tabs.push((PickTab::Disabled, "Disabled"));
                        for (t, n) in tabs {
                            let sel = self.pick_tab == t;
                            let txt = RichText::new(n).color(if sel { pal().purple } else { pal().muted }).size(14.0);
                            if ui.add(egui::Button::new(if sel { txt.strong() } else { txt })
                                .fill(if sel { pal().card } else { Color32::TRANSPARENT }).stroke(Stroke::NONE)
                                .min_size(Vec2::new(134.0, 30.0))).clicked() { self.pick_tab = t; }
                        }
                        });
                    });
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    let Some(sel) = self.selected else {
                        ui.label(RichText::new("*Select a key on the virtual keyboard above to start").color(pal().disabled));
                        ui.label(RichText::new("*Combination keys can use modifiers plus up to 2 keys").color(pal().disabled));
                        return;
                    };
                    let slot = dev.slots.iter().find(|s| s.index == sel);
                    let phys = match slot.and_then(|s| s.knob_part) {
                        Some(0) => "Knob ⟲ (turn left)".to_string(), Some(2) => "Knob ⟳ (turn right)".to_string(), Some(_) => "Knob press".to_string(),
                        None => action_label(dev.defaults.get(sel).copied().unwrap_or([0; 4])),
                    };
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("Key: {}{}", if fn_layer { "Fn + " } else { "" }, phys)).strong());
                        ui.label(RichText::new(format!("→ now: {}", action_label(get(sel)))).color(pal().muted));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let can = self.pending.is_some() && !self.busy;
                            if ui.add_enabled(can, purple_button("Confirm").min_size(Vec2::new(140.0, 34.0))).clicked() {
                                if let Some(a) = self.pending.take() { self.apply_key(fn_layer, sel, a); }
                            }
                            if ui.add_enabled(!self.busy, egui::Button::new("Reset key")).clicked() {
                                self.apply_key(fn_layer, sel, def(sel));
                            }
                            if let Some(p) = self.pending { ui.label(RichText::new(format!("New: {}", action_label(p))).color(pal().purple).strong()); }
                        });
                    });
                    ui.separator();
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| self.picker(ui));
                });
            });
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(!self.busy, egui::Button::new(RichText::new("⟲ Restore this layer to default").color(pal().muted)).frame(false)).clicked() {
                        let changed: Vec<usize> = dev.slots.iter().map(|s| s.index).filter(|&i| get(i) != def(i)).collect();
                        for i in changed { self.apply_key(fn_layer, i, def(i)); }
                    }
                });
            });
        });
    }

    fn apply_key(&mut self, fn_layer: bool, index: usize, a: Action) {
        let layer = if fn_layer { self.fn_index } else { self.profile };
        self.req(Req::SetKey { fn_layer, profile: layer, index, action: a });
        let v = if fn_layer { &mut self.fn_layer } else { &mut self.base };
        if index < v.len() { v[index] = a; }
    }

    fn picker(&mut self, ui: &mut egui::Ui) {
        match self.pick_tab {
            PickTab::Cat(i) => {
                let items = CATEGORIES[i].items;
                ui.horizontal_wrapped(|ui| {
                    for (name, a) in items {
                        if chip(ui, name, self.pending == Some(*a)).clicked() { self.pending = Some(*a); }
                    }
                });
            }
            PickTab::Combo => {
                ui.horizontal_wrapped(|ui| {
                    for (bit, n) in MOD_NAMES.iter().enumerate() {
                        let on = self.combo_mods & (1 << bit) != 0;
                        if chip(ui, n, on).clicked() { self.combo_mods ^= 1 << bit; }
                    }
                });
                ui.label(RichText::new("+ key:").color(pal().muted));
                ui.horizontal_wrapped(|ui| {
                    for (name, a) in CAT_LETTERS.iter().chain(CAT_CONTROL.iter()) {
                        if a[2] >= 0xE0 { continue; }
                        if chip(ui, name, self.combo_key == a[2]).clicked() { self.combo_key = a[2]; }
                    }
                });
                if self.combo_key != 0 || self.combo_mods != 0 { self.pending = Some([0, self.combo_mods, self.combo_key, 0]); }
            }
            PickTab::Macro => {
                if self.macros.is_empty() {
                    ui.label(RichText::new("No macros yet: create one on the Macro page.").color(pal().muted));
                    return;
                }
                ui.horizontal(|ui| {
                    ui.label("Play mode:");
                    ui.selectable_value(&mut self.macro_mode, 0, "Repeat count");
                    ui.selectable_value(&mut self.macro_mode, 1, "Toggle on/off");
                    ui.selectable_value(&mut self.macro_mode, 2, "While held");
                });
                let mut assign = None;
                ui.horizontal_wrapped(|ui| {
                    for (i, m) in self.macros.iter().enumerate() {
                        if chip(ui, &m.name, false).clicked() { assign = Some(i); }
                    }
                });
                if let (Some(i), Some(sel)) = (assign, self.selected) {
                    let m = &self.macros[i];
                    if m.events.is_empty() {
                        self.toast("This macro is empty: record or add events on the Macro page first", true);
                    } else {
                        let fn_layer = self.page == Page::Fn;
                        let events = m.events.iter().map(|&(code, down, d)| MacroEvent { code, down, delay_ms: d }).collect();
                        let profiles = self.dev.as_ref().map(|d| d.info.profiles).unwrap_or(4);
                        let layer = if fn_layer { self.fn_index } else { self.profile };
                        self.req(Req::AssignMacro { fn_layer, layer, index: sel, mode: self.macro_mode, events, repeat: m.repeat.max(1), profiles });
                    }
                }
            }
            PickTab::Disabled => {
                if chip(ui, "Disable this key", self.pending == Some([0; 4])).clicked() { self.pending = Some([0; 4]); }
            }
        }
    }

    // ---------------------------------------------------------------- lighting
    fn light_page(&mut self, ui: &mut egui::Ui) {
        let custom = self.light_edit.mode == CUSTOM_MODE;
        if custom && self.dev.is_some() && self.key_colors_profile != Some(self.profile) {
            self.key_colors_profile = Some(self.profile);
            self.req(Req::LoadKeyColors(self.profile));
        }
        card(ui, |ui| {
            ui.vertical_centered(|ui| {
                let w = ui.available_width() * 0.92;
                let colors = self.key_colors.clone();
                let lc = to_c32(self.light_edit.color);
                let label = |i: usize| -> (String, Color32, Option<Color32>) {
                    let txt = self.dev.as_ref().map(|d| action_cap(d.defaults.get(i).copied().unwrap_or([0; 4]))).unwrap_or_default();
                    if custom { (txt, to_c32(colors.get(i).copied().unwrap_or(0)), None) }
                    else if self.light_edit.mode == 0 { (txt, pal().key, None) }
                    else if self.light_edit.rainbow { (txt, pal().key, Some(Color32::from_rgb(0xB0, 0x7C, 0xFF))) }
                    else { (txt, pal().key, Some(lc)) }
                };
                if let Some(i) = self.keyboard(ui, w, &label) {
                    if custom && i < self.key_colors.len() {
                        // click toggles: paint with the current colour, click again to turn the key off
                        let c = from_rgb(self.paint_color);
                        let before = self.key_colors_saved.get(i).copied().unwrap_or(0);
                        self.key_colors[i] = if self.key_colors[i] == c { before } else { c };
                    }
                }
            });
            if custom {
                ui.label(RichText::new("Custom mode: pick a colour, click a key to paint it, click again to restore its previous colour.").color(pal().muted));
                ui.horizontal_wrapped(|ui| {
                    ui.color_edit_button_srgb(&mut self.paint_color);
                    for c in [0xFF0000u32, 0xFF8000, 0xFFFF00, 0x00FF00, 0x00FFFF, 0x0000FF, 0xFF00FF, 0xFFFFFF, 0x000000] {
                        let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                        ui.painter().rect(r, 4.0, to_c32(c), Stroke::new(1.0_f32, pal().key_border), StrokeKind::Inside);
                        if resp.clicked() { self.paint_color = [(c >> 16) as u8, (c >> 8) as u8, c as u8]; }
                    }
                    if ui.button("Fill all").clicked() { let c = from_rgb(self.paint_color); self.key_colors.iter_mut().for_each(|x| *x = c); }
                    if ui.add_enabled(!self.busy, purple_button("Save colours")).clicked() {
                        self.req(Req::SetKeyColors { profile: self.profile, colors: self.key_colors.clone() });
                        self.key_colors_saved = self.key_colors.clone();
                    }
                });
            }
        });
        ui.add_space(10.0);
        ui.columns(2, |cols| {
            let mut l = self.light_edit;
            card(&mut cols[0], |ui| {
                ui.label(RichText::new("Keyboard lighting").strong().size(16.0));
                if let Some(apply) = light_editor(ui, &mut l, LIGHT_MODES, "main") {
                    if apply { self.req(Req::SetLight(l)); }
                }
            });
            self.light_edit = l;
            card(&mut cols[1], |ui| {
                ui.label(RichText::new("Side lighting").strong().size(16.0));
                match self.side_edit {
                    Some(mut s2) => {
                        if let Some(true) = light_editor(ui, &mut s2, SIDE_LIGHT_MODES, "side") { self.req(Req::SetSideLight(s2)); }
                        self.side_edit = Some(s2);
                    }
                    None => { ui.label(RichText::new("This keyboard has no side light.").color(pal().muted)); }
                }
            });
        });
    }

    // ------------------------------------------------------------------ macros
    fn macro_page(&mut self, ui: &mut egui::Ui) {
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Macro list").strong().size(16.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(purple_button("+ New macro")).clicked() {
                            self.macros.push(StoredMacro { name: format!("Macro {}", self.macros.len() + 1), repeat: 1, events: vec![] });
                            self.macro_sel = Some(self.macros.len() - 1);
                            save_macros(&self.macros);
                        }
                    });
                });
                ui.separator();
                if self.macros.is_empty() {
                    ui.label(RichText::new("No macros yet. Click \"+ New macro\", then Record.").color(pal().muted));
                }
                let mut del = None;
                for (i, m) in self.macros.iter().enumerate() {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.macro_sel == Some(i), format!("{}   ({} events)", m.name, m.events.len())).clicked() {
                            self.macro_sel = Some(i);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("🗑").clicked() { del = Some(i); }
                        });
                    });
                }
                if let Some(i) = del { self.macros.remove(i); self.macro_sel = None; save_macros(&self.macros); }
                ui.add_space(6.0);
                ui.label(RichText::new("Assign a macro to a key in Configuration → Macro.\nMacros are stored locally and written to the keyboard when assigned.").color(pal().muted).size(12.5));
            });
            card(&mut cols[1], |ui| self.macro_editor(ui));
        });
    }

    fn macro_editor(&mut self, ui: &mut egui::Ui) {
        let Some(i) = self.macro_sel.filter(|&i| i < self.macros.len()) else {
            ui.label(RichText::new("Select or create a macro").color(pal().muted));
            ui.label(RichText::new("Tip: see Help → Macro for recording and play modes.").color(pal().muted).size(12.5));
            return;
        };
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Name");
            changed |= ui.text_edit_singleline(&mut self.macros[i].name).changed();
        });
        {
            let evs: Vec<MacroEvent> = self.macros[i].events.iter().map(|&(code, down, d)| MacroEvent { code, down, delay_ms: d }).collect();
            let size = 2 + evs.iter().map(|e| if e.delay_ms.max(1) <= 127 { 2 } else { 4 }).sum::<usize>();
            let max = crate::protocol::MACRO_MAX_BYTES - 4;
            let col = if size > max { pal().disabled } else if size * 10 > max * 8 { pal().modified } else { pal().muted };
            ui.label(RichText::new(format!("Size: {size} / {max} bytes{}", if size > max { "  (too long for the keyboard)" } else { "" })).color(col).size(12.5));
        }
        ui.horizontal(|ui| {
            ui.label("Repeat");
            changed |= ui.add(egui::DragValue::new(&mut self.macros[i].repeat).range(1..=65535)).changed();
            ui.label("Default delay");
            ui.add(egui::DragValue::new(&mut self.default_delay).range(0..=10000).suffix(" ms"));
        });
        ui.horizontal(|ui| {
            if self.recording.is_some() {
                if ui.add(egui::Button::new(RichText::new("■ Stop recording").color(Color32::WHITE)).fill(pal().disabled)).clicked() {
                    self.recording = None;
                    self.hw_rec = None;
                }
                let msg = if self.hw_rec.is_some() { "Recording from the keyboard… type your macro (all keys incl. Win)" }
                          else { "Recording… press keys in this window (Win key not available)" };
                ui.label(RichText::new(msg).color(pal().disabled));
            } else if ui.add(purple_button("⏺ Record")).clicked() {
                self.macros[i].events.clear();
                self.hw_rec = self.dev.as_ref().and_then(|d| crate::keyrec::start(&d.info.path).ok());
                self.recording = Some((Instant::now(), ui.input(|i| i.modifiers)));
                changed = true;
            }
            if ui.button("Clear").clicked() { self.macros[i].events.clear(); changed = true; }
        });
        // recording
        if let Some((t0, mods0)) = self.recording {
            let (events, mods) = ui.input(|inp| (inp.events.clone(), inp.modifiers));
            // t0 = time of the previous recorded event; only advances when an event is recorded
            let mut last_t = t0;
            let default_delay = self.default_delay;
            let mut push = |m: &mut StoredMacro, code: u8, down: bool, at: Instant| {
                if let Some(last) = m.events.last_mut() { last.2 = (at.saturating_duration_since(last_t).as_millis().min(60000) as u16).max(1); }
                m.events.push((code, down, default_delay));
                last_t = at;
            };
            let m = &mut self.macros[i];
            if let Some(rec) = &self.hw_rec {
                // exact events from the keyboard itself
                while let Ok((code, down, at)) = rec.rx.try_recv() { push(m, code, down, at); changed = true; }
            } else {
                // fallback: what the GUI toolkit reports (no Win key)
                let now = Instant::now();
                for (code, was, is) in [(0xE0, mods0.ctrl, mods.ctrl), (0xE1, mods0.shift, mods.shift), (0xE2, mods0.alt, mods.alt)] {
                    if was != is { push(m, code, is, now); changed = true; }
                }
                for e in events {
                    if let egui::Event::Key { key, pressed, repeat: false, .. } = e {
                        if let Some(c) = egui_key_to_hid(key) { push(m, c, pressed, now); changed = true; }
                    }
                }
            }
            self.recording = Some((last_t, mods));
            ui.ctx().request_repaint();
        }
        ui.separator();
        let mut remove = None;
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            egui::Grid::new("events").striped(true).num_columns(4).show(ui, |ui| {
                for (j, ev) in self.macros[i].events.iter_mut().enumerate() {
                    ui.label(RichText::new(hid_label(ev.0)).strong());
                    ui.label(if ev.1 { RichText::new("↓ press").color(pal().purple) } else { RichText::new("↑ release").color(pal().muted) });
                    changed |= ui.add(egui::DragValue::new(&mut ev.2).range(1..=60000).suffix(" ms")).changed();
                    if ui.small_button("✖").clicked() { remove = Some(j); }
                    ui.end_row();
                }
            });
        });
        if let Some(j) = remove { self.macros[i].events.remove(j); changed = true; }
        ui.horizontal(|ui| {
            ui.label("Add:");
            egui::ComboBox::from_id_salt("addkey").selected_text("key…").show_ui(ui, |ui| {
                for (n, a) in CAT_LETTERS.iter().chain(CAT_CONTROL.iter()) {
                    if ui.selectable_label(false, *n).clicked() {
                        let d = self.default_delay;
                        self.macros[i].events.push((a[2], true, d));
                        self.macros[i].events.push((a[2], false, d));
                        changed = true;
                    }
                }
            });
        });
        if changed { save_macros(&self.macros); }
    }

    // ---------------------------------------------------------------- settings
    fn settings_page(&mut self, ui: &mut egui::Ui) {
        let Some(dev) = self.dev.clone() else { return };
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                ui.label(RichText::new("Performance").strong().size(16.0));
                ui.add_space(4.0);
                egui::Grid::new("perf").num_columns(2).spacing([18.0, 12.0]).show(ui, |ui| {
                    ui.label("Polling rate");
                    if dev.has_report_rate {
                        let cur = self.state.as_ref().and_then(|s| s.report_rate).unwrap_or(1000);
                        ui.horizontal(|ui| {
                            for (_, hz) in REPORT_RATES {
                                if chip(ui, &format!("{hz}"), cur == *hz).clicked() {
                                    self.req(Req::SetReportRate(*hz));
                                    if let Some(s) = self.state.as_mut() { s.report_rate = Some(*hz); }
                                }
                            }
                        });
                    } else {
                        ui.label(RichText::new("1000 Hz (fixed on this model)").color(pal().muted));
                    }
                    ui.end_row();
                    ui.label("Debounce");
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut self.debounce_edit, 0..=20).suffix(" ms"));
                        if ui.add(purple_button("Apply")).clicked() { self.req(Req::SetDebounce(self.debounce_edit)); }
                    });
                    ui.end_row();
                });
                ui.add_space(10.0);
                ui.label(RichText::new("Wireless sleep").strong().size(16.0));
                egui::Grid::new("sleep").num_columns(2).spacing([18.0, 10.0]).show(ui, |ui| {
                    let s = &mut self.sleep_edit;
                    ui.label("Sleep after (2.4G)"); ui.add(egui::DragValue::new(&mut s.rf).range(30..=3600).suffix(" s")); ui.end_row();
                    ui.label("Sleep after (Bluetooth)"); ui.add(egui::DragValue::new(&mut s.bt).range(30..=3600).suffix(" s")); ui.end_row();
                    ui.label("Deep sleep (2.4G)"); ui.add(egui::DragValue::new(&mut s.deep_rf).range(60..=7200).suffix(" s")); ui.end_row();
                    ui.label("Deep sleep (Bluetooth)"); ui.add(egui::DragValue::new(&mut s.deep_bt).range(60..=7200).suffix(" s")); ui.end_row();
                });
                if ui.add(purple_button("Apply sleep timers")).clicked() { self.req(Req::SetSleep(self.sleep_edit)); }
                ui.add_space(12.0);
                self.firmware_section(ui, &dev);
            });
            card(&mut cols[1], |ui| {
                ui.label(RichText::new("Appearance").strong().size(16.0));
                ui.horizontal(|ui| {
                    for (t, n) in [(ThemeChoice::System, "◐ System"), (ThemeChoice::Light, "☀ Light"), (ThemeChoice::Dark, "🌙 Dark")] {
                        if chip(ui, n, self.settings.theme == t).clicked() { self.set_theme(ui.ctx(), t); }
                    }
                });
                ui.add_space(12.0);
                ui.label(RichText::new("Keyboard options").strong().size(16.0));
                let o = &mut self.options_edit;
                let mut ch = false;
                ch |= ui.checkbox(&mut o.win_lock, "Lock Windows/Super key").changed();
                ch |= ui.checkbox(&mut o.mac_mode, "Mac mode").changed();
                ch |= ui.checkbox(&mut o.wasd_swap, "Swap WASD and arrow keys").changed();
                ch |= ui.checkbox(&mut o.fn_swap, "Swap Fn layer (F-keys ⇄ media)").changed();
                ch |= ui.checkbox(&mut o.power_save, "Power-saving mode").changed();
                if ch { self.req(Req::SetOptions(self.profile, self.options_edit)); }
                ui.add_space(16.0);
                ui.label(RichText::new("Reset").strong().size(16.0));
                if !self.confirm_reset {
                    if ui.add(egui::Button::new(RichText::new("⟲ Restore Factory Settings").color(pal().disabled))).clicked() { self.confirm_reset = true; }
                } else {
                    ui.label(RichText::new("This erases all keymaps, macros and lighting on the keyboard.").color(pal().disabled));
                    ui.horizontal(|ui| {
                        if ui.add(egui::Button::new(RichText::new("Yes, reset").color(Color32::WHITE)).fill(pal().disabled)).clicked() {
                            self.confirm_reset = false;
                            self.req(Req::FactoryReset);
                            self.req(Req::LoadState);
                        }
                        if ui.button("Cancel").clicked() { self.confirm_reset = false; }
                    });
                }
            });
        });
    }

    fn firmware_section(&mut self, ui: &mut egui::Ui, dev: &Connected) {
        if let Some(rx) = &self.fw_rx {
            if let Ok(r) = rx.try_recv() { self.fw_result = Some(r); self.fw_rx = None; }
        }
        ui.label(RichText::new("Firmware").strong().size(16.0));
        let fw = dev.info.firmware;
        ui.label(format!("Installed: v{:x}  (V{}.{:02})", fw, fw >> 8, fw & 0xFF));
        ui.horizontal(|ui| {
            let checking = self.fw_rx.is_some();
            if ui.add_enabled(!checking, purple_button("Check for updates")).clicked() {
                let (tx, rx) = std::sync::mpsc::channel();
                let (id, ctx) = (dev.info.device_id, ui.ctx().clone());
                std::thread::spawn(move || { let _ = tx.send(crate::firmware::check(id, fw)); ctx.request_repaint(); });
                self.fw_rx = Some(rx);
                self.fw_result = None;
            }
            if checking { ui.spinner(); ui.label(RichText::new("Asking Akko's server…").color(pal().muted)); }
        });
        match &self.fw_result {
            Some(Ok(c)) if c.update_available => {
                ui.label(RichText::new(format!("⬆ Update available: {}", c.latest)).strong().color(pal().modified));
                ui.label(RichText::new("Open Akko does not flash firmware (bricking risk). Install it with the official Akko Cloud Driver on Windows.").color(pal().muted).size(12.5));
            }
            Some(Ok(c)) => { ui.label(RichText::new(format!("✔ Up to date (latest is {})", c.latest)).strong().color(pal().ok)); }
            Some(Err(e)) => { ui.label(RichText::new(format!("Check failed: {e}")).color(pal().disabled)); }
            None => {}
        }
        ui.label(RichText::new(format!("Only sent when you click: the model ID ({}) to api2.rongyuan.tech (Akko's server). Nothing is checked automatically.", dev.info.device_id)).color(pal().muted).size(12.0));
    }

    fn about_page(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            if let Some(d) = &self.dev {
                egui::Grid::new("about").num_columns(2).spacing([24.0, 10.0]).show(ui, |ui| {
                    for (k, v) in [
                        ("Device", d.info.display_name.clone()),
                        ("Model", d.info.model.clone()),
                        ("Device ID", d.info.device_id.to_string()),
                        ("Firmware", format!("V{}.{:02}", d.info.firmware >> 8, d.info.firmware & 0xFF)),
                        ("Connection", d.info.connection.to_string()),
                        ("USB", format!("3151:{:04x}  {}", d.info.pid, d.info.usb_name.trim())),
                        ("Interface", d.info.path.clone()),
                        ("Profiles", d.info.profiles.to_string()),
                        ("Supported", if d.info.supported { "yes".into() } else { "untested model, basic features only".into() }),
                    ] { ui.label(RichText::new(k).color(pal().muted)); ui.label(RichText::new(v).strong()); ui.end_row(); }
                });
                ui.add_space(12.0);
            }
            ui.label(RichText::new(format!("Open Akko {}", env!("CARGO_PKG_VERSION"))).strong());
            ui.label(RichText::new("Unofficial native Linux configurator for Akko keyboards.\nProtocol reverse engineered from Akko Cloud Driver v4. Offline: no account, no telemetry. The only network use is the optional firmware check in Settings, and only when you click it.").color(pal().muted));
        });
    }
}

/// System fonts: Noto Sans for text, DejaVu / Noto Symbols for arrows and symbols,
/// Noto CJK as last resort; egui's built-in fonts stay as final fallback.
fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        ("noto", "/usr/share/fonts/noto/NotoSans-Regular.ttf"),
        ("dejavu", "/usr/share/fonts/TTF/DejaVuSans.ttf"),
        ("symbols2", "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf"),
        ("symbols", "/usr/share/fonts/noto/NotoSansSymbols-Regular.ttf"),
    ];
    let mut order = vec![];
    for (name, path) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            order.push(name.to_string());
        }
    }
    let prop = fonts.families.entry(egui::FontFamily::Proportional).or_default();
    for (i, n) in order.iter().enumerate() { prop.insert(i, n.clone()); }
    let mono = fonts.families.entry(egui::FontFamily::Monospace).or_default();
    for n in order.iter().skip(1) { mono.push(n.clone()); }
    ctx.set_fonts(fonts);
}

/// Median of the recent readings, rounded to 5 %; None until 3 samples exist.
fn smoothed_battery(v: &std::collections::VecDeque<u8>) -> Option<u8> {
    if v.len() < 3 { return None; }
    let mut s: Vec<u8> = v.iter().copied().collect();
    s.sort();
    let m = s[s.len() / 2] as u32;
    Some((((m + 2) / 5) * 5).min(100) as u8)
}

// -------------------------------------------------------------------- help
const HELP: &[(&str, &str, &[&str])] = &[
    ("🔌", "Getting started", &[
        "Connect the keyboard with a USB cable, or plug in the 2.4 GHz receiver with the keyboard in 2.4 GHz mode (press a key to wake it). Bluetooth configuration is not supported yet.",
        "The card at the bottom of the sidebar shows the detected model and \"● USB connected\". Open Akko reconnects by itself when you replug.",
        "If it says \"Permission denied\", install the udev rule once: sudo cp 70-akko.rules /etc/udev/rules.d/ && sudo udevadm control --reload && sudo udevadm trigger, then replug.",
        "Settings are stored inside the keyboard, so they keep working on any computer, even without Open Akko.",
    ]),
    ("🗂", "Profiles", &[
        "Your keyboard has 4 profiles. The picker in the top bar switches the ACTIVE profile on the keyboard.",
        "Each profile has its own keymap, lighting and options. Edits always go to the profile shown in the picker.",
        "\"Retrieve Current Setting\" re-reads everything from the keyboard, e.g. after you changed something with Fn shortcuts.",
    ]),
    ("⌨", "Configuration (key remapping)", &[
        "Click a key on the virtual keyboard, pick the new action below, then press Confirm.",
        "Basic / Control & F-keys: any normal key. Media: volume, play, calculator… Mouse: clicks and scroll. Function: Fn, LED brightness, profile switching, shortcuts like Copy or Lock.",
        "Combination: tick modifiers (Ctrl, Shift, Alt, Win) and pick a key, e.g. Ctrl+Shift+Esc.",
        "Macro: bind a macro from your library (see Macro). Disabled: the key does nothing.",
        "Orange outline = changed from factory. Red = disabled. Hover any key to see its full action.",
        "\"Reset key\" restores one key; \"Restore this layer to default\" restores all keys of this profile.",
        "The knob has three parts: turn left, press, turn right; each can be remapped.",
    ]),
    ("Fn", "FnSetting (Fn layer)", &[
        "What a key does while you hold Fn. Fn layers are shared by all profiles (they are not per profile).",
        "Fn layer 1 is what the Fn key uses. Fn layer 2 is a second layer reachable only if a key is mapped to it.",
        "★ keys are Akko's built-in Fn functions (lighting, wireless channels, etc.). Gray keys are not set and send their normal key.",
        "Example: select P, choose Control & F-keys → PrtSc, Confirm: Fn+P now takes a screenshot.",
    ]),
    ("⏺", "Macro", &[
        "Create a macro with \"+ New macro\", press Record, type the keys on your Akko keyboard, then press Stop.",
        "Recording reads the keyboard itself, so every key works (including Win) with your real timing, whichever window has focus.",
        "Each event has a delay (time until the next event); edit it in the list, or use \"Add\" to insert keys by hand.",
        "Repeat sets how many times the macro plays. Macros are saved on this computer until you assign them.",
        "Assign in Configuration (or FnSetting) → select a key → Macro tab → pick the play mode → click the macro. It is then written to the keyboard (up to 50 macros).",
        "Play modes: Repeat count (plays N times), Toggle on/off (press to start, press again to stop), While held (repeats while the key is held).",
    ]),
    ("💡", "Light", &[
        "Choose an effect, colour (or Rainbow), brightness, speed and direction, then press Apply.",
        "Custom effect = per-key colours: pick a colour, click a key to paint it, click it again to restore its previous colour, then press Save colours.",
        "Side lighting has its own effects on the right (if your model has a side light bar).",
    ]),
    ("⚙", "Settings", &[
        "Debounce: how long a key must be stable to register (lower = faster, higher = fewer double presses).",
        "Keyboard options: lock the Windows/Super key, Mac mode, swap WASD with arrows, swap the Fn layer, power saving.",
        "Wireless sleep timers apply to 2.4 GHz and Bluetooth use.",
        "Appearance: System (follows your desktop), Light or Dark theme. Also available from the button at the top right.",
        "Firmware: \"Check for updates\" asks Akko's server for the latest version (only when clicked). Open Akko never flashes firmware.",
        "Restore Factory Settings erases all keymaps, macros and lighting on the keyboard.",
    ]),
    (">_", "Command line & self-test", &[
        "openakko help: all commands. openakko info: device and settings. openakko keymap / keymap --fn: show layers (* = changed).",
        "openakko light wave rainbow 4 3 · openakko remap <slot> Mute · openakko fwcheck.",
        "openakko selftest checks everything read-only; openakko selftest --write also changes each setting, reads it back and restores it.",
        "openakko record 5 prints the keys you press for 5 seconds, straight from the keyboard (handy to test macro recording).",
    ]),
    ("🩺", "Troubleshooting", &[
        "Nothing changes on the keyboard: check the toast at the bottom for errors, then run openakko selftest.",
        "Settings look outdated: press \"Retrieve Current Setting\".",
        "Keyboard not found: use a data-capable USB cable (wired mode) or the 2.4 GHz receiver (2.4 GHz mode, keyboard awake). Over 2.4 GHz everything works but is slower.",
        "Close other keyboard tools while configuring; only one program should talk to the keyboard at a time.",
    ]),
    ("🔒", "Privacy", &[
        "Open Akko works offline: no account, no telemetry, no cloud.",
        "The only network request is the optional firmware check, which sends just the model ID when you click it.",
    ]),
];

fn help_page(ui: &mut egui::Ui) {
    card(ui, |ui| {
        ui.label(RichText::new("Welcome to Open Akko").strong().size(20.0));
        ui.label(RichText::new("An open, offline configurator for Akko keyboards on Linux. Click a topic to expand it.").color(pal().muted));
    });
    ui.add_space(10.0);
    for (i, (icon, title, lines)) in HELP.iter().enumerate() {
        card(ui, |ui| {
            egui::CollapsingHeader::new(RichText::new(format!("{icon}   {title}")).strong().size(16.0).color(pal().text))
                .id_salt(("help", i)).default_open(i < 3).show(ui, |ui| {
                    ui.add_space(4.0);
                    for l in lines.iter() {
                        ui.horizontal_top(|ui| {
                            ui.label(RichText::new("•").color(pal().purple).strong());
                            ui.add(egui::Label::new(RichText::new(*l).size(14.0)).wrap());
                        });
                    }
                });
        });
        ui.add_space(8.0);
    }
}

// ---------------------------------------------------------------- widgets
fn card<R>(ui: &mut egui::Ui, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new().fill(pal().card).corner_radius(16.0).inner_margin(Margin::same(18))
        .shadow(egui::Shadow { offset: [0, 2], blur: 12, spread: 0, color: Color32::from_black_alpha(14) })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // normal (non-stretched) layout inside cards, even within ui.columns()
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), f).inner
        }).inner
}

fn purple_button(t: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(t.to_string()).color(Color32::WHITE)).fill(pal().purple).corner_radius(8.0)
}

fn chip(ui: &mut egui::Ui, t: &str, on: bool) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(t).color(if on { Color32::WHITE } else { pal().text }))
        .fill(if on { pal().purple } else { pal().chip }).corner_radius(8.0).stroke(Stroke::NONE)
        .min_size(Vec2::new(54.0, 38.0)))
}

fn legend(ui: &mut egui::Ui, c: Color32, t: &str) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    ui.painter().rect(r, 3.0, pal().key, Stroke::new(2.0_f32, c), StrokeKind::Inside);
    ui.label(RichText::new(t).size(12.5).color(pal().muted));
    ui.add_space(10.0);
}

/// Returns Some(true) when "Apply" was pressed.
fn light_editor(ui: &mut egui::Ui, l: &mut Light, modes: &[(u8, &str)], id: &str) -> Option<bool> {
    ui.add_space(4.0);
    ui.label(RichText::new("Effect").color(pal().muted));
    ui.horizontal_wrapped(|ui| {
        for (m, n) in modes {
            if chip(ui, n, l.mode == *m).clicked() { l.mode = *m; }
        }
    });
    ui.add_space(6.0);
    egui::Grid::new(format!("light_{id}")).num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
        ui.label("Colour");
        ui.horizontal_wrapped(|ui| {
            let mut c = [(l.color >> 16) as u8, (l.color >> 8) as u8, l.color as u8];
            if ui.color_edit_button_srgb(&mut c).changed() { l.color = from_rgb(c); l.rainbow = false; }
            for p in [0xFF0000u32, 0x00FF00, 0x0000FF, 0xFF8000, 0xFF00FF, 0xFFFF00, 0xFFFFFF] {
                let (r, resp) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
                ui.painter().rect(r, 10.0, to_c32(p), Stroke::new(if l.color == p && !l.rainbow { 2.0 } else { 1.0_f32 }, if l.color == p && !l.rainbow { pal().purple } else { pal().key_border }), StrokeKind::Inside);
                if resp.clicked() { l.color = p; l.rainbow = false; }
            }
            ui.checkbox(&mut l.rainbow, "🌈 Rainbow");
        });
        ui.end_row();
        ui.label("Brightness"); ui.add(egui::Slider::new(&mut l.brightness, 0..=4)); ui.end_row();
        ui.label("Speed"); ui.add(egui::Slider::new(&mut l.speed, 0..=4)); ui.end_row();
        ui.label("Direction"); ui.add(egui::Slider::new(&mut l.option, 0..=3)); ui.end_row();
    });
    ui.add_space(6.0);
    Some(ui.add(purple_button("Apply").min_size(Vec2::new(120.0, 32.0))).clicked())
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        IS_DARK.store(ctx.theme() == egui::Theme::Dark, std::sync::atomic::Ordering::Relaxed);
        self.poll();
        self.load_logo(ctx);
        egui::SidePanel::left("nav").exact_width(250.0).resizable(false)
            .frame(egui::Frame::new().fill(pal().sidebar)).show(ctx, |ui| self.sidebar(ui));
        egui::TopBottomPanel::top("top").exact_height(64.0)
            .frame(egui::Frame::new().fill(pal().bg).inner_margin(Margin::symmetric(0, 14))).show(ctx, |ui| self.topbar(ui));
        egui::CentralPanel::default().frame(egui::Frame::new().fill(pal().bg).inner_margin(Margin { left: 24, right: 24, top: 4, bottom: 16 }))
            .show(ctx, |ui| {
                if self.dev.is_none() && !matches!(self.page, Page::Macro | Page::Help) {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new(format!("⌨\n\n{}", self.status)).size(18.0).color(pal().muted));
                    });
                    return;
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    // centred content column with a maximum width
                    const MAX_W: f32 = 1400.0;
                    let full = ui.available_width();
                    let w = full.min(MAX_W);
                    ui.horizontal_top(|ui| {
                        ui.add_space(((full - w) / 2.0).max(0.0));
                        ui.vertical(|ui| {
                            ui.set_width(w);
                            ui.set_max_width(w);
                            match self.page {
                                Page::Config => self.remap_page(ui, false),
                                Page::Fn => self.remap_page(ui, true),
                                Page::Macro => self.macro_page(ui),
                                Page::Light => self.light_page(ui),
                                Page::Settings => self.settings_page(ui),
                                Page::About => self.about_page(ui),
                                Page::Help => help_page(ui),
                            }
                        });
                    });
                });
            });
        // toast
        if let Some((m, err, t)) = &self.toast {
            if t.elapsed().as_secs_f32() < 3.0 {
                egui::Area::new(egui::Id::new("toast")).anchor(egui::Align2::CENTER_BOTTOM, [0.0, -24.0]).show(ctx, |ui| {
                    egui::Frame::new().fill(if *err { pal().disabled } else { pal().toast })
                        .corner_radius(10.0).inner_margin(Margin::symmetric(16, 10))
                        .show(ui, |ui| ui.label(RichText::new(m).color(Color32::WHITE)));
                });
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
            } else {
                self.toast = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::smoothed_battery;
    #[test]
    fn battery_smoothing() {
        let v: std::collections::VecDeque<u8> = [94, 54, 93].into_iter().collect();
        assert_eq!(smoothed_battery(&v), Some(95)); // outlier 54 ignored, 93 -> 95
        let v: std::collections::VecDeque<u8> = [74, 85, 93, 54, 86].into_iter().collect();
        assert_eq!(smoothed_battery(&v), Some(85));
        let v: std::collections::VecDeque<u8> = [80, 81].into_iter().collect();
        assert_eq!(smoothed_battery(&v), None); // not enough samples yet
    }
}
