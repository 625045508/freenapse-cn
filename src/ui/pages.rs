//! Page layouts for the settings window, mirroring Synapse 4's mouse tabs.

#![allow(non_snake_case, unsafe_op_in_unsafe_fn)]

use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::HDC;

use super::draw::theme::*;
use super::draw::*;
use super::{FontKind, Id, Kind, Layer, Page, TileArt, TileItem, Ui, Widget};
use crate::accel::Mode;
use crate::config::{MacroConfig, parse_button};
use crate::device::{CLICK_CODES, DeviceSpec, PhysicalButton, TURBO_MAX, TURBO_MIN};
use crate::macros::{self, Playback};

/// Card padding, the label column of a settings row, and the row heights (DESIGN.md 3.3, 4.6).
const PAD: i32 = 16;
const LABEL_W: i32 = 400;
const ROW_H: i32 = 48;
const ROW_H_SUB: i32 = 64;
/// Control heights.
const BUTTON_H: i32 = 32;
const CHIP_H: i32 = 28;
/// Game tile size (2:3 cover) and the gap of the grid.
const TILE_W: i32 = 150;
const TILE_H: i32 = 212;
const TILE_GAP: i32 = 16;

/// Builds widgets top to bottom in logical pixels, grouping them in cards.
pub struct Form {
    scale: f32,
    dc: HDC,
    body: windows_sys::Win32::Graphics::Gdi::HFONT,
    x: i32,
    w: i32,
    y: i32,
    widgets: Vec<Widget>,
    card: Option<(usize, i32)>,
    /// The divider under the last settings row, removed when its card closes.
    last_divider: Option<usize>,
}

impl Form {
    pub fn new(scale: f32, dc: HDC, fonts: &Fonts, x: i32, w: i32) -> Form {
        Form {
            scale,
            dc,
            body: fonts.body,
            x: (x as f32 / scale) as i32,
            w: (w as f32 / scale) as i32,
            y: 24,
            widgets: Vec::new(),
            card: None,
            last_divider: None,
        }
    }

    /// Right edge of the content inside a card.
    fn right(&self) -> i32 {
        self.x + self.w - PAD
    }

    /// Hairline under a settings row (dropped again if it is the card's last row).
    fn divider(&mut self, y: i32) {
        let i = self.push(Id::None, self.x + PAD, y, self.w - 2 * PAD, 1, Kind::Line);
        self.last_divider = Some(i);
    }

    fn s(&self, v: i32) -> i32 {
        (v as f32 * self.scale).round() as i32
    }

    fn measure(&self, font: windows_sys::Win32::Graphics::Gdi::HFONT, t: &str) -> i32 {
        (unsafe { text_width(self.dc, font, t) } as f32 / self.scale).ceil() as i32
    }

    fn push(&mut self, id: Id, x: i32, y: i32, w: i32, h: i32, kind: Kind) -> usize {
        self.widgets.push(Widget { id, rect: rect(self.s(x), self.s(y), self.s(w), self.s(h)), kind, fixed: false });
        self.widgets.len() - 1
    }

    fn cx(&self) -> i32 {
        self.x + PAD + LABEL_W
    }

    fn cw(&self) -> i32 {
        self.w - 2 * PAD - LABEL_W
    }

    /// Start a new card with a section title; closes the previous one.
    pub fn section(&mut self, title: &str) {
        self.section_icon(title, None);
    }

    /// Card with a title (DESIGN.md 4.5): Body strong, sentence case, no icon and no rule.
    /// The icon argument is kept for the call sites but not drawn.
    pub fn section_icon(&mut self, title: &str, _icon: Option<Icon>) {
        self.close_card();
        self.card = Some((self.widgets.len(), self.y));
        self.widgets.push(Widget { id: Id::None, rect: rect(0, 0, 0, 0), kind: Kind::Card, fixed: false });
        self.y += 12;
        self.push(
            Id::None,
            self.x + PAD,
            self.y,
            self.w - 2 * PAD,
            24,
            Kind::Text { text: title.to_string(), font: FontKind::Bold, color: TEXT, flags: DT_ROW },
        );
        self.y += 28;
    }

    /// A grid of game tiles, as many per row as fit.
    pub fn tiles(&mut self, items: Vec<TileItem>) {
        if items.is_empty() {
            return;
        }
        let cols = ((self.w - 2 * PAD + TILE_GAP) / (TILE_W + TILE_GAP)).max(1) as usize;
        let n = items.len();
        self.y += 4;
        for (k, it) in items.into_iter().enumerate() {
            let col = (k % cols) as i32;
            let x = self.x + PAD + col * (TILE_W + TILE_GAP);
            self.push(it.id, x, self.y, TILE_W, TILE_H, Kind::Tile { title: it.title, sub: it.sub, badge: it.badge, art: it.art, selected: it.selected });
            if k % cols == cols - 1 || k + 1 == n {
                self.y += TILE_H + TILE_GAP;
            }
        }
    }

    fn close_card(&mut self) {
        if let Some((i, y0)) = self.card.take() {
            if let Some(d) = self.last_divider.take() {
                self.widgets[d].rect = rect(0, 0, 0, 0);
            }
            self.y += 8;
            self.widgets[i].rect = rect(self.s(self.x), self.s(y0), self.s(self.w), self.s(self.y - y0));
            self.y += 12;
        }
    }

    pub fn finish(mut self) -> (Vec<Widget>, i32) {
        self.close_card();
        let h = self.s(self.y);
        (self.widgets, h)
    }

    /// Settings row (DESIGN.md 4.6): title in Body strong, description in Caption,
    /// a divider underneath; the caller puts the control at the right. Returns the row height.
    fn label(&mut self, title: &str, sub: &str) -> i32 {
        let h = if sub.is_empty() {
            ROW_H
        } else {
            // The description wraps inside the label column; the row grows with it.
            let lines = (self.measure(self.body, sub) * 12 / 14 / (LABEL_W - 16) + 1).clamp(1, 3);
            ROW_H_SUB + 16 * (lines - 1)
        };
        if sub.is_empty() {
            self.push(
                Id::None,
                self.x + PAD,
                self.y,
                LABEL_W - 10,
                h,
                Kind::Text { text: title.to_string(), font: FontKind::Bold, color: TEXT, flags: DT_ROW },
            );
        } else {
            self.push(
                Id::None,
                self.x + PAD,
                self.y + 12,
                LABEL_W - 16,
                20,
                Kind::Text { text: title.to_string(), font: FontKind::Bold, color: TEXT, flags: DT_ROW },
            );
            self.push(
                Id::None,
                self.x + PAD,
                self.y + 32,
                LABEL_W - 16,
                h - 40,
                Kind::Text { text: sub.to_string(), font: FontKind::Small, color: MUTED, flags: DT_LEFT_ | DT_WORDBREAK_ | DT_NOPREFIX_ | DT_END_ELLIPSIS_ },
            );
        }
        self.divider(self.y + h - 1);
        h
    }

    pub fn note(&mut self, text: &str) {
        // Descriptions wrap at about 60 characters (DESIGN.md 3.2).
        let w = (self.w - 2 * PAD).min(560);
        let lines = (self.measure(self.body, text) / w.max(1) + 1).clamp(1, 4);
        let h = 16 * lines + 12;
        self.push(
            Id::None,
            self.x + PAD,
            self.y,
            w,
            h,
            Kind::Text { text: text.to_string(), font: FontKind::Small, color: MUTED, flags: DT_LEFT_ | DT_WORDBREAK_ | DT_NOPREFIX_ },
        );
        self.y += h;
    }

    pub fn info(&mut self, title: &str, value: &str) {
        self.push(Id::None, self.x + PAD, self.y, LABEL_W - 10, 36, Kind::Text { text: title.to_string(), font: FontKind::Body, color: MUTED, flags: DT_ROW });
        self.push(Id::None, self.cx(), self.y, self.cw(), 36, Kind::Text { text: value.to_string(), font: FontKind::Body, color: TEXT, flags: DT_ROW });
        self.divider(self.y + 35);
        self.y += 36;
    }

    /// An info row whose value starts with a clickable address; `rest` follows it as plain text.
    pub fn link(&mut self, title: &str, shown: &str, url: &'static str, rest: &str) {
        self.push(Id::None, self.x + PAD, self.y, LABEL_W - 10, 36, Kind::Text { text: title.to_string(), font: FontKind::Body, color: MUTED, flags: DT_ROW });
        let w = (self.measure(self.body, shown) + 2).min(self.cw());
        self.push(Id::Link(url), self.cx(), self.y, w, 36, Kind::Link { text: shown.to_string() });
        if !rest.is_empty() {
            let text = format!("  \u{b7}  {rest}");
            self.push(Id::None, self.cx() + w, self.y, self.cw() - w, 36, Kind::Text { text, font: FontKind::Body, color: TEXT, flags: DT_ROW });
        }
        self.divider(self.y + 35);
        self.y += 36;
    }

    pub fn toggle(&mut self, title: &str, sub: &str, on: bool, id: Id) {
        let h = self.label(title, sub);
        self.push(id, self.right() - 40, self.y + (h - 20) / 2, 40, 20, Kind::Toggle { on });
        self.y += h;
    }

    /// Chip spans for `options`, relative to the group's left edge; returns the spans and the total width.
    fn chip_spans(&self, options: &[String]) -> (Vec<(i32, i32)>, i32) {
        let mut xs = Vec::new();
        let mut x = 0;
        for o in options {
            let w = self.measure(self.body, o) + 24;
            xs.push((self.s(x), self.s(x + w)));
            x += w + 8;
        }
        (xs, (x - 8).max(0))
    }

    /// Chips right-aligned in a settings row.
    pub fn chips(&mut self, title: &str, sub: &str, options: &[String], selected: Option<usize>, id: Id) {
        let h = self.label(title, sub);
        let (xs, total) = self.chip_spans(options);
        let w = total.min(self.cw());
        self.push(id, self.right() - w, self.y + (h - CHIP_H) / 2, w, CHIP_H, Kind::Chips { options: options.to_vec(), selected, xs });
        self.y += h;
    }

