#![forbid(unsafe_code)]

pub mod carried;
pub mod consent;
pub mod input;
pub mod names;
pub mod reading;
pub mod shapes;

pub const ENTRY_TYPE: &str = "zikaron.face/1";

pub const DOMAIN: &str = ENTRY_TYPE;

pub const MARKER_PREFIX: &str = "zikaron:";

pub const ANCHOR_SELECTOR: [u8; 4] = [0xee, 0xcd, 0xf9, 0x27];

pub fn marker(root: &str) -> String {
    format!("{MARKER_PREFIX}{root}")
}
