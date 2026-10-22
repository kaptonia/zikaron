//! The settled numbers: spacing scale, type scale, corner radii and motion timings.
//!
//! Pages take sizes only from here (through the controls); nothing else defines a spacing, a font size, a
//! corner or a duration. Colors live in `palette`.

/// Spacing scale. Every gap on a page is one of these seven steps.
pub const S1: f32 = 4.0;
pub const S2: f32 = 8.0;
pub const S3: f32 = 12.0;
pub const S4: f32 = 16.0;
pub const S5: f32 = 24.0;
pub const S6: f32 = 32.0;
pub const S7: f32 = 48.0;

/// The whole scale, smallest first.
pub const SCALE: [f32; 7] = [S1, S2, S3, S4, S5, S6, S7];

/// Card padding: 24 across, 22 down.
pub const CARD_PAD_X: f32 = 24.0;
pub const CARD_PAD_Y: f32 = 22.0;
/// Between cards.
pub const CARD_GAP: f32 = S4;
/// Between a page's blocks (cards, groups with their titles, the overview): 24.
pub const BLOCK_GAP: f32 = 24.0;
/// Page side and bottom padding.
pub const PAGE_PAD: f32 = S6;
/// Sheet padding.
pub const SHEET_PAD: f32 = S5;

/// Text roles. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Type {
    /// Page title 22/30, bold.
    Page,
    /// Card title 16/24, semibold.
    Card,
    /// Large row title 15/22 (settings home, alerts): a step lighter and smaller than a card title.
    Row,
    /// Sheet title 17/24, semibold.
    Sheet,
    /// Body 15/22.
    Body,
    /// Body in the heavy face.
    Strong,
    /// Note 14/20.
    Note,
    /// Small 13/18.
    Small,
    /// Monospace 14.
    Mono,
    /// Small monospace 13 (sequence numbers and times in tables).
    MonoSmall,
    /// Monospace 12.5 under a row's title (addresses, paths).
    MonoSub,
    /// Key text 14 (keys draw it in the medium face, see `button::key_font`).
    Key,
    /// A small key inside a field 13.5, medium face.
    InnerKey,
    /// Rail item 14.5.
    Rail,
    /// Rail group title and status line 12.
    Tiny,
    /// Identity kind under the name in the rail, menu headings 11.
    Micro,
    /// Large figure on a tile 22, semibold.
    Figure,
    /// Stat figure 16, semibold.
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

    /// Font size in points, as the design sets them: the smallest are 12 (rail group titles, the status
    /// line) and 11 (the identity kind, menu headings).
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

    /// Line height in points.
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

    /// Whether the role takes the heavy face.
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

/// The rail's count figure (a numeric mark, semibold).
pub const COUNT_TEXT: f32 = 12.0;

/// Corner radii. Closed. A radius larger than half a shape's short side draws that shape as a capsule.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Radius {
    /// Cards, lists, forms, drop zones: 16.
    Card,
    /// Keys: 17 (a key 34 high is a capsule).
    Key,
    /// Inputs, passcode and word cells, and the band under a lit list row: 10.
    Field,
    /// Type tags, date blocks, notes, result boxes and file rows: 12.
    Tag,
    /// Sheets and toasts of more than one line: 22.
    Sheet,
    /// Menus, pickers, the calendar, choice rows: 14.
    Menu,
    /// The calendar's days and its head and foot keys: 15.
    Cell,
    /// Segmented track 17, its thumb 15.
    Track,
    Thumb,
    /// Pills, toasts, switches and round icon keys: fully round.
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

/// The pace of every animation: each duration in the kit (and any the window gives) is its base time times this, so
/// all of them quicken or slow together from here (0.7: every designed time is played at seven tenths). Loop periods
/// (`CYCLE` and the spinners), how long a toast stays and how long a key shows its result are not durations of a
/// change and keep their times.
pub const PACE: f32 = 0.7;

/// Motion timings in seconds: fast for hover, press and switches; mid for page changes, folds, sheets and
/// sliders; slow for toasts and the appearance blend. Loops run once per `CYCLE`, and only while a task runs.
pub const FAST: f32 = 0.12 * PACE;
pub const MID: f32 = 0.2 * PACE;
/// The identity lens slides between rows over this long (seconds).
pub const LENS: f32 = 0.22 * PACE;
pub const SLOW: f32 = 0.3 * PACE;
/// Pushing into or popping out of a detail page.
pub const PUSH: f32 = 0.24 * PACE;
pub const CYCLE: f32 = 1.2;

/// Key: one shape, 34 high, 14 text, 16 across, pressed to 0.97.
pub const KEY_H: f32 = 34.0;
pub const KEY_PAD_X: f32 = 16.0;
pub const KEY_PRESS: f32 = 0.97;
pub const KEY_GAP: f32 = S2;
/// Disabled keys and controls are drawn at this opacity.
pub const OFF: f32 = 0.45;

