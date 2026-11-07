//! Device / model / layout tables extracted from Akko Cloud v4 (see tools/extract_*.py).

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Deserialize, Clone, Debug)]
pub struct DeviceEntry {
    pub id: i64,
    pub pid: u32,
    pub name: Option<String>,
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "keyLayout")]
    pub key_layout: Option<String>,
    pub layer: Option<u32>,
    #[serde(default)]
    pub magnetism: bool,
    #[serde(default)]
    pub supported: bool,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Model {
    #[serde(rename = "defaultMatrix")]
    pub default_matrix: Vec<u8>,
    #[serde(rename = "defaultFnMatrix", default)]
    pub default_fn_matrix: Vec<u8>,
    #[serde(rename = "DAZZLE")]
    pub dazzle: Option<u8>,
    #[serde(rename = "NORMAL")]
    pub normal: Option<u8>,
    #[serde(rename = "MAXSPEED")]
    pub maxspeed: Option<u8>,
    #[serde(rename = "COMMONCOLOR", default)]
    pub common_color: HashMap<String, u32>,
    #[serde(rename = "reportRate", default)]
    pub report_rate: bool,
    #[serde(rename = "FEA_CMD_GET_REV")]
    pub get_rev: Option<u8>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct LayoutKey {
    pub code: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    #[serde(default)]
    pub round: bool,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Layout {
    pub width: f32,
    pub height: f32,
    pub keys: Vec<LayoutKey>,
}

pub struct Tables {
    pub devices: Vec<DeviceEntry>,
    pub models: HashMap<String, Model>,
    pub layouts: HashMap<String, Layout>,
}

pub fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| Tables {
        devices: serde_json::from_str(include_str!("../data/devices.json")).expect("devices.json"),
        models: serde_json::from_str(include_str!("../data/models.json")).expect("models.json"),
        layouts: serde_json::from_str(include_str!("../data/layouts.json")).expect("layouts.json"),
    })
}
