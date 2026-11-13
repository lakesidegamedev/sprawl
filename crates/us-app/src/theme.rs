//! The studio look: Silkscreen pixel type, solid panels, hard shadows, no glow.

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

pub fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).font(pixel(16.0)).color(INK)).min_size(
            egui::vec2(220.0, if cfg!(target_os = "ios") { 48.0 } else { 40.0 }),
        ),
    )
}