    /// Chips spanning the whole card width, with the title on its own line (for long option lists).
    pub fn chips_wide(&mut self, title: &str, options: &[String], selected: Option<usize>, id: Id) {
        if !title.is_empty() {
            self.push(
                Id::None,
                self.x + PAD,
                self.y,
                self.w - 2 * PAD,
                24,
                Kind::Text { text: title.to_string(), font: FontKind::Bold, color: TEXT, flags: DT_ROW },
            );
            self.y += 28;
        }
        let (xs, _) = self.chip_spans(options);
        self.push(id, self.x + PAD, self.y, self.w - 2 * PAD, CHIP_H, Kind::Chips { options: options.to_vec(), selected, xs });
        self.y += CHIP_H + 12;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn slider(&mut self, title: &str, sub: &str, min: i32, max: i32, step: i32, value: i32, fmt: fn(i32) -> String, id: Id) {
        self.slider_opt(title, sub, min, max, step, value, fmt, id, true);
    }

    /// Slider that can be greyed out (shown, but not adjustable).
    #[allow(clippy::too_many_arguments)]
    pub fn slider_opt(&mut self, title: &str, sub: &str, min: i32, max: i32, step: i32, value: i32, fmt: fn(i32) -> String, id: Id, enabled: bool) {
        let h = self.label(title, sub);
        self.push(
            id,
            self.cx(),
            self.y,
            self.cw(),
            h,
            Kind::Slider { min, max, step, value: value.clamp(min, max), fmt, ticks: Vec::new(), show_value: true, enabled },
        );
        self.y += h;
    }

    /// Warning line in amber with its icon, e.g. why a control above is greyed out.
    pub fn warning(&mut self, text: &str) {
        self.push(Id::None, self.x + PAD, self.y + 4, 16, 16, Kind::Icon { icon: Icon::Warning, color: WARN });
        self.push(
            Id::None,
            self.x + PAD + 24,
            self.y,
            self.w - 2 * PAD - 24,
            24,
            Kind::Text { text: text.to_string(), font: FontKind::Small, color: WARN, flags: DT_ROW },
        );
        self.y += 28;
    }

    /// Discrete slider with one tick label per step.
    pub fn ticks(&mut self, title: &str, sub: &str, labels: &[String], selected: usize, id: Id) {
        let h = self.label(title, sub).max(64);
        let n = labels.len().max(1) as i32;
        self.push(
            id,
            self.cx(),
            self.y,
            self.cw(),
            h,
            Kind::Slider {
                min: 0,
                max: n - 1,
                step: 1,
                value: (selected as i32).min(n - 1),
                fmt: |v| v.to_string(),
                ticks: labels.to_vec(),
                show_value: false,
                enabled: true,
            },
        );
        self.y += h;
    }

    /// A DPI stage row: radio (active stage), slider, and a box to type the value.
    /// With `y`, a second row for the vertical DPI follows (independent X/Y).
    pub fn stage(&mut self, i: usize, dpi: u16, y: Option<u16>, active: bool, selectable: bool, dpi_max: u16) {
        let h = ROW_H;
        let title = if selectable { format!("档位 {}", i + 1) } else { "灵敏度".to_string() };
        let title = if y.is_some() { format!("{title}  ·  X 轴") } else { title };
        let title = if active && selectable { format!("{title}  ·  当前") } else { title };
        if selectable {
            self.push(Id::StageActive(i), self.x + PAD, self.y, LABEL_W - 10, h, Kind::Radio { on: active, text: title });
        } else {
            self.push(Id::None, self.x + PAD, self.y, LABEL_W - 10, h, Kind::Text { text: title, font: FontKind::Bold, color: TEXT, flags: DT_ROW });
        }
        self.dpi_controls(Id::StageValue(i), dpi, h, dpi_max);
        self.y += h;
        if let Some(yd) = y {
            self.push(
                Id::None,
                self.x + PAD + 28,
                self.y,
                LABEL_W - 38,
                h,
                Kind::Text { text: "Y".to_string(), font: FontKind::Body, color: MUTED, flags: DT_ROW },
            );
            self.dpi_controls(Id::StageY(i), yd, h, dpi_max);
            self.y += h;
        }
        self.divider(self.y - 1);
    }

    fn dpi_controls(&mut self, id: Id, dpi: u16, h: i32, dpi_max: u16) {
        let edit_w = 88;
        self.push(
            id,
            self.cx(),
            self.y,
            self.cw() - edit_w - 16,
            h,
            Kind::Slider {
                min: 100,
                max: dpi_max as i32,
                step: 50,
                value: dpi as i32,
                fmt: |v| v.to_string(),
                ticks: Vec::new(),
                show_value: false,
                enabled: true,
            },
        );
        self.push(id, self.right() - edit_w, self.y + (h - BUTTON_H) / 2, edit_w, BUTTON_H, Kind::Edit { text: dpi.to_string(), numeric: true });
    }

    /// Acceleration curve plot (Dynamic Sensitivity); editable curves take an id.
    pub fn curve(&mut self, points: &[(u8, f32)], editable: bool, selected: Option<usize>) {
        let h = 230;
        let samples = crate::accel::curve(points).to_vec();
        let id = if editable { Id::Curve } else { Id::None };
        self.push(id, self.x + PAD, self.y, self.w - 2 * PAD, h, Kind::Curve { points: points.to_vec(), samples, editable, selected });
        self.y += h + 8;
    }

    pub fn edit(&mut self, title: &str, sub: &str, text: &str, numeric: bool, id: Id) {
        let h = self.label(title, sub);
        let w = self.cw().min(if numeric { 88 } else { 320 });
        self.push(id, self.right() - w, self.y + (h - BUTTON_H) / 2, w, BUTTON_H, Kind::Edit { text: text.to_string(), numeric });
        self.y += h;
    }

    /// A search box with its icon, full width of the card.
    pub fn search(&mut self, text: &str, id: Id) {
        self.push(Id::None, self.x + PAD + 4, self.y + (BUTTON_H - 16) / 2, 16, 16, Kind::Icon { icon: Icon::Search, color: MUTED });
        self.push(id, self.x + PAD + 28, self.y, (self.w - 2 * PAD - 28).min(320), BUTTON_H, Kind::Edit { text: text.to_string(), numeric: false });
        self.y += BUTTON_H + 12;
    }

    pub fn buttons(&mut self, items: &[(&str, Id, bool)]) {
        let mut x = self.x + PAD;
        self.y += 4;
        for (t, id, primary) in items {
            let icon = button_icon(*id, t);
            let w = self.measure(self.body, t) + 24 + if icon.is_some() { 24 } else { 0 };
            self.push(*id, x, self.y, w, BUTTON_H, Kind::Button { text: t.to_string(), primary: *primary, enabled: *id != Id::None, icon });
            x += w + 8;
        }
        self.y += BUTTON_H + 8;
    }

    pub fn battery(&mut self) {
        self.push(Id::None, self.x + PAD, self.y, self.w - 2 * PAD, 72, Kind::Battery);
        self.divider(self.y + 79);
        self.y += 80;
    }

    pub fn gap(&mut self, h: i32) {
        self.y += h;
    }

    /// One feature line on the Roadmap page: status pill, name, and a muted note.
    pub fn status(&mut self, st: Status, title: &str, note: &str) {
        let h = 36;
        let badge_w = 72;
        self.push(Id::None, self.x + PAD, self.y + 4, 16, 16 + 12, Kind::Icon { icon: st.icon(), color: st.color() });
        self.push(Id::None, self.x + PAD + 24, self.y + (h - 22) / 2, badge_w, 22, Kind::Badge { text: st.label().to_string(), color: st.color() });
        let title_x = self.x + PAD + 24 + badge_w + 12;
        let title_w = LABEL_W - badge_w - 36;
        self.push(Id::None, title_x, self.y, title_w, h, Kind::Text { text: title.to_string(), font: FontKind::Bold, color: TEXT, flags: DT_ROW });
        let note_x = title_x + title_w + 12;
        self.push(
            Id::None,
            note_x,
            self.y,
            (self.x + self.w - PAD - note_x).max(40),
            h,
            Kind::Text { text: note.to_string(), font: FontKind::Small, color: MUTED, flags: DT_ROW },
        );
        self.divider(self.y + h - 1);
        self.y += h;
    }

    /// Bulleted roadmap item.
    pub fn bullet(&mut self, color: u32, text: &str) {
        let w = self.w - 2 * PAD - 24;
        let lines = (self.measure(self.body, text) / w.max(1) + 1).clamp(1, 4);
        let h = 20 * lines + 8;
        self.push(Id::None, self.x + PAD + 4, self.y + 4, 12, 20, Kind::Bullet { color });
        self.push(
            Id::None,
            self.x + PAD + 24,
            self.y + 4,
            w,
            h - 4,
            Kind::Text { text: text.to_string(), font: FontKind::Body, color: TEXT, flags: DT_LEFT_ | DT_WORDBREAK_ | DT_NOPREFIX_ },
        );
        self.y += h;
    }
}

/// Icon in front of a button's label, by what the button does.
fn button_icon(id: Id, label: &str) -> Option<Icon> {
    Some(match id {
        Id::GameScan | Id::Refresh => Icon::Refresh,
        Id::GameBrowse | Id::OpenConfig => Icon::Folder,
        Id::GameRemove | Id::MacroDelete => Icon::Trash,
        Id::GameButtons => Icon::Mouse,
        Id::MacroNew | Id::MacroRecord => Icon::Record,
        Id::MacroStop => Icon::Stop,
        Id::Reimport => Icon::Sync,
        Id::Reapply => Icon::Check,
        Id::PairStart => Icon::Link,
        Id::PairCancel | Id::Unpair => Icon::Stop,
        Id::CurveAdd => Icon::Add,
        Id::CurveRemove => Icon::Trash,
        Id::None if label.starts_with("正在扫描") => Icon::Refresh,
        _ => return None,
    })
}

/// Implementation state of a Synapse feature, for the Roadmap page.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Done,
    Partial,
    Todo,
    Never,
    /// Devices page: writes confirmed by an owner.
    Verified,
    /// Devices page: partly tested by an owner, writes need allow_unverified.
    Testing,
    /// Devices page: known from the device table only.
    ReadOnly,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Done => "已完成",
            Status::Partial => "部分",
            Status::Todo => "计划中",
            Status::Never => "否",
            Status::Verified => "已验证",
            Status::Testing => "测试中",
            Status::ReadOnly => "只读",
        }
    }

    /// Colour is never the only signal (DESIGN.md 6): each state has an icon too.
    fn icon(self) -> Icon {
        match self {
            Status::Done => Icon::Check,
            Status::Partial => Icon::Warning,
            Status::Todo => Icon::Flag,
            Status::Never => Icon::Stop,
            Status::Verified => Icon::Check,
            Status::Testing => Icon::Warning,
            Status::ReadOnly => Icon::Info,
        }
    }

    fn color(self) -> u32 {
        match self {
            Status::Done => ACCENT,
            Status::Partial => WARN,
            Status::Todo => BLUE,
            Status::Never => MUTED,
            Status::Verified => ACCENT,
            Status::Testing => WARN,
            Status::ReadOnly => MUTED,
        }
    }
}

// ----------------------------------------------------------------------
// Pages
// ----------------------------------------------------------------------

pub fn build(ui: &Ui, f: &mut Form) {
    if let Some(e) = ui.config_error() {
        f.section_icon("配置文件有问题", Some(Icon::Warning));
        f.note(&format!("{e}。现在显示的是默认值；请修好文件，或在设置页点“从鼠标重新导入”。"));
    }
    match ui.page {
        Page::Performance => performance(ui, f),
        Page::Calibration => calibration(ui, f),
        Page::Power => power(ui, f),
        Page::Games => games(ui, f),
        Page::Customize => customize(ui, f),
        Page::Devices => devices(ui, f),
        Page::Roadmap => roadmap(f),
        Page::App => app(ui, f),
    }
}

fn hz_labels(rates: &[u16]) -> Vec<String> {
    rates.iter().map(|r| format!("{r}")).collect()
}

