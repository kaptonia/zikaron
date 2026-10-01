//! The ZIKARON design system: the settled numbers and every control. Pages take sizes, colors and motion
//! only from here and keep no copy of their own.
//!
//! Files follow the control families: `tokens` (spacing, type, corners, timings), `palette` (the two color
//! tables and the blend between them), `motion` (curves, tweens, entrances), `skin` (installing into egui),
//! then one file per family of controls.
//!
//! egui enters only through this crate and the app's `window` module. No law decision is made here: this
//! crate knows colors, shapes and rectangles, not entries, ledgers or kits.

pub mod button;
pub mod card;
pub mod drop;
pub mod fold;
pub mod fonts;
pub mod full;
pub mod grid;
pub mod icons;
pub mod input;
pub mod kv;
pub mod layer;
pub mod mark;
pub mod menu;
pub mod motion;
pub mod page;
pub mod paint;
pub mod palette;
pub mod pin;
pub mod probe;
pub mod rail;
pub mod secret;
pub mod seg;
pub mod sheet;
pub mod skin;
pub mod states;
pub mod table;
pub mod toast;
pub mod toggle;
pub mod tokens;
pub mod toolbar;
pub mod width;

/// egui re-exported, so consumers need no second path to the third-party crate.
pub use egui;
