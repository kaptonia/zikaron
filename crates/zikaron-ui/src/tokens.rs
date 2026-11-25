pub const S1: f32 = 4.0;
pub const S2: f32 = 8.0;
pub const S3: f32 = 12.0;
pub const S4: f32 = 16.0;
pub const S5: f32 = 24.0;
pub const S6: f32 = 32.0;
pub const S7: f32 = 48.0;

pub const SCALE: [f32; 7] = [S1, S2, S3, S4, S5, S6, S7];

pub const CARD_PAD_X: f32 = 24.0;
pub const CARD_PAD_Y: f32 = 22.0;
pub const CARD_GAP: f32 = S4;
pub const BLOCK_GAP: f32 = 24.0;
pub const PAGE_PAD: f32 = S6;
pub const SHEET_PAD: f32 = S5;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Type {
    Page,
    Card,
    Row,
    Sheet,
    Body,
    Strong,
    Note,
    Small,
    Mono,
    MonoSmall,
    MonoSub,
    Key,
    InnerKey,
    Rail,
    Tiny,
    Micro,
    Figure,
    Stat,
}

impl Type {
    pub const ALL: [Type; 18] = [
        Type::Page,
        Type::Card,
        Type::Row,
        Type::Sheet,
        Type::Body,
        Type::Strong,
        Type::Note,
        Type::Small,
        Type::Mono,
        Type::MonoSmall,
        Type::MonoSub,
        Type::Key,
        Type::InnerKey,
        Type::Rail,
        Type::Tiny,
        Type::Micro,
        Type::Figure,
        Type::Stat,
    ];

    pub fn size(self) -> f32 {
        match self {
            Type::Page | Type::Figure => 22.0,
            Type::Sheet => 17.0,
            Type::Card | Type::Stat => 16.0,
            Type::Body | Type::Strong | Type::Row => 15.0,
            Type::Rail => 14.5,
            Type::Note | Type::Mono | Type::Key => 14.0,
            Type::InnerKey => 13.5,
            Type::Small | Type::MonoSmall => 13.0,
            Type::MonoSub => 12.5,
            Type::Tiny => 12.0,
            Type::Micro => 11.0,
        }
    }

    pub fn line(self) -> f32 {
        match self {
            Type::Page | Type::Figure => 30.0,
            Type::Card | Type::Sheet | Type::Stat => 24.0,
            Type::Body | Type::Strong | Type::Row => 22.0,
            Type::Note | Type::Mono | Type::Key | Type::Rail => 20.0,
            Type::Small | Type::MonoSmall | Type::MonoSub | Type::InnerKey => 18.0,
            Type::Tiny => 16.0,
            Type::Micro => 14.0,
        }
    }

    pub fn heavy(self) -> bool {
        matches!(self, Type::Page | Type::Card | Type::Sheet | Type::Strong | Type::Figure | Type::Stat)
    }

    pub fn font(self) -> egui::FontId {
        let family = match self {
            Type::Mono | Type::MonoSmall | Type::MonoSub => egui::FontFamily::Monospace,
            Type::InnerKey => crate::fonts::medium(),
            _ if self.heavy() => crate::fonts::strong(),
            _ => egui::FontFamily::Proportional,
        };
        egui::FontId::new(self.size(), family)
    }
}

pub const COUNT_TEXT: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Radius {
    Card,
    Key,
    Field,
    Tag,
    Sheet,
    Menu,
    Cell,
    Track,
    Thumb,
    Pill,
}

impl Radius {
    pub const ALL: [Radius; 10] = [
        Radius::Card,
        Radius::Key,
        Radius::Field,
        Radius::Tag,
        Radius::Sheet,
        Radius::Menu,
        Radius::Cell,
        Radius::Track,
        Radius::Thumb,
        Radius::Pill,
    ];

    pub fn px(self) -> f32 {
        match self {
            Radius::Card => 16.0,
            Radius::Key => 17.0,
            Radius::Field => 10.0,
            Radius::Tag => 12.0,
            Radius::Sheet => 22.0,
            Radius::Menu => 14.0,
            Radius::Cell => 15.0,
            Radius::Track => 17.0,
            Radius::Thumb => 15.0,
            Radius::Pill => 255.0,
        }
    }