fn performance(ui: &Ui, f: &mut Form) {
    let cfg = &ui.cfg;
    f.section_icon("灵敏度", Some(Icon::Speed));
    let staged = cfg.dpi.stages.len() > 1;
    f.toggle("启用 DPI 档位", "用鼠标的 DPI 按键在它们之间切换", staged, Id::StagesEnabled);
    let xy = !cfg.dpi.y_stages.is_empty();
    f.toggle("X、Y 轴独立", "横向和纵向灵敏度分开设置", xy, Id::XySplit);
    let y_of = |i: usize| if xy { cfg.dpi.y_stages.get(i).copied() } else { None };
    if staged {
        let counts: Vec<String> = (1..=5).map(|n| n.to_string()).collect();
        f.chips("档位数量", "", &counts, Some(cfg.dpi.stages.len() - 1), Id::StageCount);
        f.gap(4);
        for (i, d) in cfg.dpi.stages.iter().enumerate() {
            f.stage(i, *d, y_of(i), cfg.dpi.active as usize == i + 1, true, ui.spec.dpi_max);
        }
        f.note(&format!("选择现在使用的档位。范围 100 到 {} DPI，步进 50；也可以在框里直接输入具体数值。", ui.spec.dpi_max));
    } else {
        f.stage(0, cfg.dpi.stages[0], y_of(0), true, false, ui.spec.dpi_max);
        f.note(&format!("范围 100 到 {} DPI，步进 50；也可以在框里直接输入具体数值。", ui.spec.dpi_max));
    }

    if ui.spec.supports_acceleration {
        f.section_icon("动态灵敏度", Some(Icon::Sliders));
        let a = &cfg.acceleration;
        f.toggle("启用动态灵敏度", "鼠标按你移动的快慢自动调整灵敏度（保存在鼠标里）", a.enabled, Id::AccelOn);
        if a.enabled {
            let mode = a.mode().unwrap_or(Mode::Classic);
            let modes = [Mode::Classic, Mode::Natural, Mode::Jump, Mode::Custom].map(|m| m.label().to_string());
            f.chips("曲线", "", &modes, Some(mode as usize), Id::AccelMode);
            f.gap(4);
            if mode == Mode::Custom {
                f.curve(&a.curve, true, ui.curve_sel);
                let can_remove = a.curve.len() > 2 && ui.curve_sel.is_some_and(|i| i > 0);
                let can_add = a.curve.len() < crate::accel::MAX_CONTROL_POINTS;
                f.buttons(&[
                    ("添加点", if can_add { Id::CurveAdd } else { Id::None }, false),
                    ("删除点", if can_remove { Id::CurveRemove } else { Id::None }, false),
                    ("从经典开始", Id::CurveTemplate(0), false),
                    ("自然", Id::CurveTemplate(1), false),
                    ("跳跃", Id::CurveTemplate(2), false),
                ]);
                f.note("拖动这些点：横轴是移动速度，纵轴是灵敏度倍数（0.1 到 1.5）。最后一个点之后曲线保持水平。点中一个点后按“删除点”可以删掉它。");
            } else {
                f.curve(&mode.template(), false, None);
                f.note("经典、自然、跳跃这三种内置在鼠标固件里，图上是雷云对它们各自的画法。选“自定义”可以自己拉一条曲线。");
            }
        }
    }
    f.section_icon("回报率", Some(Icon::Bolt));
    let rates: &[u16] = ui.spec.polling_rates;
    let sel = rates.iter().position(|r| *r == cfg.polling.rate).unwrap_or(0);
    let sub = if rates.iter().any(|r| *r > 1000) {
        "每秒上报次数。4000 和 8000 需要 HyperPolling 接收器，也更费电"
    } else {
        "每秒上报次数"
    };
    f.ticks("回报率（Hz）", sub, &hz_labels(rates), sel, Id::Polling);

    f.note("游戏运行时可以切换到自己的回报率、DPI 档位和按键：见“游戏”页。");

    if !ui.spec.lighting.is_empty() {
        lighting(ui, f);
    }

    if ui.spec.supports_rotation {
        f.section_icon("旋转", Some(Icon::Sync));
        let on = cfg.sensor.rotation != 0;
        f.toggle("启用旋转", "通过旋转传感器坐标轴来补偿握持的倾斜", on, Id::RotationOn);
        if on {
            f.slider("角度", "", -30, 30, 1, cfg.sensor.rotation as i32, |v| format!("{v}°"), Id::Rotation);
        }
    }
}

/// Colour chips of the Lighting section: (label, rgb).
pub const LIGHT_COLORS: [(&str, [u8; 3]); 8] = [
    ("红色", [0xff, 0x00, 0x00]),
    ("橙色", [0xff, 0x60, 0x00]),
    ("黄色", [0xff, 0xd0, 0x00]),
    ("绿色", [0x00, 0xff, 0x40]),
    ("青色", [0x00, 0xd0, 0xff]),
    ("蓝色", [0x00, 0x30, 0xff]),
    ("紫色", [0xa0, 0x00, 0xff]),
    ("白色", [0xff, 0xff, 0xff]),
];

fn lighting(ui: &Ui, f: &mut Form) {
    use crate::lighting::Effect;
    let cfg = &ui.cfg;
    f.section_icon("灯光", Some(Icon::Bolt));
    let kinds: Vec<String> = ["关闭", "常亮", "呼吸", "光谱循环"].iter().map(|s| s.to_string()).collect();
    for (i, (zone, _)) in ui.spec.lighting.iter().enumerate() {
        let title = match *zone {
            "logo" => "标志",
            "wheel" => "滚轮",
            z => z,
        };
        let e = cfg.lighting.zone(zone).and_then(|s| Effect::parse(s).ok());
        let sel = e.map(|e| match e {
            Effect::Off => 0,
            Effect::Static(_) => 1,
            Effect::Breathing(_) | Effect::BreathingRandom => 2,
            Effect::Spectrum => 3,
        });
        let sub = if e.is_none() { "未设置：鼠标保持原有灯效" } else { "" };
        f.chips(title, sub, &kinds, sel, Id::LightEffect(i));
        if let Some(e @ (Effect::Static(_) | Effect::Breathing(_) | Effect::BreathingRandom)) = e {
            let mut colors: Vec<String> = LIGHT_COLORS.iter().map(|(l, _)| l.to_string()).collect();
            if !matches!(e, Effect::Static(_)) {
                colors.push("随机色".into());
            }
            let csel = match e {
                Effect::BreathingRandom => Some(LIGHT_COLORS.len()),
                _ => e.color().and_then(|c| LIGHT_COLORS.iter().position(|(_, rgb)| *rgb == c)),
            };
            f.chips_wide("", &colors, csel, Id::LightColor(i));
        }
        f.gap(4);
    }
    f.slider("亮度", "所有区域", 0, 100, 5, cfg.lighting.brightness.unwrap_or(100) as i32, |v| format!("{v} %"), Id::Brightness);
    f.note("保存在鼠标里，换台电脑灯效也一样。其它颜色：在配置文件的 [lighting] 里写 \"static:#rrggbb\"。");
}

/// "1600 DPI  ·  2000 Hz" for a profile's tile.
fn tile_summary(cfg: &crate::config::Config, g: &crate::config::GameConfig) -> String {
    let stage = match g.dpi_stage.and_then(|s| cfg.dpi.stages.get(s as usize - 1)) {
        Some(d) => format!("{d} DPI"),
        None => "保持原 DPI".to_string(),
    };
    let polling = match g.polling {
        Some(p) => format!("{p} Hz"),
        None => format!("{} Hz", cfg.polling.game_rate),
    };
    format!("{stage}  ·  {polling}")
}

/// Launcher name shown on a tile while hovering it.
fn badge_of(source: Option<&str>) -> &'static str {
    match source {
        Some("Steam") => "Steam",
        Some("Epic 游戏商城") => "Epic 游戏商城",
        Some("GOG") => "GOG",
        Some("Xbox") => "Xbox",
        Some("EA 应用") => "EA 应用",
        Some("Battle.net") => "Battle.net",
        Some("Riot") => "Riot",
        Some("育碧 Connect") => "育碧 Connect",
        Some("Rockstar") => "Rockstar",
        Some(_) => "启动器",
        None => "程序",
    }
}

fn games(ui: &Ui, f: &mut Form) {
    let cfg = &ui.cfg;
    let rates: &[u16] = ui.spec.polling_rates;

    f.section_icon("游戏配置", Some(Icon::Game));
    if cfg.games.is_empty() {
        f.note("还没有游戏配置。点下面已安装的游戏，或添加一个程序，就能给它单独的 DPI 档位、回报率和按键。游戏在前台时托盘程序会把鼠标切换到这套设置。");
    } else {
        f.note("点一套配置就能修改。游戏切到前台约两秒后托盘程序会写入它，离开游戏时恢复你的常规设置。");
    }
    let mut tiles: Vec<TileItem> = cfg
        .games
        .iter()
        .enumerate()
        .map(|(i, g)| TileItem {
            id: Id::GameRow(i),
            title: g.label(),
            sub: tile_summary(cfg, g),
            badge: badge_of(ui.game_source(g)),
            art: ui.profile_art(g),
            selected: ui.game_open && ui.sel_game == i,
        })
        .collect();
    // The last tile adds any program (games outside a launcher).
    tiles.push(TileItem { id: Id::GameBrowse, title: "添加程序".to_string(), sub: "任意 .exe".to_string(), badge: "", art: TileArt::Add, selected: false });
    f.tiles(tiles);

    if let Some(g) = cfg.games.get(ui.sel_game).filter(|_| ui.game_open) {
        f.section_icon(&format!("配置：{}", g.label()), Some(Icon::Sliders));
        game_settings(ui, f, g, rates);
    }

    f.section_icon("已安装的游戏", Some(Icon::Folder));
    let free = ui.games_without_profile();
    let sources = ui.library_sources();
    if ui.scanning && ui.library.is_empty() {
        f.note("正在查找已安装的游戏…");
    } else if ui.library.is_empty() {
        f.note("在 Steam、Epic、GOG、EA、Battle.net、Riot、育碧 Connect、Rockstar 和 Xbox 应用里都没有找到游戏。");
    } else {
        f.note(&format!("找到 {} 个游戏。点一个即可为它创建配置。", ui.library.len()));
    }
    // Filter chips per launcher (DESIGN.md 4.13), with counts, then the search box.
    if !sources.is_empty() {
        let mut chips = vec![format!("全部 {}", ui.library.len())];
        chips.extend(sources.iter().map(|s| format!("{s} {}", ui.library.iter().filter(|l| l.source == *s).count())));
        let sel = ui.lib_source.and_then(|s| sources.iter().position(|x| *x == s)).map(|i| i + 1).unwrap_or(0);
        f.chips_wide("", &chips, Some(sel), Id::GameSource);
    }
    if free.len() > 8 {
        f.search(&ui.game_filter, Id::GameFilter);
    }
    let filter = ui.game_filter.trim().to_lowercase();
    let shown: Vec<TileItem> = free
        .into_iter()
        .filter(|j| filter.is_empty() || ui.library[*j].name.to_lowercase().contains(&filter))
        .filter(|j| ui.lib_source.is_none_or(|s| ui.library[*j].source == s))
        .map(|j| {
            let l = &ui.library[j];
            TileItem { id: Id::GameLib(j), title: l.name.clone(), sub: String::new(), badge: badge_of(Some(l.source)), art: TileArt::Lib(j), selected: false }
        })
        .collect();
    let none = shown.is_empty();
    f.tiles(shown);
    if none && (!filter.is_empty() || ui.lib_source.is_some()) {
        f.note("没有匹配的游戏。");
    }
    f.buttons(&[(if ui.scanning { "正在扫描…" } else { "重新扫描" }, if ui.scanning { Id::None } else { Id::GameScan }, false)]);

    f.section_icon("游戏内回报率", Some(Icon::Bolt));
    let gsel = rates.iter().position(|r| *r == cfg.polling.game_rate).unwrap_or(0);
    f.ticks("游戏时的回报率（Hz）", "供设为“游戏内回报率”的游戏配置使用", &hz_labels(rates), gsel, Id::GameRate);
    if !cfg.polling.games.is_empty() {
        f.edit("更多程序", "可执行文件名，用逗号分隔：只对它们使用游戏内回报率", &cfg.polling.games.join(", "), false, Id::GamesEdit);
    }
    f.note("相当于雷云的游戏内回报率。回报率越高越费电，所以只在游戏位于前台时才提高。");
}

