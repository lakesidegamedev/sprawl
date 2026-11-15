//! The Slate look: navy panels, 1 px borders, 8 px corners, Inter for text and numbers, the pixel face for headings only.

#![allow(deprecated)]

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Stroke};

// The studio palette shared with Corporate Warfare: dark navy panels, light type, gold for the
// primary action and money. No cream or beige.
/// Dark outline and shape edges.
pub const LINE: Color32 = Color32::from_rgb(11, 22, 34);
/// Panel edge.
pub const EDGE: Color32 = Color32::from_rgb(34, 54, 74);
/// Type on controls and headings.
pub const INK: Color32 = Color32::from_rgb(232, 238, 245);
pub const PANEL: Color32 = Color32::from_rgb(19, 33, 47);
pub const PANEL_DARK: Color32 = Color32::from_rgb(11, 22, 34);
pub const RAISED: Color32 = Color32::from_rgb(26, 43, 60);
pub const ACCENT: Color32 = Color32::from_rgb(226, 176, 74);
pub const GOOD: Color32 = Color32::from_rgb(63, 191, 138);
pub const BAD: Color32 = Color32::from_rgb(229, 96, 94);
pub const TEXT: Color32 = Color32::from_rgb(232, 238, 245);

/// Phosphor icon glyphs (MIT) in the "icons" font family. Only the ones the game uses.
pub mod icon {
    pub const HAMMER: &str = "\u{e80e}";
    pub const ROAD: &str = "\u{e838}";
    pub const DISTRICTS: &str = "\u{e464}";
    pub const ORDINANCES: &str = "\u{eb7a}";
    pub const TAXES: &str = "\u{e3ec}";
    pub const BUDGET: &str = "\u{e78e}";
    pub const BONDS: &str = "\u{e0b4}";
    pub const GRAPHS: &str = "\u{e154}";
    pub const NEWS: &str = "\u{e344}";
    pub const RIVALS: &str = "\u{e67e}";
    pub const VOTERS: &str = "\u{e68e}";
    pub const MAP: &str = "\u{e31a}";
    pub const SUN: &str = "\u{e472}";
    pub const CLOUD: &str = "\u{e1aa}";
    pub const RAIN: &str = "\u{e1b4}";
    pub const SNOW: &str = "\u{e1b8}";
    pub const STORM: &str = "\u{e1b2}";
    pub const WIND: &str = "\u{e5d2}";
    pub const PAUSE: &str = "\u{e39e}";
    pub const PLAY: &str = "\u{e3d0}";
    pub const FAST: &str = "\u{e6a6}";
    pub const MENU: &str = "\u{e2f0}";
    pub const CLOSE: &str = "\u{e4f6}";
    pub const PEOPLE: &str = "\u{e4d6}";
    pub const CALENDAR: &str = "\u{e10a}";
    pub const TREND_UP: &str = "\u{e4ae}";
    pub const TREND_DOWN: &str = "\u{e4ac}";
    pub const SETTINGS: &str = "\u{e270}";
    pub const INFO: &str = "\u{e2ce}";
    pub const HOUSE: &str = "\u{e2c2}";
    pub const CARET: &str = "\u{e13a}";
    pub const EYE: &str = "\u{e220}";
}

/// Text as written: sentence case. Kept so call sites read the same everywhere.
pub fn ui_font(size: f32) -> FontId {
    FontId::new(size * 1.12, FontFamily::Name("ui".into()))
}

/// The pixel face: window and screen titles and the logo only.
pub fn pixel_face(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("pixel".into()))
}

/// The icon font at a size.
pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("icons".into()))
}

/// An icon as rich text.
pub fn icon_text(glyph: &str, size: f32, col: Color32) -> egui::RichText {
    egui::RichText::new(glyph).font(icon_font(size)).color(col)
}

// ----- motion -----

static REDUCE_MOTION: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set once per frame from the settings.
pub fn set_reduce_motion(on: bool) {
    REDUCE_MOTION.store(on, std::sync::atomic::Ordering::Relaxed);
}

pub fn reduce_motion() -> bool {
    REDUCE_MOTION.load(std::sync::atomic::Ordering::Relaxed)
}

/// Seconds for a normal transition, 0 when motion is reduced.
pub fn dur(secs: f32) -> f32 {
    if reduce_motion() { 0.0 } else { secs }
}

