//! Opt-in firmware update CHECK (never downloads or flashes).
//!
//! Same request as Akko Cloud v4: POST {api}/get_fw_version {"dev_id": <model id>}.
//! Only the model id is sent (identical for every unit of a model). Called only when the
//! user presses "Check for updates"; nothing in Open Akko touches the network otherwise.

use serde_json::{json, Value};
use std::time::Duration;

const API: &str = "https://api2.rongyuan.tech:3816/api/v2/get_fw_version";

#[derive(Clone, Debug)]
pub struct FwCheck {
    pub latest: String,       // e.g. "v104"
    pub latest_num: u32,      // parsed like the official app: digits read as hex
    pub update_available: bool,
}

/// Official app logic: join the digits of the version string and parse them as hex.
pub fn parse_version(s: &str) -> Option<u32> {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    u32::from_str_radix(&digits, 16).ok()
}

pub fn check(device_id: i64, current: u16) -> Result<FwCheck, String> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).user_agent("OpenAkko").build();
    let resp: Value = agent.post(API).send_json(json!({ "dev_id": device_id }))
        .map_err(|e| format!("could not reach Akko's server: {e}"))?
        .into_json().map_err(|e| format!("bad reply: {e}"))?;
    if resp["code"].as_i64() != Some(0) {
        return Err(resp["err_message"].as_str().unwrap_or("no firmware listed for this model").to_string());
    }
    // a multi-part version like "v104_rfv105" lists the main (USB) firmware first
    let full = resp["data"]["version_str"].as_str().ok_or("reply has no version")?.to_string();
    let main = full.split('_').next().unwrap_or(&full).to_string();
    let latest_num = parse_version(&main).ok_or("unreadable version")?;
    Ok(FwCheck { latest: full, latest_num, update_available: latest_num > current as u32 })
}

#[cfg(test)]
mod tests {
    use super::parse_version;
    #[test]
    fn parses_like_official_app() {
        assert_eq!(parse_version("v104"), Some(0x104));
        assert_eq!(parse_version("v1.05"), Some(0x105));
    }
}