/// The settings of the open game profile.
fn game_settings(ui: &Ui, f: &mut Form, g: &crate::config::GameConfig, rates: &[u16]) {
    let cfg = &ui.cfg;
    let mut stages = vec!["保持".to_string()];
    stages.extend((1..=cfg.dpi.stages.len()).map(|i| format!("档位 {i}（{}）", cfg.dpi.stages[i - 1])));
    f.chips_wide("游戏时使用的 DPI 档位", &stages, Some(g.dpi_stage.map(|s| s as usize).filter(|s| *s <= cfg.dpi.stages.len()).unwrap_or(0)), Id::GameStage);
    if cfg.dpi.stages.len() < 2 {
        f.note("现在只设了一个 DPI 档位。要在“性能”页启用 DPI 档位，游戏才能有自己的灵敏度。");
    }
    let mut polls = vec![format!("游戏内回报率（{}）", cfg.polling.game_rate)];
    polls.extend(rates.iter().map(|r| format!("{r} Hz")));
    let psel = g.polling.and_then(|p| rates.iter().position(|r| *r == p)).map(|i| i + 1).unwrap_or(0);
    f.chips_wide("游戏时使用的回报率", &polls, Some(psel), Id::GamePolling);
    f.edit("名称", "显示在这里和托盘提示里。按 Enter 保存", &g.label(), false, Id::GameName);
    let exe_sub =
        if g.dir.is_empty() { "可执行文件名，例如 cs2.exe。按 Enter 保存" } else { "可选：游戏文件夹里的程序本来就算" };
    f.edit("程序", exe_sub, &g.exe, false, Id::GameExe);
    if !g.dir.is_empty() {
        f.info("文件夹", &g.dir);
    }
    let n = g.buttons.len();
    f.info(
        "按键",
        &if n == 0 {
            "与普通层相同".to_string()
        } else {
            format!("单独设置了 {n} 个按键{}，其余与普通层相同", if n == 1 { "" } else { "s" })
        },
    );
    f.buttons(&[("编辑按键布局", Id::GameButtons, true), ("删除配置", Id::GameRemove, false)]);
}

fn calibration(ui: &Ui, f: &mut Form) {
    use crate::sensor::{ASYM_LAND_MIN, ASYM_LIFT_MAX, ASYM_LIFT_MIN};
    let s = &ui.cfg.sensor;
    f.section_icon("智能追踪", Some(Icon::Track));
    let modes = ["智能追踪", "保持当前"].map(String::from);
    let sel = if s.tracking == "smart" { Some(0) } else { Some(1) };
    f.chips("追踪", "雷云给这只鼠标提供的两种抬升模式", &modes, sel, Id::TrackMode);
    if s.tracking != "smart" {
        f.note("鼠标保持它自己存储的抬升设置。选“智能追踪”就能在这里设置。");
        return;
    }
    f.toggle("非对称抬升", "抬起和落下分开设，而不是一个距离", s.asymmetric, Id::Asym);
    if s.asymmetric {
        f.slider(
            "抬起",
            "到这个档位就停止追踪（从低到高）",
            ASYM_LIFT_MIN as i32,
            ASYM_LIFT_MAX as i32,
            1,
            s.lift as i32,
            |v| v.to_string(),
            Id::Lift,
        );
        f.slider(
            "落下",
            "到这个档位恢复追踪，低于抬起档位",
            ASYM_LAND_MIN as i32,
            (s.lift as i32 - 1).max(ASYM_LAND_MIN as i32),
            1,
            s.land as i32,
            |v| v.to_string(),
            Id::Land,
        );
        f.note("和雷云给这只鼠标的“非对称抬升”用同样的滑块（抬起 2..26、落下 1..25，雷云默认 13 / 12）。鼠标能接受这些值但读不回来，所以每次写入都会重写一遍。");
    } else {
        f.slider("抬起距离", "传感器的下限是 1 毫米", 1, 3, 1, s.lift as i32, |v| format!("{v} 毫米"), Id::Lift);
        f.note("智能追踪让传感器自己适应表面。雷云对这只鼠标没有表面校准和鼠标垫预设，freenapse 也没有。");
    }
}

fn power(ui: &Ui, f: &mut Form) {
    let cfg = &ui.cfg;
    f.section_icon("电池", Some(Icon::Battery));
    f.battery();
    f.slider(
        "电量通知",
        "电量降到这个水平时托盘程序弹通知",
        5,
        50,
        1,
        cfg.tray.notify_percent as i32,
        |v| format!("{v} %"),
        Id::NotifyPercent,
    );

    f.section_icon("节能", Some(Icon::Power));
    f.slider(
        "空闲休眠",
        "闲置这么久后休眠（一动就唤醒）",
        1,
        15,
        1,
        (cfg.power.idle_seconds / 60) as i32,
        |v| if v == 1 { "1 分钟".to_string() } else { format!("{v} 分钟") },
        Id::Idle,
    );
    // Same rule as Synapse: the firmware only honours low power mode at 1000 Hz or less.
    let lpm_ok = cfg.polling.rate <= 1000;
    f.slider_opt(
        "低功耗模式",
        "无线时，电量低于这个水平就进入低功耗模式",
        5,
        100,
        5,
        cfg.power.low_battery_percent as i32,
        |v| format!("{v} %"),
        Id::LowBattery,
        lpm_ok,
    );
    if !lpm_ok {
        f.warning("回报率 2000 Hz 及以上时不支持低功耗模式。要用请先降低回报率。");
    }

    if ui.wireless && ui.spec.pairing {
        f.section_icon("HyperPolling 无线接收器", Some(Icon::Wifi));
        let leds = ["电池状态", "回报率", "关闭"].map(String::from);
        f.chips("指示灯", "接收器上的灯显示什么", &leds, Some((cfg.dongle.led as usize).clamp(1, 3) - 1), Id::DongleLed);
    }
}

pub const CATEGORIES: [&str; 11] = ["默认", "鼠标", "键盘", "DPI", "多媒体", "系统", "宏", "程序", "文本", "Hypershift", "禁用"];
pub const CAT_MOUSE: usize = 1;
pub const CAT_KEYBOARD: usize = 2;
pub const CAT_SYSTEM: usize = 5;
pub const CAT_MACRO: usize = 6;
/// Start a program: a one-step `launch:` macro played by the tray app.
pub const CAT_LAUNCH: usize = 7;
/// Type a text: a one-step `text:` macro played by the tray app.
pub const CAT_TEXT: usize = 8;
pub const CAT_HYPERSHIFT: usize = 9;
pub const CAT_DISABLED: usize = 10;

/// The single software step of a macro bound by the Program or Text function:
/// (`launch` or `text`, its value), when the assignment is exactly that.
pub fn software_step(spec: Option<&str>, macros_: &[MacroConfig]) -> Option<(&'static str, String)> {
    let (id, _) = parse_button(spec?).ok().flatten().and_then(|f| f.macro_binding())?;
    let m = macros_.iter().find(|m| m.id == id)?;
    if m.events.len() != 1 {
        return None;
    }
    if let Some(p) = m.events[0].strip_prefix("launch:") {
        return Some(("launch", p.to_string()));
    }
    if let Some(t) = m.events[0].strip_prefix("text:") {
        return Some(("text", t.to_string()));
    }
    None
}

pub fn category_of(spec: Option<&str>, macros_: &[MacroConfig]) -> usize {
    match software_step(spec, macros_) {
        Some(("launch", _)) => return CAT_LAUNCH,
        Some(("text", _)) => return CAT_TEXT,
        _ => {}
    }
    match spec {
        None | Some("default") => 0,
        Some(s) if s.starts_with("mouse:") || s.starts_with("turbo:") || s == "double-click" => CAT_MOUSE,
        Some(s) if s.starts_with("key:") || s.starts_with("turbo-key:") => CAT_KEYBOARD,
        Some(s) if s.starts_with("dpi") || s.starts_with("clutch:") => 3,
        Some(s) if s.starts_with("profile-") => CAT_SYSTEM,
        Some(s) if s.starts_with("media:") => 4,
        Some(s) if s.starts_with("system:") => CAT_SYSTEM,
        Some(s) if s.starts_with("macro") => CAT_MACRO,
        Some("hypershift") => CAT_HYPERSHIFT,
        Some("disabled") => CAT_DISABLED,
        _ => 0,
    }
}

/// Playback options in the order of the chips on the Customize page.
pub const PLAYBACKS: [&str; 5] = ["播放一次", "N 次", "按住时", "开关", "序列"];

pub fn playback_index(p: Playback) -> usize {
    match p {
        Playback::Times(1) => 0,
        Playback::Times(_) => 1,
        Playback::Held => 2,
        Playback::Toggle => 3,
        Playback::Sequence => 4,
    }
}

/// Split a turbo assignment into its plain form and the repeat rate:
/// `turbo:left:10` -> (`mouse:left`, Some(10)); `key:c` -> (`key:c`, None).
pub fn turbo_split(spec: &str) -> (String, Option<u8>) {
    let rate = |rest: &str| rest.rsplit_once(':').map(|(base, n)| (base.to_string(), n.parse::<u8>().ok()));
    if let Some((base, n)) = spec.strip_prefix("turbo:").and_then(rate) {
        return (format!("mouse:{base}"), n);
    }
    if let Some((base, n)) = spec.strip_prefix("turbo-key:").and_then(rate) {
        return (format!("key:{base}"), n);
    }
    (spec.to_string(), None)
}

/// Inverse of [`turbo_split`]: add a repeat rate to a mouse or key assignment.
pub fn with_turbo(base: &str, rate: Option<u8>) -> String {
    let Some(n) = rate else { return base.to_string() };
    let n = n.clamp(TURBO_MIN, TURBO_MAX);
    if let Some(b) = base.strip_prefix("mouse:") {
        format!("turbo:{b}:{n}")
    } else if let Some(k) = base.strip_prefix("key:") {
        format!("turbo-key:{k}:{n}")
    } else {
        base.to_string()
    }
}

/// (label, spec) choices inside a category.
pub fn options_for(cat: usize, stages: usize) -> Vec<(String, String)> {
    match cat {
        CAT_MOUSE => {
            let mut v: Vec<(String, String)> = CLICK_CODES.iter().map(|(n, _)| (pretty_button(n), format!("mouse:{n}"))).collect();
            v.push(("双击".to_string(), "double-click".to_string()));
            v
        }
        CAT_SYSTEM => {
            let mut v: Vec<(String, String)> =
                [("睡眠", "sleep"), ("电源", "power"), ("唤醒", "wake")].iter().map(|(l, s)| (l.to_string(), format!("system:{s}"))).collect();
            v.push(("循环到下一套配置".to_string(), "profile-cycle-up".to_string()));
            v.push(("循环到上一套配置".to_string(), "profile-cycle-down".to_string()));
            v
        }
        3 => {
            let mut v = vec![
                ("循环 DPI 档位".to_string(), "dpi-cycle".to_string()),
                ("上一档".to_string(), "dpi-up".to_string()),
                ("下一档".to_string(), "dpi-down".to_string()),
            ];
            for i in 1..=stages.clamp(1, 5) {
                v.push((format!("档位 {i}"), format!("dpi-stage:{i}")));
            }
            v.push(("按住切到 400 DPI".to_string(), "clutch:400".to_string()));
            v.push(("按住切到 800 DPI".to_string(), "clutch:800".to_string()));
            v
        }
        4 => [
            ("播放 / 暂停", "play-pause"),
            ("静音", "mute"),
            ("音量加", "vol-up"),
            ("音量减", "vol-down"),
            ("下一曲", "next"),
            ("上一曲", "prev"),
            ("停止", "stop"),
        ]
        .iter()
        .map(|(l, s)| (l.to_string(), format!("media:{s}")))
        .collect(),
        _ => Vec::new(),
    }
}