/// Ease a number toward its target in `secs` (screen figures count up, bars fill).
pub fn ease(
    ctx: &egui::Context,
    id: impl std::hash::Hash + std::fmt::Debug,
    target: f32,
    secs: f32,
) -> f32 {
    ctx.animate_value_with_time(egui::Id::new(id), target, dur(secs))
}

/// 0..1 for an on or off state, eased.
pub fn ease_bool(
    ctx: &egui::Context,
    id: impl std::hash::Hash + std::fmt::Debug,
    on: bool,
    secs: f32,
) -> f32 {
    ctx.animate_bool_with_time_and_easing(
        egui::Id::new(id),
        on,
        dur(secs),
        egui::emath::easing::cubic_out,
    )
}

/// A light-weight eased integer for the top bar (counts to the new value).
pub fn count_to(
    ctx: &egui::Context,
    id: impl std::hash::Hash + std::fmt::Debug,
    target: i64,
) -> i64 {
    ease(ctx, id, target as f32, 0.35).round() as i64
}

/// Panel fill, raised control fill, edge colour and corner radius of the Slate look.
pub const FILL: Color32 = Color32::from_rgb(18, 30, 43);
pub const RAISED_FILL: Color32 = Color32::from_rgb(29, 45, 62);
pub const EDGE_LIGHT: Color32 = Color32::from_rgb(48, 68, 90);
pub const MUTED: Color32 = Color32::from_rgb(150, 166, 186);
pub const RADIUS: u8 = 8;

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (key, bytes) in [
        (
            "silk-bold",
            &include_bytes!("../assets/Silkscreen-Bold.ttf")[..],
        ),
        ("inter", &include_bytes!("../assets/Inter-Regular.ttf")[..]),
        (
            "inter-semi",
            &include_bytes!("../assets/Inter-SemiBold.ttf")[..],
        ),
        ("phosphor", &include_bytes!("../assets/Phosphor.ttf")[..]),
    ] {
        fonts.font_data.insert(
            key.into(),
            std::sync::Arc::new(FontData::from_static(bytes)),
        );
    }
    fonts
        .families
        .insert(FontFamily::Name("pixel".into()), vec!["silk-bold".into()]);
    fonts
        .families
        .insert(FontFamily::Name("ui".into()), vec!["inter-semi".into()]);
    fonts
        .families
        .insert(FontFamily::Name("icons".into()), vec!["phosphor".into()]);
    // plain text (tips, inputs) in the clean face too
    fonts
        .families
        .insert(FontFamily::Proportional, vec!["inter".into()]);
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = FILL;
    v.window_fill = FILL;
    v.window_stroke = Stroke::new(1.0, EDGE_LIGHT);
    v.window_corner_radius = RADIUS.into();
    v.menu_corner_radius = RADIUS.into();
    v.window_shadow = soft_shadow();
    v.popup_shadow = soft_shadow();
    v.override_text_color = Some(TEXT);
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, LINE);
    let w = &mut v.widgets;
    for s in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        s.corner_radius = (RADIUS - 2).into();
        s.bg_stroke = Stroke::new(1.0, EDGE_LIGHT);
        s.fg_stroke = Stroke::new(1.5, INK);
    }
    w.noninteractive.bg_fill = FILL;
    w.noninteractive.weak_bg_fill = FILL;
    w.inactive.bg_fill = RAISED_FILL;
    w.inactive.weak_bg_fill = RAISED_FILL;
    w.hovered.bg_fill = Color32::from_rgb(40, 64, 88);
    w.hovered.weak_bg_fill = Color32::from_rgb(40, 64, 88);
    w.hovered.bg_stroke = Stroke::new(1.0, ACCENT);
    w.active.bg_fill = Color32::from_rgb(120, 92, 36);
    w.active.weak_bg_fill = Color32::from_rgb(120, 92, 36);
    w.active.bg_stroke = Stroke::new(1.0, ACCENT);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.slider_width = 200.0;
        // hover and press states ease in about 150 ms
        s.animation_time = 0.15;
        if cfg!(target_os = "ios") {
            // fingers, not a pointer: every control is at least 44 points tall and wide
            s.spacing.interact_size = egui::vec2(44.0, 44.0);
            s.spacing.button_padding = egui::vec2(14.0, 10.0);
            s.spacing.item_spacing = egui::vec2(10.0, 10.0);
            s.spacing.icon_width = 28.0;
            s.spacing.slider_rail_height = 12.0;
        }
    });
}

