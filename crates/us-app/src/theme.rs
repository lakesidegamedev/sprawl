//! The studio look: Silkscreen pixel type, solid panels, hard shadows, no glow.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, Frame, Margin, Stroke};

pub const INK: Color32 = Color32::from_rgb(28, 30, 38);
pub const PANEL: Color32 = Color32::from_rgb(236, 230, 214);
pub const PANEL_DARK: Color32 = Color32::from_rgb(52, 56, 72);
pub const ACCENT: Color32 = Color32::from_rgb(226, 150, 52);
pub const GOOD: Color32 = Color32::from_rgb(52, 130, 76);
pub const BAD: Color32 = Color32::from_rgb(190, 58, 52);
pub const TEXT: Color32 = Color32::from_rgb(34, 36, 46);

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

    let mut v = egui::Visuals::light();
    v.panel_fill = PANEL;
    v.window_fill = PANEL;
    v.window_stroke = Stroke::new(2.0, INK);
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
    v.selection.stroke = Stroke::new(1.0, INK);
    let w = &mut v.widgets;
    for s in [
        &mut w.noninteractive,
        &mut w.inactive,
        &mut w.hovered,
        &mut w.active,
        &mut w.open,
    ] {
        s.corner_radius = 0.into();
        s.bg_stroke = Stroke::new(2.0, INK);
        s.fg_stroke = Stroke::new(1.5, INK);
    }
    w.inactive.bg_fill = Color32::from_rgb(250, 246, 234);
    w.inactive.weak_bg_fill = Color32::from_rgb(250, 246, 234);
    w.hovered.bg_fill = Color32::from_rgb(255, 214, 140);
    w.hovered.weak_bg_fill = Color32::from_rgb(255, 214, 140);
    w.active.bg_fill = ACCENT;
    w.active.weak_bg_fill = ACCENT;
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(12.0, 6.0);
        s.spacing.slider_width = 200.0;
    });
}

pub fn panel() -> Frame {
    Frame::new()
        .fill(PANEL)
        .stroke(Stroke::new(2.0, INK))
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
        .stroke(Stroke::new(2.0, INK))
        .inner_margin(Margin::symmetric(14, 8))
}

pub fn heading(text: &str, size: f32) -> egui::RichText {
    egui::RichText::new(text).font(pixel(size)).color(INK)
}

pub fn button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    ui.add(
        egui::Button::new(egui::RichText::new(text).font(pixel(16.0)).color(INK))
            .min_size(egui::vec2(220.0, 40.0)),
    )
}