fn pretty_button(name: &str) -> String {
    match name {
        "left" => "左键单击",
        "right" => "右键单击",
        "middle" => "中键单击",
        "back" => "后退",
        "forward" => "前进",
        "scroll-up" => "向上滚动",
        "scroll-down" => "向下滚动",
        "dpi" => "DPI 按键",
        "dpi-up" => "DPI 加一档",
        "dpi-down" => "DPI 减一档",
        "tilt-left" => "滚轮左倾",
        "tilt-right" => "滚轮右倾",
        "clutch" => "按住切换灵敏度",
        "profile" => "配置按键",
        "scroll-mode" => "滚轮模式按键",
        other => other,
    }
    .to_string()
}

/// What a button does out of the box on the mouse whose buttons these are: a click is
/// itself, anything else is named after the function the table has for it.
fn default_label(own: &str, buttons: &[PhysicalButton], macros_: &[MacroConfig]) -> String {
    let Some(b) = buttons.iter().find(|b| b.name == own).filter(|_| crate::device::button_code(own).is_none()) else {
        return pretty_button(own);
    };
    match (crate::config::spec_for(&b.default), b.default.0) {
        (Some(spec), _) => pretty_spec(Some(&spec), own, buttons, macros_),
        // Functions the config has no value for (the Basilisk V3's scroll-mode button and tilts).
        (None, [0x12, ..]) => "滚轮模式".into(),
        (None, [0x0e, _, 0x68, ..]) => "向左滚动".into(),
        (None, [0x0e, _, 0x69, ..]) => "向右滚动".into(),
        (None, _) => pretty_button(own),
    }
}