/// A tight drop shadow under panels: depth without haze.
pub fn soft_shadow() -> egui::epaint::Shadow {
    egui::epaint::Shadow {
        offset: [0, 4],
        blur: 12,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 110),
    }
}

pub fn panel() -> Frame {
    Frame::new()
        .fill(FILL)
        .stroke(Stroke::new(1.0, EDGE_LIGHT))
        .corner_radius(RADIUS)
        .inner_margin(Margin::same(14))
        .shadow(soft_shadow())
}

pub fn bar() -> Frame {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(18, 30, 43, 244))
        .stroke(Stroke::new(1.0, EDGE_LIGHT))
        .inner_margin(Margin::symmetric(14, 8))
}

/// A heading in the pixel face. Headings are the one place text is upper case.
pub fn heading(text: &str, size: f32) -> egui::RichText {
    egui::RichText::new(text.to_uppercase())
        .font(pixel_face(size))
        .color(INK)
}

/// Smallest finger target, in points.
pub const TOUCH: f32 = 44.0;

/// The compact (phone) layout is on. Set once per frame by `State::ui`.
pub fn is_compact(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new("compact-layout")))
        .unwrap_or(false)
}

pub fn set_compact(ctx: &egui::Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("compact-layout"), on));
}

/// Narrow or short screens get the phone layout: width under 700 points (a phone in portrait)
/// or height under 520 (a phone in landscape). `URBAN_SPRAWL_COMPACT=1` or `0` forces it.
pub fn wants_compact(ctx: &egui::Context) -> bool {
    match std::env::var("URBAN_SPRAWL_COMPACT").as_deref() {
        Ok("1") => return true,
        Ok("0") => return false,
        _ => {}
    }
    let r = ctx.content_rect();
    r.width() < 700.0 || r.height() < 520.0
}

pub fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let compact = is_compact(ui.ctx());
    let w = if compact {
        ui.available_width().clamp(120.0, 220.0)
    } else {
        220.0
    };
    let h = if compact || cfg!(target_os = "ios") {
        48.0
    } else {
        40.0
    };
    ui.add(
        egui::Button::new(egui::RichText::new(text).font(ui_font(16.0)).color(INK))
            .min_size(egui::vec2(w, h)),
    )
}

/// A centered modal panel that scrolls instead of overflowing a small screen. The panel sits
/// in the middle of the screen at the height it had last frame.
pub fn centered(ctx: &egui::Context, id: &str, margin: i8, body: impl FnOnce(&mut egui::Ui)) {
    let r = ctx.content_rect();
    let max_h = (r.height() - 24.0).max(120.0);
    let max_w = (r.width() - 16.0).max(200.0);
    let key = egui::Id::new(("centered-h", id));
    let (prev, prev_content) = ctx
        .data(|d| d.get_temp::<(f32, f32)>(key))
        .unwrap_or((300.0, 300.0));
    let y = ((r.height() - prev) / 2.0).max(12.0);
    let inner_max = max_h - 2.0 * margin as f32 - 8.0;
    let mut content_h = prev_content;
    // above the dim layer, whatever order the layers were first made in
    let out = egui::Area::new(egui::Id::new(id))
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, r.top() + y))
        .show(ctx, |ui| {
            panel().inner_margin(Margin::same(margin)).show(ui, |ui| {
                let o = egui::ScrollArea::vertical()
                    .max_height(inner_max)
                    .max_width(max_w - 2.0 * margin as f32)
                    .min_scrolled_height(prev_content.min(inner_max))
                    .auto_shrink([true, true])
                    .show(ui, body);
                content_h = o.content_size.y;
            });
        });
    ctx.data_mut(|d| d.insert_temp(key, (out.response.rect.height(), content_h)));
}

/// Dim the town behind a modal screen.
pub fn dim(ctx: &egui::Context, alpha: u8) {
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("dim"),
    ))
    .rect_filled(
        ctx.content_rect(),
        0.0,
        Color32::from_rgba_unmultiplied(6, 12, 20, alpha),
    );
}
