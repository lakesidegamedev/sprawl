//! The studio look: Silkscreen pixel type, solid panels, hard shadows, no glow.

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

pub fn pixel(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("pixel".into()))
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "silk-bold".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/Silkscreen-Bold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "silk".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../assets/Silkscreen-Regular.ttf"
        ))),
    );
    fonts
        .families
        .insert(FontFamily::Name("pixel".into()), vec!["silk-bold".into()]);
    fonts
        .families
        .insert(FontFamily::Name("pixel-light".into()), vec!["silk".into()]);
    ctx.set_fonts(fonts);

    let mut v = egui::Visuals::dark();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(2.0, EDGE);
    v.window_corner_radius = 0.into();
    v.menu_corner_radius = 0.into();
    v.window_shadow = egui::epaint::Shadow {
        offset: [4, 4],
        blur: 0,
        spread: 0,
        color: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
    };
    v.popup_shadow = v.window_shadow;
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
        s.corner_radius = 0.into();
        s.bg_stroke = Stroke::new(2.0, EDGE);
        s.fg_stroke = Stroke::new(1.5, INK);
    }
    w.noninteractive.bg_fill = PANEL;
    w.noninteractive.weak_bg_fill = PANEL;
    w.inactive.bg_fill = RAISED;
    w.inactive.weak_bg_fill = RAISED;
    w.hovered.bg_fill = Color32::from_rgb(40, 64, 88);
    w.hovered.weak_bg_fill = Color32::from_rgb(40, 64, 88);
    w.hovered.bg_stroke = Stroke::new(2.0, ACCENT);
    w.active.bg_fill = Color32::from_rgb(120, 92, 36);
    w.active.weak_bg_fill = Color32::from_rgb(120, 92, 36);
    w.active.bg_stroke = Stroke::new(2.0, ACCENT);
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.slider_width = 200.0;
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

pub fn panel() -> Frame {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(2.0, EDGE))
        .inner_margin(Margin::same(12))
        .shadow(egui::epaint::Shadow {
            offset: [4, 4],
            blur: 0,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(0, 0, 0, 150),
        })
}

pub fn bar() -> Frame {
    Frame::new()
        .fill(PANEL_DARK)
        .stroke(Stroke::new(2.0, EDGE))
        .inner_margin(Margin::symmetric(14, 8))
}

pub fn heading(text: &str, size: f32) -> egui::RichText {
    egui::RichText::new(text).font(pixel(size)).color(INK)
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
        egui::Button::new(egui::RichText::new(text).font(pixel(16.0)).color(INK))
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
    let out = egui::Area::new(egui::Id::new(id))
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