/// Human description of an assignment for the button list.
pub fn pretty_spec(spec: Option<&str>, own: &str, buttons: &[PhysicalButton], macros_: &[MacroConfig]) -> String {
    if let Some(s) = spec {
        let (base, rate) = turbo_split(s);
        if let Some(n) = rate {
            return format!("{} · 连发 {n}/秒", pretty_spec(Some(&base), own, buttons, macros_));
        }
        match software_step(spec, macros_) {
            Some(("launch", p)) => return format!("程序：{}", macros::launch_label(&p)),
            Some(("text", t)) => return format!("文本：{t}"),
            _ => {}
        }
        if let Some((id, play)) = parse_button(s).ok().flatten().and_then(|f| f.macro_binding()) {
            let name = macros_.iter().find(|m| m.id == id).map(|m| m.name.clone()).unwrap_or_else(|| format!("宏 {id}"));
            return format!("宏：{name}（{}）", play.label().to_lowercase());
        }
    }
    match spec {
        None | Some("default") => format!("{}（默认）", default_label(own, buttons, macros_)),
        Some("disabled") => "禁用".into(),
        Some("hypershift") => "Hypershift".into(),
        Some("double-click") => "双击".into(),
        Some("system:sleep") => "睡眠".into(),
        Some("system:power") => "电源".into(),
        Some("system:wake") => "唤醒".into(),
        Some("dpi-cycle") => "DPI 循环".into(),
        Some("dpi-up") => "DPI 循环加".into(),
        Some("dpi-down") => "DPI 循环减".into(),
        Some(s) if s.starts_with("dpi-stage:") => format!("DPI 档位 {}", &s[10..]),
        Some(s) if s.starts_with("clutch:") => format!("按住切到 {} DPI", s[7..].replace(':', " x ")),
        Some("profile-next") => "下一套配置".into(),
        Some("profile-prev") => "上一套配置".into(),
        Some("profile-cycle-up") => "循环到下一套配置".into(),
        Some("profile-cycle-down") => "循环到上一套配置".into(),
        Some(s) if s.starts_with("mouse:") => pretty_button(&s[6..]),
        Some(s) if s.starts_with("media:") => options_for(4, 1).iter().find(|(_, v)| v == s).map(|(l, _)| l.clone()).unwrap_or_else(|| s.to_string()),
        Some("key:") => "键盘：按一个键".into(),
        Some(s) if s.starts_with("key:") => {
            let parts: Vec<String> = s[4..]
                .split('+')
                .map(|p| {
                    let p = p.trim();
                    if p.len() == 1 {
                        p.to_uppercase()
                    } else {
                        let mut c = p.chars();
                        c.next().map(|x| x.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
                    }
                })
                .collect();
            format!("按键：{}", parts.join(" + "))
        }
        Some(s) => s.to_string(),
    }
}

fn customize(ui: &Ui, f: &mut Form) {
    let cfg = &ui.cfg;
    f.section_icon("按键功能", Some(Icon::Mouse));
    let mut layers = vec!["普通层".to_string(), "Hypershift".to_string()];
    layers.extend(cfg.games.iter().map(|g| {
        let l = g.label();
        if l.chars().count() > 20 { format!("{}...", l.chars().take(18).collect::<String>().trim_end()) } else { l }
    }));
    let layer_sel = match ui.layer {
        Layer::Normal => 0,
        Layer::Hypershift => 1,
        Layer::Game(i) => 2 + i.min(cfg.games.len().saturating_sub(1)),
    };
    f.chips_wide("层", &layers, Some(layer_sel), Id::Layer);
    f.gap(2);

    // Left: list of the physical buttons. Right: the mouse with clickable areas.
    let list_w = 300;
    let row_h = 50;
    let y0 = f.y;
    let hs = ui.layer == Layer::Hypershift;
    let layer_buttons = ui.layer_buttons();
    let buttons = ui.buttons();
    for (i, b) in buttons.iter().enumerate() {
        let name = b.name;
        let key = if hs { format!("hypershift.{name}") } else { name.to_string() };
        let mut spec = layer_buttons.get(&key).map(String::as_str);
        let mut inherited = false;
        if let Layer::Game(_) = ui.layer {
            if spec.is_none() {
                spec = cfg.buttons.get(&key).map(String::as_str);
                inherited = spec.is_some();
            }
        }
        let mut sub = pretty_spec(spec, name, buttons, &cfg.macros);
        if inherited {
            sub.push_str(" · 来自普通层");
        }
        f.push(Id::SelectButton(i), f.x + PAD, f.y, list_w, row_h - 4, Kind::Row { title: pretty_button(name), sub, selected: ui.sel().name == name });
        f.y += row_h;
    }
    let art_h = (row_h * buttons.len() as i32 - 4).min(row_h * 7 - 4);
    let art_x = f.x + PAD + list_w + 24;
    let art_w = f.w - 2 * PAD - list_w - 24;
    let regions = mouse_regions(f, art_x, y0, art_w, art_h, buttons);
    let sel = buttons.iter().position(|b| b.name == ui.sel().name).unwrap_or(0);
    f.push(Id::SelectButton(sel), art_x, y0, art_w, art_h, Kind::MouseArt { highlight: sel, regions });
    if hs && !cfg.buttons.values().any(|v| v == "hypershift") {
        f.gap(6);
        f.note("还没有按键被设为 Hypershift，所以进不了这一层。请先在普通层把一个按键设为 Hypershift。");
    }
    if let Layer::Game(i) = ui.layer {
        f.gap(6);
        let exe = cfg.games.get(i).map(|g| g.label()).unwrap_or_else(|| "?".into());
        f.note(&format!("{exe} 在前台时使用的布局：游戏启动时由托盘程序写入鼠标，退出时恢复普通层。设为“默认”的按键沿用普通层的设置。"));
    }

    let name = ui.sel().name;
    let key = ui.button_key();
    let spec = layer_buttons.get(&key).map(String::as_str);
    let cat = if ui.key_mode.as_deref() == Some(key.as_str()) {
        CAT_KEYBOARD
    } else if ui.text_mode.as_deref() == Some(key.as_str()) {
        CAT_TEXT
    } else {
        category_of(spec, &cfg.macros)
    };
    let (base, rate) = spec.map(turbo_split).unwrap_or((String::new(), None));
    let layer_title = match ui.layer {
        Layer::Normal => String::new(),
        Layer::Hypershift => "（Hypershift）".to_string(),
        Layer::Game(i) => format!(" ({})", cfg.games.get(i).map(|g| g.label()).unwrap_or_default()),
    };
    f.section_icon(&format!("设置{}{layer_title}", pretty_button(name)), Some(Icon::Edit));
    let cats = CATEGORIES.map(String::from);
    f.chips_wide("功能", &cats, Some(cat), Id::Category(0));
    match cat {
        CAT_MOUSE | 3 | 4 | CAT_SYSTEM => {
            let opts = options_for(cat, cfg.dpi.stages.len());
            let labels: Vec<String> = opts.iter().map(|(l, _)| l.clone()).collect();
            let sel = opts.iter().position(|(_, s)| *s == base);
            f.chips_wide("", &labels, sel, Id::Option(0));
            if cat == CAT_MOUSE && base.starts_with("mouse:") {
                turbo_controls(f, rate);
            }
            if cat == CAT_SYSTEM {
                f.note("睡眠、关机、唤醒会发送系统的电源键，和某些键盘上的电源键一样，具体动作（睡眠、关机、唤醒）由 Windows 决定。");
            }
        }
        CAT_KEYBOARD => {
            let cur = base.strip_prefix("key:").unwrap_or("");
            f.edit("组合键", "按 Enter 保存", cur, false, Id::KeyEdit);
            if base.starts_with("key:") {
                turbo_controls(f, rate);
            }
            f.note("键名：a-z、0-9、f1-f24、enter、esc、tab、space、backspace、delete、home、end、pageup、pagedown、up、down、left、right 以及各标点键。修饰键：ctrl、shift、alt、win，例如 ctrl+shift+c。");
        }
        CAT_MACRO => {
            let bound = spec.and_then(|s| parse_button(s).ok().flatten()).and_then(|f| f.macro_binding());
            let names: Vec<String> = cfg.macros.iter().map(|m| m.name.clone()).collect();
            let sel = bound.and_then(|(id, _)| cfg.macros.iter().position(|m| m.id == id));
            f.chips_wide("宏", &names, sel, Id::Option(0));
            let play = bound.map(|(_, p)| p).unwrap_or(Playback::Times(1));
            f.chips_wide("播放方式", &PLAYBACKS.map(String::from), Some(playback_index(play)), Id::MacroPlay);
            if let Playback::Times(n) = play {
                if n > 1 {
                    f.slider("重复次数", "", 2, macros::TIMES_MAX as i32, 1, n as i32, |v| format!("{v} 次"), Id::MacroTimes);
                }
            }
            if ui.macro_onboard == Some(true) {
                f.note("宏从鼠标自己的存储里播放，不需要任何软件。按住＝按住时一直重复；“开关”＝按一下开始、再按一下停止；“序列”＝每按一次播放一步。");
            } else {
                f.note("这只鼠标没有宏存储空间，所以按键按下时由托盘程序播放宏（雷云对这类鼠标也是这样），请保持托盘程序运行。按键会发送 F13 到 F24 中的一个，由托盘程序转换成宏。按住＝按住时一直重复；“开关”＝按一下开始、再按一下停止；“序列”＝每按一次播放一步。");
            }
        }
        CAT_LAUNCH => {
            match software_step(spec, &cfg.macros) {
                Some((_, p)) => f.note(&format!("按下时启动 {p}。再点一次“程序”可以换一个。")),
                None => f.note("在弹出的对话框里选程序。之后按下按键就会启动它，或打开选中的文件。"),
            }
            f.note("启动动作由托盘程序完成（鼠标发送 F13–F24 中的一个，由托盘程序转成启动），请保持它运行。Windows 快捷键不需要指定程序：直接当键盘按键分配即可，例如 win+d。");
        }
        CAT_TEXT => {
            let cur = software_step(spec, &cfg.macros).filter(|(k, _)| *k == "text").map(|(_, t)| t).unwrap_or_default();
            f.edit("要输入的文本", "按 Enter 保存", &cur, false, Id::TextEdit);
            f.note("按下按键时由托盘程序把文本当作 Unicode 字符输入，所以任何键盘布局、任何字符都能用。请保持托盘程序运行。");
        }
        CAT_HYPERSHIFT => f.note("按住这个按键时进入 Hypershift 层：其它按键使用各自的 Hypershift 设置。"),
        CAT_DISABLED => f.note("这个按键不做任何事。"),
        _ => f.note("按键原本的功能。设置保存在鼠标里（板载配置 1），不需要任何软件在运行。"),
    }

    macros_section(ui, f);
}

/// The macro list with the recorder.
fn macros_section(ui: &Ui, f: &mut Form) {
    let cfg = &ui.cfg;
    f.section_icon("宏", Some(Icon::Keyboard));
    if ui.is_recording() {
        f.note(&format!(
            "已录制 {} 秒，共 {} 个事件。按键盘和鼠标按键（它们之间的间隔也会记录），然后点“停止录制”或按 Esc。",
            ui.rec_secs, ui.rec_count
        ));
        f.buttons(&[("停止录制", Id::MacroStop, true)]);
        return;
    }
    if cfg.macros.is_empty() {
        f.note("还没有宏。录一个：你按下的每个键和鼠标键、以及它们之间的停顿都会被保存，然后可以绑定到上面的按键。");
    }
    for (i, m) in cfg.macros.iter().enumerate() {
        let events = m.events().unwrap_or_default();
        let bytes = macros::encode(&events).len();
        let mut sub = macros::describe(&events);
        if sub.len() > 70 {
            sub.truncate(67);
            sub.push_str("...");
        }
        if sub.is_empty() {
            sub = "空".into();
        }
        f.push(
            Id::SelectMacro(i),
            f.x + PAD,
            f.y,
            f.w - 2 * PAD,
            44,
            Kind::Row { title: format!("{}  ·  编号 {}  ·  {} 字节", m.name, m.id, bytes), sub, selected: ui.sel_macro == Some(i) },
        );
        f.y += 48;
    }
    f.gap(4);
    let has_sel = ui.sel_macro.is_some_and(|i| i < cfg.macros.len());
    f.buttons(&[
        ("录制新宏", Id::MacroNew, true),
        ("重新录制", if has_sel { Id::MacroRecord } else { Id::None }, false),
        ("删除", if has_sel { Id::MacroDelete } else { Id::None }, false),
    ]);
    if let Some(m) = ui.sel_macro.and_then(|i| cfg.macros.get(i)) {
        f.edit("名称", "按 Enter 保存", &m.name, false, Id::MacroName);
        let steps = m.events.join(", ");
        f.note(&format!("步骤：{steps}"));
    }
    match (ui.macro_onboard, ui.macro_storage) {
        (Some(true), Some(st)) => f.note(&format!("鼠标里的宏存储：{} / {} 字节可用。步骤也可以在配置文件（[[macros]]）里编辑。", st.available, st.total)),
        (Some(false), _) => f.note(&format!("这只鼠标没有宏存储空间，宏由托盘程序播放（雷云对它也是这样），最多 {} 个不同绑定。步骤也可以在配置文件（[[macros]]）里编辑。", macros::MAX_SOFT_SLOTS)),
        _ => f.note("步骤也可以在配置文件（[[macros]]）里编辑。"),
    }
}

/// Turbo toggle and rate slider under a mouse or keyboard assignment.
fn turbo_controls(f: &mut Form, rate: Option<u8>) {
    f.toggle("连发", "按住按键时重复该功能", rate.is_some(), Id::Turbo);
    if let Some(n) = rate {
        f.slider("连发频率", "", TURBO_MIN as i32, TURBO_MAX as i32, 1, n as i32, |v| format!("{v} 次/秒"), Id::TurboRate);
    }
}

fn app(ui: &Ui, f: &mut Form) {
    let cfg = &ui.cfg;
    f.section_icon("托盘程序", Some(Icon::Program));
    f.toggle("开机启动", "登录时启动托盘程序（电池图标、游戏内回报率）", ui.autostart, Id::Autostart);
    f.slider(
        "电池刷新",
        "使用鼠标时，托盘图标多久读一次电量",
        10,
        300,
        10,
        cfg.tray.poll_seconds as i32,
        |v| format!("{v} 秒"),
        Id::PollSeconds,
    );
    f.gap(4);
    f.buttons(&[
        ("打开配置文件", Id::OpenConfig, false),
        ("从鼠标重新导入", Id::Reimport, false),
        ("重新全部写入", Id::Reapply, false),
        ("刷新", Id::Refresh, false),
    ]);
    f.note("“从鼠标重新导入”按鼠标里保存的内容重建配置；“重新全部写入”把配置里的每一项设置再写进鼠标一次。");

    f.section_icon("板载配置", Some(Icon::Layers));
    let (stored, max) = ui.profiles.unwrap_or((1, 1));
    let slots: Vec<String> = (1..=max.clamp(1, 5)).map(|n| n.to_string()).collect();
    f.chips(
        "当前使用的配置",
        "按键和动态灵敏度都会写进这个板载配置",
        &slots,
        Some((cfg.profile.active as usize).clamp(1, slots.len()) - 1),
        Id::Profile,
    );
    if max <= 1 {
        f.note("这只鼠标只报告一个板载配置，所以没有可切换的。这个选择器是给能存多套配置的雷蛇鼠标准备的。");
    } else {
        f.note(&format!("鼠标里有 {stored} / {max} 套配置。选一个空位会创建它并写入当前配置。"));
    }

    if ui.wireless && ui.spec.pairing {
        f.section_icon("接收器配对", Some(Icon::Link));
        match &ui.paired {
            Some(list) if !list.is_empty() => {
                for d in list {
                    let state = match d.status {
                        0 => "未连接",
                        1 => "已连接",
                        _ => "状态未知",
                    };
                    f.info("已配对的设备", &format!("产品 ID 0x{:04x}  ·  {state}", d.product_id));
                }
            }
            Some(_) => f.info("已配对的设备", "没有报告"),
            None => f.info("已配对的设备", "-"),
        }
        if ui.pairing_since.is_some() {
            f.note("配对模式已开启：把鼠标关掉再打开（或者把接收器插到鼠标旁边）。新配对的鼠标会显示在上面。");
            f.buttons(&[("取消配对", Id::PairCancel, true)]);
        } else {
            f.buttons(&[("配对设备", Id::PairStart, true), (if ui.confirm_unpair { "确定解除" } else { "解除配对这只鼠标" }, Id::Unpair, false)]);
        }
        if let Some(on) = ui.auto_pair {
            f.toggle("自动配对", "让接收器自己配对手边兼容的鼠标", on, Id::AutoPair);
        }
        f.note("和雷云的设备配对照的是同一组命令。解除配对会让接收器忘掉这只鼠标，在重新配对之前它就不能用了，所以只在换接收器时才用。");
    }

    f.section_icon("设备", Some(Icon::Info));
    let model = ui.spec.name.split(" (").next().unwrap_or(&ui.spec.name);
    let link = if ui.wireless { "通过无线接收器" } else { "有线" };
    let trust = if ui.spec.verified {
        ""
    } else if ui.cfg.device.allow_unverified.allows(ui.spec.product_id) {
        "  ·  未验证，已在设备页允许写入"
    } else {
        "  ·  未验证：只读，除非在设备页允许写入"
    };
    if ui.spec.is_placeholder() {
        f.info("鼠标", "还没有打开的鼠标");
    } else {
        f.info("鼠标", &format!("{model}，{link}（产品 ID 0x{:04x}）{trust}", ui.spec.product_id));
    }
    f.info("鼠标固件", &ui.fw.map(|(a, b)| format!("{a}.{b}")).unwrap_or_else(|| "-".into()));
    if ui.wireless {
        f.info("接收器固件", &ui.dongle_fw.map(|(a, b)| format!("{a}.{b}")).unwrap_or_else(|| "-".into()));
    }
    f.info("序列号", ui.serial.as_deref().unwrap_or("-"));
    f.info("配置文件", &crate::config_path().to_string_lossy());
    f.info(
        "状态",
        if ui.pending {
            "有改动待写入（鼠标在休眠）"
        } else if ui.asleep {
            "鼠标在休眠"
        } else if ui.battery.is_some() {
            "已连接"
        } else {
            "未连接"
        },
    );
    f.info("版本", &format!("freenapse {}", crate::VERSION));
    f.info("诊断", "崩溃后看配置文件目录里的 crash.txt；加上 --log 启动会把和鼠标的每一次通信记到同目录的 hid.log");

    f.section_icon("关于", Some(Icon::Help));
    f.note("freenapse 用两个小的原生程序取代雷云 4，不装任何服务。所有设置都存在鼠标的板载存储里，换台电脑也一样。设备页列出了 freenapse 认得的每一只鼠标，以及各自支持到什么程度。");
    f.link("支持的鼠标", "freenapse.app/devices", "https://freenapse.app/devices/", "每一只鼠标以及 freenapse 对它支持到什么程度");
    f.link("官网", "freenapse.app", "https://freenapse.app", "下载、更新日志和说明");
    f.link("源代码", "github.com/MatthewSaintBull/freenapse", "https://github.com/MatthewSaintBull/freenapse", "");
    f.info("许可", "MIT 许可，作者 MatthewSaintbull。与雷蛇公司无关。");
    f.note("Esc 关闭这个窗口，托盘图标会留着。");
}

/// Rows an owner has partly tested (reads and some writes), short of verified.
const TESTING: &[u16] = &[0x00B6];

/// How far freenapse goes with one row of the device table.
fn device_status(d: &DeviceSpec) -> Status {
    if d.verified {
        Status::Verified
    } else if TESTING.contains(&d.product_id) {
        Status::Testing
    } else {
        Status::ReadOnly
    }
}

/// One line per model and status: wired and wireless rows of the same mouse
/// share a line when they have the same status. (model, rows)
fn device_models(st: Status) -> Vec<(&'static str, Vec<&'static DeviceSpec>)> {
    let mut out: Vec<(&'static str, Vec<&'static DeviceSpec>)> = Vec::new();
    for d in crate::device::DEVICES.iter().filter(|d| device_status(d) == st) {
        let name: &'static str = match &d.name {
            std::borrow::Cow::Borrowed(s) => s,
            std::borrow::Cow::Owned(_) => continue,
        };
        let model = name.split(" (").next().unwrap_or(name).trim_start_matches("Razer ");
        match out.iter_mut().find(|(m, _)| *m == model) {
            Some((_, rows)) => rows.push(d),
            None => out.push((model, vec![d])),
        }
    }
    out
}

/// "0x00C0 wired, 0x00C1 wireless  ·  35000 DPI  ·  125-8000 Hz  ·  7 buttons"
fn device_note(rows: &[&DeviceSpec]) -> String {
    let ids: Vec<String> = rows.iter().map(|d| format!("0x{:04X} {}", d.product_id, if d.wireless { "无线" } else { "有线" })).collect();
    let dpi = rows.iter().map(|d| d.dpi_max).max().unwrap_or(0);
    let hz = rows.iter().flat_map(|d| d.polling_rates.iter().copied()).max().unwrap_or(1000);
    let mut parts = vec![ids.join(", "), format!("{dpi} DPI"), format!("125-{hz} Hz")];
    if device_status(rows[0]) != Status::ReadOnly {
        parts.push(format!("{} 个按键", rows[0].buttons.len()));
    }
    if !rows[0].lighting.is_empty() {
        parts.push("灯光".into());
    }
    if !rows.iter().any(|d| d.battery) {
        parts.push("无电池".into());
    }
    parts.join("  \u{b7}  ")
}