/// Input: 38 high, 15 text, 12 inside.
pub const INPUT_H: f32 = 38.0;
pub const INPUT_PAD_X: f32 = 12.0;
/// A recovery word's cell, shown or typed into: 36 high.
pub const WORD_H: f32 = 36.0;
/// The mark at the right edge of a field that opens a picker (the date field's calendar).
pub const INPUT_ICON: f32 = 14.0;
/// The label column of a path row and of key-value tables (a key-value table's key column widens to its
/// widest key, up to 180; a key wider still wraps).
pub const LABEL_W: f32 = 132.0;
pub const LABEL_MAX_W: f32 = 180.0;

/// Segmented control 34 high (2 of track around a 30 thumb); switch 34 × 20; pills 24 high; type tags 24
/// high and at least 52 wide; status icons 18.
pub const SEG_H: f32 = 34.0;
/// The small segmented control, on a sheet's title line.
pub const SEG_SM_H: f32 = 30.0;
pub const SWITCH_W: f32 = 34.0;
pub const SWITCH_H: f32 = 20.0;
pub const PILL_H: f32 = 24.0;
pub const TAG_H: f32 = 24.0;
pub const TAG_MIN_W: f32 = 52.0;
pub const MARK: f32 = 18.0;

/// Tables: head 40, rows 56, the sequence column 36 and the type column at least 84 (as wide as its widest
/// tag up to 148, a tag wider still elided); 12 between cells; the body column keeps 120 before the fixed
/// columns give way.
pub const TABLE_HEAD_H: f32 = 40.0;
pub const ROW_H: f32 = 56.0;
pub const SEQ_W: f32 = 36.0;
pub const TYPE_W: f32 = 84.0;
pub const TYPE_MAX_W: f32 = 148.0;
/// A tag's words and its padding (10 each side).
pub const TAG_PAD: f32 = 20.0;
/// The body (share) columns' floor: narrower than this, the fixed columns give way first.
pub const BODY_MIN_W: f32 = 120.0;
/// The type column of a pick list inside a sheet.
pub const PICK_TYPE_W: f32 = 64.0;
pub const CELL_GAP: f32 = S3;
/// Inside a list card: 4 down, 6 across; row content 14 in from the row block, so a table's first cell starts
/// 20 in from the card's edge, where a row card's title does.
pub const LIST_PAD_Y: f32 = 4.0;
pub const LIST_PAD_X: f32 = 6.0;
pub const ROW_INSET: f32 = 14.0;

/// Key-value: key column 132 (up to 180 for a wider key), rows 14 apart, columns 20 apart.
pub const KV_ROW_GAP: f32 = 14.0;
pub const KV_COL_GAP: f32 = 20.0;

/// Settings-style form rows: at least 56 high, 20 in; list rows on the settings home 72 high, 22 in.
pub const FORM_ROW_H: f32 = 56.0;
pub const FORM_PAD_X: f32 = 20.0;
pub const SET_ROW_H: f32 = 72.0;
pub const SET_PAD_X: f32 = 22.0;

/// Sheets: three widths.
pub const SHEET_W: f32 = 460.0;
pub const SHEET_WIDE: f32 = 520.0;
pub const SHEET_XWIDE: f32 = 640.0;

/// Side rail 216 wide; items 36 high. At the top, macOS keeps 46: its window buttons float over the rail (a
/// 28 band), with 18 below them. Elsewhere the system's own title bar holds the buttons, above the rail, so
/// only the 18 below them is kept, and the identity chip sits as far under the title bar as it does under the
/// buttons on macOS.
pub const RAIL_W: f32 = 216.0;
pub const RAIL_ITEM_H: f32 = 36.0;
#[cfg(target_os = "macos")]
pub const RAIL_TOP: f32 = 46.0;
#[cfg(not(target_os = "macos"))]
pub const RAIL_TOP: f32 = 18.0;
pub const RAIL_PAD: f32 = 10.0;

/// Toolbar 56 high (10 above its keys).
pub const TOOLBAR_H: f32 = 56.0;

/// Stroke width of icons on their twenty-four-cell grid.
pub const ICON_STROKE: f32 = 1.7;

/// Minimum cell width of a three-across grid: three cells across on a wide window, two on a narrow one.
pub const GRID3_MIN_W: f32 = 200.0;
/// Minimum cell width of a two-across grid that stacks on a narrow window.
pub const GRID2_MIN_W: f32 = 360.0;
/// The side column of a two-column form (300) and the least the main column keeps beside it.
pub const SIDE_W: f32 = 300.0;
pub const MAIN_MIN_W: f32 = 520.0;

/// Passcode cells 40 × 54, 8 apart (small ones 30 × 40).
pub const PIN_W: f32 = 40.0;
pub const PIN_H: f32 = 54.0;
pub const PIN_SMALL_W: f32 = 30.0;
pub const PIN_SMALL_H: f32 = 40.0;
pub const PIN_GAP: f32 = 8.0;
