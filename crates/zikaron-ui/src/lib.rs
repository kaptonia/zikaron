//! The ZIKARON design system: the settled numbers and every control. Pages take sizes, colors and motion
//! only from here and keep no copy of their own.
//!
//! Files follow the control families: `tokens` (spacing, type, corners, timings), `palette` (the two color
//! tables and the blend between them), `motion` (curves, tweens, entrances, control states), `flow` (in-place
//! changes kept continuous: heights, rows, words), `skin` (installing into egui), then one file per family of
//! controls.
//!
//! egui enters only through this crate and the app's `window` module. No protocol decision is made here:
//! this crate knows colors, shapes and rectangles, not entries, ledgers or kits.

// `unsafe` lives in the platform piece (`zikaron-os`) alone: in the shipped build it does not compile here. The
// crate's own tests read past a secret's length to check it was zeroed, which only raw memory can show.
#![cfg_attr(not(test), forbid(unsafe_code))]

pub mod band;
pub mod button;
pub mod callout;
pub mod card;
pub mod cells;
pub mod compose;
pub mod covers;
pub mod datepick;
pub mod drop;
pub mod dropline;
pub mod flow;
pub mod fields;
pub mod figcard;
pub mod fold;
pub mod formparts;
pub mod fonts;
pub mod full;
pub mod grid;
pub mod hand;
pub mod headtabs;
pub mod inform;
pub mod icons;
pub mod input;
pub mod kv;
pub mod lasts;
pub mod layer;
pub mod mark;
pub mod material;
pub mod menu;
pub mod motion;
pub mod nav;
pub mod page;
pub mod paint;
pub mod pick;
pub mod palette;
pub mod pin;
pub mod probe;
pub mod rail;
pub mod scroll;
pub mod secret;
pub mod seg;
pub mod sheet;
pub mod sheetparts;
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