/// The device table: verified, being tested, and known but read-only.
fn devices(ui: &Ui, f: &mut Form) {
    let here = |rows: &[&DeviceSpec]| ui.battery.is_some() && rows.iter().any(|d| d.product_id == ui.spec.product_id);

    f.section_icon("已连接", Some(Icon::Mouse));
    if ui.battery.is_some() {
        let model = ui.spec.name.split(" (").next().unwrap_or(&ui.spec.name);
        let allowed = ui.cfg.device.allow_unverified.allows(ui.spec.product_id);
        let state = match (ui.spec.verified, allowed) {
            (true, _) => "已验证，所有设置都会写入",
            (false, true) => "未验证：由你允许写入，用于测试",
            (false, false) => "未验证：只读",
        };
        f.info("鼠标", &format!("{model}（产品 ID 0x{:04x}）  \u{b7}  {state}", ui.spec.product_id));
    } else {
        f.info("鼠标", "没有连接的鼠标");
    }
    // The owner's opt-in, per product id ([device] allow_unverified in the config file).
    if !ui.spec.verified && !ui.spec.is_placeholder() {
        let allowed = ui.cfg.device.allow_unverified.allows(ui.spec.product_id);
        let sub = if allowed {
            "开：在这里、在托盘菜单里、以及用命令行做的改动都会写入这只鼠标"
        } else {
            "关：freenapse 只读取这只鼠标。打开即可测试写入，风险自负"
        };
        f.toggle("允许写入这只鼠标", sub, allowed, Id::AllowUnverified);
        f.note("只对这个型号（这个产品 ID）有效，其它未验证的鼠标仍然是只读。测试完之后，请把鼠标接受了什么、拒绝了什么发到“New mouse”issue 里，这样就可以把它标为已验证。");
    }

    let groups: [(&str, Icon, Status, &str); 3] = [
        ("已验证", Icon::Check, Status::Verified, "在这只鼠标上逐项写入并读回验证过。freenapse 会直接写入这些鼠标，不再询问。"),
        (
            "测试中",
            Icon::Warning,
            Status::Testing,
            "有用户读回验证了部分设置。要写入需要用户自己允许：接上鼠标后用上面的开关。",
        ),
        (
            "只读",
            Icon::Info,
            Status::ReadOnly,
            "已知产品 ID、回报率命令和 DPI 上限；status 和 probe 可用。上面那个开关打开之前，所有写入都会被拒绝。",
        ),
    ];
    for (title, icon, st, text) in groups {
        let rows = device_models(st);
        if rows.is_empty() {
            continue;
        }
        f.section_icon(title, Some(icon));
        f.note(text);
        for (model, specs) in &rows {
            let mut note = device_note(specs);
            if here(specs) {
                note.push_str("  \u{b7}  当前连接");
            }
            f.status(st, model, &note);
        }
    }

    f.section_icon("其它雷蛇鼠标", Some(Icon::Help));
    f.note("它会以只读方式打开，假定使用经典回报率命令、DPI 上限 30000。想把它加进来：运行 freenapse probe，把输出贴到 GitHub 的“New mouse”issue，然后测试写入，并把 src/device.rs 里它那一行发过来（步骤见 CONTRIBUTING.md）。");
    f.link("在线查看同一份列表", "freenapse.app/devices", "https://freenapse.app/devices/", "");
    f.link("提交新鼠标", "github.com/MatthewSaintBull/freenapse/issues", "https://github.com/MatthewSaintBull/freenapse/issues/new/choose", "");
}

/// Feature status and release roadmap (same content as ROADMAP.md).
fn roadmap(f: &mut Form) {
    use Status::*;
    f.section_icon("功能对照", Some(Icon::Flag));
    f.note("freenapse 目前能做什么，以及与雷云 4 的对比。已完成＝在鼠标上验证过；部分＝能用但有局限；计划中＝在待办列表里；不支持＝不在范围内。");

    f.section("性能");
    f.status(Done, "DPI 档位与当前档位", "最多 5 档，命令行、配置文件和本窗口都能改");
    f.status(Done, "即时 DPI", "从 100 到鼠标上限（按型号 20000 至 45000），步进 1 DPI");
    f.status(Done, "X/Y 轴独立 DPI", "命令行、配置文件（y_stages）以及上面的 X/Y 开关");
    f.status(Done, "回报率 125..8000 Hz", "8000 需要 HyperPolling 接收器");
    f.status(Done, "游戏内回报率", "当带配置的游戏在前台时");
    f.status(Done, "传感器旋转", "角度微调 -30..30 度");
    f.status(Done, "动态灵敏度", "经典 / 自然 / 跳跃预设以及自定义曲线编辑，保存在鼠标里");
    f.status(Never, "灵敏度匹配", "雷云里纯软件的辅助功能；直接设成你要的 DPI 即可");

    f.section("校准");
    f.status(Done, "智能追踪 1/2/3 毫米", "");
    f.status(Done, "非对称抬升", "抬起 2..26、落下 1..25，按雷云相同的顺序写入；适用于带 Focus Pro 追踪页的鼠标");
    f.status(
        Never,
        "表面校准、鼠标垫预设",
        "只在 Viper V3 Pro 上试过，雷云对它没有这项功能，固件也拒绝这些命令",
    );
    f.status(Partial, "读回当前校准", "非对称档位读不出来，所以选“保持当前”就维持原样");

    f.section("电源");
    f.status(Done, "电量与充电", "托盘图标、窗口标题栏、命令行");
    f.status(Done, "低电量通知", "阈值可在配置文件和窗口里设置");
    f.status(Done, "空闲休眠", "1..15 分钟");
    f.status(Done, "低功耗模式门槛", "5..100 %；2000 Hz 及以上固件会忽略它，雷云也一样");
    f.status(Done, "接收器指示灯模式", "HyperPolling 接收器");

    f.section("自定义");
    f.status(Done, "按键重映射", "鼠标布局里的每个按键，普通层和 Hypershift 层都有，保存在鼠标里");
    f.status(Done, "按键功能", "鼠标功能、带修饰键的键盘、多媒体、DPI 档位／循环、Hypershift、禁用");
    f.status(Done, "双击、连发、系统键", "双击、可设频率的连发、睡眠／关机／唤醒");
    f.status(
        Partial,
        "配置切换按键",
        "配置相关的功能可以绑到任意按键；只有在能存多套板载配置的鼠标上才有作用",
    );
    f.status(Done, "宏", "可以在这里录制，也可以在配置文件里写步骤；播放一次／N 次／按住重复／开关／序列。鼠标有宏存储时存在鼠标里，否则由托盘程序播放，和雷云一样");
    f.status(
        Done,
        "程序与文本按键",
        "启动程序或打开文件／网址、输入文本；由托盘程序像宏一样播放。Windows 快捷键直接当键盘按键分配（win+d）",
    );

    f.section("程序与设备");
    f.status(Done, "设置窗口，改动立即写入", "鼠标休眠时挂起，唤醒后写入");
    f.status(Done, "托盘图标", "电池、DPI 档位、回报率菜单");
    f.status(Done, "配置文件、导入、重新写入", "%APPDATA%\\freenapse\\config.toml");
    f.status(Done, "开机启动", "注册表 Run 键");
    f.status(Done, "固件与序列号", "鼠标和接收器");
    f.status(Done, "板载配置选择", "配置文件、命令行和设置页；只报告一个配置的鼠标只接受 1");
    f.status(Done, "游戏配置", "每个游戏单独的 DPI 档位、回报率和按键布局，由托盘程序切换");
    f.status(Done, "游戏库", "Steam、Epic、GOG、EA、Battle.net、Riot、Ubisoft、Rockstar、Xbox；其它程序用文件对话框添加");
    f.status(Done, "接收器配对", "在设置页和命令行里配对、取消、解除配对以及自动配对");
    f.status(Todo, "蓝牙", "还没有已验证的鼠标用到它；先留着给用到的型号");
    f.status(Partial, "其它雷蛇鼠标", "由设备表决定每个型号支持什么：已验证、测试中或只读，见设备页");
    f.status(Never, "固件更新", "需要时用雷蛇官方更新工具升一次即可");
    f.status(Partial, "Chroma 灯效", "在有灯的鼠标上按区域设置关闭、常亮、呼吸、光谱（Basilisk V2）；没有波浪或逐键帧");
    f.status(Never, "账号、云同步", "遥测也没有；这是有意为之");

    f.section_icon("下个版本：已完成", Some(Icon::Checklist));
    f.bullet(ACCENT, "宏：在“自定义”页录制（按键、鼠标键、滚轮、间隔），用五种播放方式绑定。在没有宏存储的鼠标上由托盘程序通过 F13..F24 播放，和雷云一样；已验证：从鼠标读回了槽位按键，按键按下时宏被播放。");
    f.bullet(ACCENT, "游戏配置：每个程序单独的 DPI 档位、回报率和按键布局；已验证：记事本切到前台约 2 秒后回报率和按键被切换，关闭后又恢复。");
    f.bullet(ACCENT, "接收器配对：在设置页和命令行里配对、取消、解除配对；已验证：已配对列表、配对模式的开启与取消（解除配对没有故意去试；这个接收器不支持自动配对）。");

    f.section("上上个版本");
    f.bullet(MUTED, "双击、连发和系统键；带曲线编辑的动态灵敏度；X/Y 轴独立 DPI；板载配置选择；“校准”页按雷云的引擎为 Focus Pro 追踪页重建。");

    f.section_icon("以后", Some(Icon::Checklist));
    f.bullet(BLUE, "程序与文本按键：用一个按键启动程序或输入文本，由托盘程序像宏一样播放。热插拔和唤醒后自动重连、配置文件被编辑后自动重新写入，以及每天检查一次更新。");
    f.bullet(BLUE, "一张产品表，让共用这套协议的其它雷蛇鼠标不用改代码就能加入。");
    f.bullet(BLUE, "单文件签名下载。");
    f.bullet(MUTED, "支持带蓝牙的鼠标用蓝牙连接。");

    f.section("不打算做");
    f.note("雷蛇账号、云配置同步、遥测、固件更新和灵敏度匹配。freenapse 的意义就是不需要这些。");
}

// ----------------------------------------------------------------------
// Mouse illustration (top view) with clickable button areas.
// ----------------------------------------------------------------------

/// Geometry of the drawing in logical units relative to the art box.
struct MouseGeom {
    cx: i32,
    top: i32,
    w: i32,
    h: i32,
}

fn geom(x: i32, y: i32, w: i32, h: i32) -> MouseGeom {
    let mh = (h - 20).min(300);
    let mw = (mh * 62 / 100).min(w - 140);
    MouseGeom { cx: x + w / 2 + 20, top: y + (h - mh) / 2, w: mw, h: mh }
}