    pub fn egui(self) -> egui::CornerRadius {
        egui::CornerRadius::same(self.px().min(255.0) as u8)
    }
}

pub const PACE: f32 = 0.7;

pub const FAST: f32 = 0.12 * PACE;
pub const MID: f32 = 0.2 * PACE;
pub const LENS: f32 = 0.22 * PACE;
pub const SLOW: f32 = 0.3 * PACE;
pub const PUSH: f32 = 0.24 * PACE;
pub const CYCLE: f32 = 1.2;

pub const KEY_H: f32 = 34.0;
pub const KEY_PAD_X: f32 = 16.0;
pub const KEY_PRESS: f32 = 0.97;
pub const KEY_GAP: f32 = S2;
pub const OFF: f32 = 0.45;

pub const INPUT_H: f32 = 38.0;
pub const INPUT_PAD_X: f32 = 12.0;
pub const WORD_H: f32 = 36.0;
pub const INPUT_ICON: f32 = 14.0;
pub const LABEL_W: f32 = 132.0;
pub const LABEL_MAX_W: f32 = 180.0;

pub const SEG_H: f32 = 34.0;
pub const SEG_SM_H: f32 = 30.0;
pub const SWITCH_W: f32 = 34.0;
pub const SWITCH_H: f32 = 20.0;
pub const PILL_H: f32 = 24.0;
pub const TAG_H: f32 = 24.0;
pub const TAG_MIN_W: f32 = 52.0;
pub const MARK: f32 = 18.0;

pub const TABLE_HEAD_H: f32 = 40.0;
pub const ROW_H: f32 = 56.0;
pub const SEQ_W: f32 = 36.0;
pub const TYPE_W: f32 = 84.0;
pub const TYPE_MAX_W: f32 = 148.0;
pub const TAG_PAD: f32 = 20.0;
pub const BODY_MIN_W: f32 = 120.0;
pub const PICK_TYPE_W: f32 = 64.0;
pub const CELL_GAP: f32 = S3;
pub const LIST_PAD_Y: f32 = 4.0;
pub const LIST_PAD_X: f32 = 6.0;
pub const ROW_INSET: f32 = 14.0;

pub const KV_ROW_GAP: f32 = 14.0;
pub const KV_COL_GAP: f32 = 20.0;

pub const FORM_ROW_H: f32 = 56.0;
pub const FORM_PAD_X: f32 = 20.0;
pub const SET_ROW_H: f32 = 72.0;
pub const SET_PAD_X: f32 = 22.0;

pub const SHEET_NARROW: f32 = SHEET_PIN_W * 8.0 + PIN_GAP * 7.0 + SHEET_SIDE * 2.0;
pub const SHEET_SIDE: f32 = 24.0;
pub const SHEET_PIN_W: f32 = 36.0;
pub const SHEET_PIN_H: f32 = 44.0;
pub const SHEET_W: f32 = 460.0;
pub const SHEET_WIDE: f32 = 520.0;
pub const SHEET_XWIDE: f32 = 640.0;

pub const RAIL_W: f32 = 216.0;
pub const RAIL_ITEM_H: f32 = 36.0;
#[cfg(target_os = "macos")]
pub const RAIL_TOP: f32 = 46.0;
#[cfg(not(target_os = "macos"))]
pub const RAIL_TOP: f32 = 18.0;
pub const RAIL_PAD: f32 = 10.0;

pub const TOOLBAR_H: f32 = 56.0;

pub const ICON_STROKE: f32 = 1.7;

pub const GRID3_MIN_W: f32 = 200.0;
pub const GRID2_MIN_W: f32 = 360.0;
pub const SIDE_W: f32 = 300.0;
pub const MAIN_MIN_W: f32 = 520.0;

pub const PIN_W: f32 = 40.0;
pub const PIN_H: f32 = 54.0;
pub const PIN_SMALL_W: f32 = 30.0;
pub const PIN_SMALL_H: f32 = 40.0;
pub const PIN_GAP: f32 = 8.0;