/// Clickable regions (button index → scaled rect), computed with the same geometry as the drawing.
fn mouse_regions(f: &Form, x: i32, y: i32, w: i32, h: i32, buttons: &[crate::device::PhysicalButton]) -> Vec<(usize, RECT)> {
    let g = geom(x, y, w, h);
    let s = |v: i32| f.s(v);
    let l = g.cx - g.w / 2;
    let split = g.top + g.h * 42 / 100;
    let wheel_w = g.w / 6;
    let wheel_h = g.h / 7;
    let wheel_y = g.top + g.h * 12 / 100;
    let r = |x: i32, y: i32, w: i32, h: i32| RECT { left: s(x), top: s(y), right: s(x + w), bottom: s(y + h) };
    let mut v: Vec<(usize, RECT)> = extra_shapes(&g, buttons)
        .into_iter()
        .filter_map(|(name, (ex, ey, ew, eh))| buttons.iter().position(|b| b.name == name).map(|i| (i, r(ex, ey, ew, eh))))
        .collect();
    v.extend([
        (0, r(l, g.top, g.w / 2, split - g.top)),
        (1, r(g.cx, g.top, g.w / 2, split - g.top)),
        (2, r(g.cx - wheel_w / 2, wheel_y, wheel_w, wheel_h)),
        (5, r(g.cx - wheel_w / 2 - 4, wheel_y - 18, wheel_w + 8, 18)),
        (6, r(g.cx - wheel_w / 2 - 4, wheel_y + wheel_h, wheel_w + 8, 18)),
        (3, r(l - 14, g.top + g.h * 48 / 100, 20, g.h * 12 / 100)),
        (4, r(l - 14, g.top + g.h * 34 / 100, 20, g.h * 12 / 100)),
    ]);
    v
}

/// Where the buttons beyond the seven clicks sit on the drawing, in unscaled
/// coordinates: the DPI button (or the Basilisk's DPI up and down) behind the
/// wheel, the tilts on either side of it, the clutch on the side below the
/// side buttons. On some mice (the Viper V3 Pro) the DPI button is really
/// underneath; the drawing puts it where a hand would look for it.
/// (x, y, w, h)
type Area = (i32, i32, i32, i32);

fn extra_shapes(g: &MouseGeom, buttons: &[crate::device::PhysicalButton]) -> Vec<(&'static str, Area)> {
    // A scroll-mode button sits right behind the wheel and pushes the DPI button back.
    let dpi_at = if buttons.iter().any(|b| b.name == "scroll-mode") { 37 } else { 32 };
    let l = g.cx - g.w / 2;
    let wheel_w = g.w / 6;
    let wheel_h = g.h / 7;
    let wheel_y = g.top + g.h * 12 / 100;
    let dpi_w = wheel_w - 6;
    let dpi_h = g.h * 4 / 100;
    vec![
        ("tilt-left", (g.cx - wheel_w / 2 - 14, wheel_y + wheel_h / 4, 10, wheel_h / 2)),
        ("tilt-right", (g.cx + wheel_w / 2 + 4, wheel_y + wheel_h / 4, 10, wheel_h / 2)),
        ("scroll-mode", (g.cx - dpi_w / 2, g.top + g.h * 32 / 100, dpi_w, dpi_h)),
        ("dpi", (g.cx - dpi_w / 2, g.top + g.h * dpi_at / 100, dpi_w, dpi_h)),
        ("dpi-up", (g.cx - dpi_w / 2, g.top + g.h * 32 / 100, dpi_w, dpi_h)),
        ("dpi-down", (g.cx - dpi_w / 2, g.top + g.h * 37 / 100, dpi_w, dpi_h)),
        ("clutch", (l - 12, g.top + g.h * 62 / 100, 16, g.h * 8 / 100)),
    ]
}

pub unsafe fn draw_mouse(ui: &Ui, dc: HDC, r: &RECT, highlight: usize, regions: &[(usize, RECT)]) {
    let s = |v: i32| ui.s(v);
    let scale = ui.scale;
    // Work in device pixels: regions already are; rebuild the geometry from the rect.
    let (x, y, w, h) = ((r.left as f32 / scale) as i32, 0, ((r.right - r.left) as f32 / scale) as i32, ((r.bottom - r.top) as f32 / scale) as i32);
    let g = geom(x, y, w, h);
    let oy = r.top; // device offset for the top of the art box
    let dx = |v: i32| s(v);
    let dy = |v: i32| s(v) + oy;

    let l = g.cx - g.w / 2;
    let rt = g.cx + g.w / 2;
    let split = g.top + g.h * 42 / 100;
    let body = RECT { left: dx(l), top: dy(g.top), right: dx(rt), bottom: dy(g.top + g.h) };
    let hl = |i: usize| if highlight == i { (ACCENT_HI, ACCENT) } else { (rgb(0x46, 0x47, 0x4b), rgb(0x34, 0x35, 0x38)) };

    // Body silhouette (rounded, lit from the top), then the two main buttons on top of it.
    shadow(dc, &body, s(g.w / 2), 4);
    clip_round(dc, &body, s(g.w / 2), || vgradient(dc, &body, rgb(0x3a, 0x3b, 0x3e), rgb(0x24, 0x25, 0x27)));
    let btn = |i: usize| -> RECT {
        if i == 0 {
            RECT { left: dx(l), top: dy(g.top), right: dx(g.cx), bottom: dy(split) }
        } else {
            RECT { left: dx(g.cx), top: dy(g.top), right: dx(rt), bottom: dy(split) }
        }
    };
    // Clip the button fills to the body outline.
    let rgn = windows_sys::Win32::Graphics::Gdi::CreateRoundRectRgn(body.left, body.top, body.right + 1, body.bottom + 1, s(g.w), s(g.w));
    windows_sys::Win32::Graphics::Gdi::SelectClipRgn(dc, rgn);
    for i in 0..2 {
        let (top, bot) = hl(i);
        vgradient(dc, &btn(i), top, bot);
    }
    // Light from the top left.
    reflection(dc, &body, 34);
    windows_sys::Win32::Graphics::Gdi::SelectClipRgn(dc, 0);
    windows_sys::Win32::Graphics::Gdi::DeleteObject(rgn);
    round(dc, &body, s(g.w / 2), None, Some((rgb(0x5a, 0x5b, 0x5e), s(2))));
    polyline(dc, &[POINT { x: dx(g.cx), y: dy(g.top + 6) }, POINT { x: dx(g.cx), y: dy(split) }], BG, s(2));
    polyline(dc, &[POINT { x: dx(l + 2), y: dy(split) }, POINT { x: dx(rt - 2), y: dy(split) }], BG, s(2));

    // Scroll wheel and the scroll up/down arrows.
    let wheel_w = g.w / 6;
    let wheel_h = g.h / 7;
    let wheel_y = g.top + g.h * 12 / 100;
    let wheel = RECT { left: dx(g.cx - wheel_w / 2), top: dy(wheel_y), right: dx(g.cx + wheel_w / 2), bottom: dy(wheel_y + wheel_h) };
    let (wt, wb) = hl(2);
    clip_round(dc, &wheel, s(wheel_w / 2), || vgradient(dc, &wheel, wt, wb));
    round(dc, &wheel, s(wheel_w / 2), None, Some((BG, s(2))));
    let arrow = |up: bool, col: u32| {
        let cx = dx(g.cx);
        let (y0, y1) = if up { (dy(wheel_y - 6), dy(wheel_y - 14)) } else { (dy(wheel_y + wheel_h + 6), dy(wheel_y + wheel_h + 14)) };
        polygon(dc, &[POINT { x: cx - s(6), y: y0 }, POINT { x: cx + s(6), y: y0 }, POINT { x: cx, y: y1 }], Some(col), None);
    };
    arrow(true, if highlight == 5 { ACCENT } else { rgb(0x66, 0x66, 0x66) });
    arrow(false, if highlight == 6 { ACCENT } else { rgb(0x66, 0x66, 0x66) });

    // Side buttons (left side of the mouse): forward above back.
    for (i, frac) in [(4usize, 34), (3usize, 48)] {
        let sb = RECT { left: dx(l - 10), top: dy(g.top + g.h * frac / 100), right: dx(l + 6), bottom: dy(g.top + g.h * (frac + 12) / 100) };
        let (st, sbt) = hl(i);
        clip_round(dc, &sb, s(4), || vgradient(dc, &sb, st, sbt));
        round(dc, &sb, s(4), None, Some((BG, s(2))));
    }

    // Buttons only some mice have, drawn where the open mouse has them.
    for (name, (ex, ey, ew, eh)) in extra_shapes(&g, ui.buttons()) {
        let Some(i) = ui.buttons().iter().position(|b| b.name == name) else { continue };
        let rr = RECT { left: dx(ex), top: dy(ey), right: dx(ex + ew), bottom: dy(ey + eh) };
        let (t, b) = hl(i);
        clip_round(dc, &rr, s(3), || vgradient(dc, &rr, t, b));
        round(dc, &rr, s(3), None, Some((BG, s(1))));
    }

    // Callout labels with leader lines.
    let label = |i: usize, text_: &str, anchor: POINT, left_side: bool| {
        let col = if highlight == i { ACCENT } else { MUTED };
        let tw = text_width(dc, ui.fonts().small, text_);
        // Chinese labels are wider than the English ones the spacing was drawn for, so the
        // right-hand callouts start past the mouse body instead of on top of it.
        let lx = if left_side { anchor.x - s(26) - tw } else { (anchor.x + s(26)).max(rt + s(12)) };
        let end = POINT { x: if left_side { lx + tw + s(4) } else { lx - s(4) }, y: anchor.y };
        polyline(dc, &[anchor, end], col, s(1));
        circle(dc, anchor.x, anchor.y, s(3), Some(col), None);
        text(dc, &rect(lx, anchor.y - s(9), tw + s(4), s(18)), text_, ui.fonts().small, col, DT_ROW);
    };
    // Left side: the left button and the two side buttons. Right side: wheel and right button.
    label(0, "左键单击", POINT { x: dx(l + 4), y: dy(g.top + g.h * 24 / 100) }, true);
    label(4, "前进", POINT { x: dx(l - 4), y: dy(g.top + g.h * 40 / 100) }, true);
    label(3, "后退", POINT { x: dx(l - 4), y: dy(g.top + g.h * 54 / 100) }, true);
    label(5, "向上滚动", POINT { x: dx(g.cx + 6), y: dy(wheel_y - 10) }, false);
    label(2, "中键单击", POINT { x: dx(g.cx + wheel_w / 2), y: dy(wheel_y + wheel_h / 2) }, false);
    label(6, "向下滚动", POINT { x: dx(g.cx + 6), y: dy(wheel_y + wheel_h + 10) }, false);
    label(1, "右键单击", POINT { x: dx(rt - 6), y: dy(g.top + g.h * 38 / 100) }, false);
    let _ = regions;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{BASILISK_V2_BUTTONS, BASILISK_V3_BUTTONS, BUTTONS};

    #[test]
    fn a_default_is_named_after_the_mouse_that_has_the_button() {
        let label = |own: &str, buttons: &[PhysicalButton]| pretty_spec(None, own, buttons, &[]);
        assert_eq!(label("left", BUTTONS), "左键单击（默认）");
        assert_eq!(label("dpi", BUTTONS), "DPI 循环（默认）");
        // The Basilisk V3 (issue 5): functions without a config value still get one name.
        assert_eq!(label("dpi", BASILISK_V3_BUTTONS), "DPI 循环（默认）");
        assert_eq!(label("scroll-mode", BASILISK_V3_BUTTONS), "滚轮模式（默认）");
        assert_eq!(label("tilt-left", BASILISK_V3_BUTTONS), "向左滚动（默认）");
        assert_eq!(label("tilt-right", BASILISK_V3_BUTTONS), "向右滚动（默认）");
        assert_eq!(label("clutch", BASILISK_V3_BUTTONS), "按住切到 400 DPI（默认）");
        assert_eq!(label("clutch", BASILISK_V2_BUTTONS), "按住切到 788 DPI（默认）");
        assert_eq!(pretty_spec(Some("disabled"), "dpi", BASILISK_V3_BUTTONS, &[]), "禁用");
    }
}
