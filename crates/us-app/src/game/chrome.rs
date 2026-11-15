//! Desktop chrome in the Slate look: the slim top bar, the tool rail on the left, the inspector
//! on the right and the map view switcher.

use super::panels::{MapView, RoadMode, lab_c};
use super::{Game, SPEED_LABELS, Sel, money, split_row};
use crate::theme::{
    self, ACCENT, BAD, EDGE_LIGHT, GOOD, INK, MUTED, RAISED_FILL, TEXT, heading, icon, icon_font,
    ui_font,
};
use egui::{Align2, Color32, Context, RichText, Sense, Stroke, Ui, vec2};

/// Width of the tool rail.
pub const RAIL_W: f32 = 152.0;
/// Width of the inspector.
pub const INSPECTOR_W: f32 = 330.0;

/// Weather as the top bar shows it: icon, short word, colour.
fn weather(sim: &us_sim::Sim) -> (&'static str, &'static str, Color32) {
    let sky = Color32::from_rgb(120, 180, 230);
    let snow = Color32::from_rgb(200, 224, 245);
    let w = sim.wind();
    if sim.storm() {
        (icon::STORM, "Storm", Color32::from_rgb(240, 200, 90))
    } else if sim.snowfall() > 0.05 {
        (icon::SNOW, "Snow", snow)
    } else if sim.rain() > 0.05 {
        (icon::RAIN, "Rain", sky)
    } else if sim.dryness() > 0.8 {
        (icon::SUN, "Dry", Color32::from_rgb(240, 150, 80))
    } else if w.x.hypot(w.y) > 9.0 {
        (icon::WIND, "Windy", sky)
    } else if sim.snow_cover() > 0.3 {
        (icon::CLOUD, "Snow on the ground", snow)
    } else {
        (icon::SUN, "Clear", Color32::from_rgb(240, 200, 90))
    }
}

/// An icon followed by a figure, with a tooltip.
fn stat(ui: &mut Ui, glyph: &str, text: String, col: Color32, tip: &str) {
    let r = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;
            ui.label(RichText::new(glyph).font(icon_font(16.0)).color(MUTED));
            ui.label(RichText::new(text).font(ui_font(14.0)).color(col));
        })
        .response;
    r.on_hover_text(tip);
}

/// One row of the rail: an icon, a label and a state, with an eased hover.
fn rail_button(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    on: bool,
    badge: Option<String>,
    tip: &str,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 36.0), Sense::click());
    let hov = theme::ease_bool(ui.ctx(), (resp.id, "hover"), resp.hovered(), 0.12);
    let sel = theme::ease_bool(ui.ctx(), (resp.id, "on"), on, 0.15);
    let p = ui.painter();
    p.rect_filled(
        rect,
        6.0,
        Color32::from_rgba_unmultiplied(40, 64, 88, (hov * 200.0) as u8),
    );
    if sel > 0.01 {
        p.rect_filled(
            rect,
            6.0,
            Color32::from_rgba_unmultiplied(226, 176, 74, (sel * 38.0) as u8),
        );
        p.rect_stroke(
            rect,
            6.0,
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(226, 176, 74, (sel * 160.0) as u8),
            ),
            egui::StrokeKind::Inside,
        );
    }
    let col = if on { ACCENT } else { INK };
    p.text(
        rect.left_center() + vec2(18.0, 0.0),
        Align2::CENTER_CENTER,
        glyph,
        icon_font(19.0),
        col,
    );
    p.text(
        rect.left_center() + vec2(38.0, 0.0),
        Align2::LEFT_CENTER,
        label,
        ui_font(13.5),
        col,
    );
    if let Some(b) = badge {
        let g = p.layout_no_wrap(b, ui_font(11.0), theme::LINE);
        let pill = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 16.0, rect.center().y),
            vec2(g.size().x + 10.0, 18.0),
        );
        p.rect_filled(pill, 9.0, ACCENT);
        p.galley(pill.center() - g.size() / 2.0, g, theme::LINE);
    }
    resp.on_hover_text(tip)
}

/// A phone dock button: an icon over a short label, 44 points tall.
pub(super) fn dock_button(
    ui: &mut Ui,
    glyph: &str,
    label: &str,
    on: bool,
    size: egui::Vec2,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hov = theme::ease_bool(
        ui.ctx(),
        (resp.id, "hover"),
        resp.hovered() || resp.is_pointer_button_down_on(),
        0.1,
    );
    let sel = theme::ease_bool(ui.ctx(), (resp.id, "on"), on, 0.15);
    let p = ui.painter();
    p.rect_filled(
        rect,
        7.0,
        RAISED_FILL.lerp_to_gamma(Color32::from_rgb(40, 64, 88), hov),
    );
    if sel > 0.01 {
        p.rect_filled(
            rect,
            7.0,
            Color32::from_rgba_unmultiplied(226, 176, 74, (sel * 255.0) as u8),
        );
    }
    let col = if on { theme::LINE } else { INK };
    p.text(
        rect.center_top() + vec2(0.0, 4.0),
        Align2::CENTER_TOP,
        glyph,
        icon_font(18.0),
        col,
    );
    p.text(
        rect.center_bottom() - vec2(0.0, 3.0),
        Align2::CENTER_BOTTOM,
        label,
        ui_font(10.0),
        col,
    );
    resp
}

/// The menu button (three bars icon).
pub(super) fn menu_button(ui: &mut Ui, size: egui::Vec2) -> egui::Response {
    ui.add(egui::Button::new(theme::icon_text(icon::MENU, 20.0, INK)).min_size(size))
        .on_hover_text("Menu")
}

/// A speed button: pause icon for 0, "1x" "3x" "8x" otherwise. Gold when on.
pub(super) fn speed_button(ui: &mut Ui, i: usize, on: bool, size: egui::Vec2) -> egui::Response {
    let col = if on { theme::LINE } else { INK };
    let content = if i == 0 {
        theme::icon_text(icon::PAUSE, 17.0, col)
    } else {
        RichText::new(SPEED_LABELS[i])
            .font(ui_font(13.0))
            .color(col)
    };
    let tips = [
        "Pause (Space)",
        "Normal speed (1)",
        "Fast (2)",
        "Fastest (3)",
    ];
    ui.add(
        egui::Button::new(content)
            .fill(if on { ACCENT } else { RAISED_FILL })
            .min_size(size),
    )
    .on_hover_text(tips[i])
}

fn rail_gap(ui: &mut Ui) {
    ui.add_space(4.0);
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(
        r.x_range().shrink(6.0),
        r.top(),
        Stroke::new(1.0, EDGE_LIGHT),
    );
    ui.add_space(5.0);
}

/// A small caption over a block of the inspector.
pub(super) fn section(ui: &mut Ui, title: &str) {
    ui.add_space(8.0);
    ui.label(RichText::new(title).font(ui_font(11.5)).color(MUTED));
    let r = ui.available_rect_before_wrap();
    ui.painter()
        .hline(r.x_range(), r.top() + 1.0, Stroke::new(1.0, EDGE_LIGHT));
    ui.add_space(5.0);
}

impl Game {
    /// The desktop top bar. Returns the panel's response so the caller can place cards below.
    pub(super) fn desktop_bar(
        &mut self,
        ui: &mut Ui,
        who: &Option<String>,
        fps: f32,
    ) -> egui::InnerResponse<()> {
        let narrow = ui.ctx().content_rect().width() < 1280.0;
        egui::Panel::top("hud")
            .frame(theme::bar().inner_margin(egui::Margin::symmetric(14, 7)))
            .show(ui, |ui| {
                let left = move |ui: &mut Ui, g: &mut Game| {
                    ui.spacing_mut().item_spacing.x = 16.0;
                    if !g.placed {
                        ui.label(
                            RichText::new("Place the town hall")
                                .font(ui_font(14.0))
                                .color(INK),
                        );
                        return;
                    }
                    let ctx = ui.ctx().clone();
                    stat(
                        ui,
                        icon::CALENDAR,
                        g.sim.date().text().to_string(),
                        INK,
                        "Today's date",
                    );
                    let pop = theme::count_to(&ctx, "fig-pop", g.stats.pop as i64);
                    stat(ui, icon::PEOPLE, pop.to_string(), INK, "People in town");
                    let tre = theme::count_to(&ctx, "fig-tre", g.sim.treasury);
                    stat(
                        ui,
                        icon::BUDGET,
                        money(tre),
                        if g.sim.treasury < 0 { BAD } else { INK },
                        "Treasury",
                    );
                    let net_now = g.stats.income - g.stats.expense;
                    let net = theme::count_to(&ctx, "fig-net", net_now);
                    stat(
                        ui,
                        if net_now < 0 {
                            icon::TREND_DOWN
                        } else {
                            icon::TREND_UP
                        },
                        format!("{}{} a month", if net < 0 { "" } else { "+" }, money(net)),
                        if net_now < 0 { BAD } else { GOOD },
                        "Income minus spending, per month",
                    );
                    g.approval_chip(ui, false);
                    if !narrow {
                        g.sparks_ui(ui);
                    }
                    let (gl, word, col) = weather(&g.sim);
                    let r = ui
                        .horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 5.0;
                            ui.label(RichText::new(gl).font(icon_font(18.0)).color(col));
                            if !narrow {
                                ui.label(RichText::new(word).font(ui_font(13.0)).color(MUTED));
                            }
                        })
                        .response;
                    r.on_hover_text(format!("Weather: {}", word.to_lowercase()));
                };
                let who = who.clone();
                let right = move |ui: &mut Ui, g: &mut Game| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    if menu_button(ui, vec2(36.0, 30.0)).clicked() {
                        g.menu_open = true;
                    }
                    if g.placed {
                        for i in (0..4).rev() {
                            if speed_button(ui, i, g.speed == i, vec2(38.0, 30.0)).clicked() {
                                g.speed = i;
                            }
                        }
                    }
                    if !narrow {
                        if let Some(n) = &who {
                            ui.label(
                                RichText::new(format!("Signed in: {n}"))
                                    .font(ui_font(11.0))
                                    .color(MUTED),
                            );
                        }
                        ui.label(
                            RichText::new(format!("{fps:.0} fps"))
                                .font(ui_font(11.0))
                                .color(MUTED),
                        );
                    }
                };
                split_row(ui, 30.0, self, left, right);
            })
    }

    /// The tool rail on the left: every screen one click away, icon and label.
    pub(super) fn rail_ui(&mut self, ui: &mut Ui) {
        let unread = self.ui.events.len().saturating_sub(self.ui.news_seen);
        let view_label = self.ui.view.label();
        egui::Panel::left("rail")
            .exact_size(RAIL_W)
            .resizable(false)
            .frame(theme::bar().inner_margin(egui::Margin::symmetric(8, 8)))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let u = &mut self.ui;
                        let on = u.build_open && u.road.is_none();
                        if rail_button(
                            ui,
                            icon::HAMMER,
                            "Build",
                            on,
                            None,
                            "Place civic buildings (B)",
                        )
                        .clicked()
                        {
                            if on {
                                u.build_open = false;
                                u.tool = None;
                            } else {
                                u.build_open = true;
                                u.road = None;
                            }
                        }
                        let on = u.build_open && u.road.is_some();
                        if rail_button(
                            ui,
                            icon::ROAD,
                            "Roads",
                            on,
                            None,
                            "Draw avenues and bridges, widen streets (Y)",
                        )
                        .clicked()
                        {
                            u.build_open = true;
                            u.tool = None;
                            u.road = if u.road.is_some() {
                                None
                            } else {
                                Some(RoadMode::Avenue)
                            };
                            u.road_pts.clear();
                        }
                        let was = u.districts_open;
                        if rail_button(
                            ui,
                            icon::DISTRICTS,
                            "Districts",
                            was,
                            None,
                            "Group neighbourhoods (I)",
                        )
                        .clicked()
                        {
                            u.districts_open = !was;
                        }
                        if was && !u.districts_open {
                            u.paint = None;
                        }
                        let on = u.ordinances_open;
                        if rail_button(
                            ui,
                            icon::ORDINANCES,
                            "Ordinances",
                            on,
                            None,
                            "Town and district policies (H)",
                        )
                        .clicked()
                        {
                            u.ordinances_open = !on;
                        }
                        rail_gap(ui);
                        let on = u.levers_open;
                        if rail_button(
                            ui,
                            icon::TAXES,
                            "Taxes",
                            on,
                            None,
                            "Tax rates and demand (P)",
                        )
                        .clicked()
                        {
                            u.levers_open = !on;
                        }
                        let on = u.budget_open;
                        if rail_button(
                            ui,
                            icon::BUDGET,
                            "Budget",
                            on,
                            None,
                            "Department funding (K)",
                        )
                        .clicked()
                        {
                            u.budget_open = !on;
                        }
                        let on = u.loans_open;
                        if rail_button(ui, icon::BONDS, "Bonds", on, None, "Borrow and repay (O)")
                            .clicked()
                        {
                            u.loans_open = !on;
                        }
                        rail_gap(ui);
                        let on = u.stats_open;
                        if rail_button(ui, icon::GRAPHS, "Graphs", on, None, "Charts over time (G)")
                            .clicked()
                        {
                            u.stats_open = !on;
                        }
                        let on = u.news_open;
                        if rail_button(
                            ui,
                            icon::NEWS,
                            "News",
                            on,
                            (unread > 0).then(|| unread.to_string()),
                            "What happened in town (J)",
                        )
                        .clicked()
                        {
                            u.news_open = !on;
                        }
                        let on = u.rivals_open;
                        if rail_button(
                            ui,
                            icon::RIVALS,
                            "Rivals",
                            on,
                            None,
                            "The three towns you compete with (R)",
                        )
                        .clicked()
                        {
                            u.rivals_open = !on;
                        }
                        let voters = self.voters_open;
                        if rail_button(
                            ui,
                            icon::VOTERS,
                            "Voters",
                            voters,
                            None,
                            "What voters think (V)",
                        )
                        .clicked()
                        {
                            self.voters_open = !voters;
                        }
                        rail_gap(ui);
                        let open = self.ui.views_open;
                        let r = rail_button(
                            ui,
                            icon::MAP,
                            "Map views",
                            open || self.ui.view != MapView::Normal,
                            None,
                            "Colour the town by what you want to see (M)",
                        );
                        if r.clicked() {
                            self.ui.views_open = !open;
                        }
                        if self.ui.view != MapView::Normal {
                            ui.add_space(2.0);
                            ui.label(RichText::new(view_label).font(ui_font(11.5)).color(MUTED));
                        }
                        self.ui.views_rect_y = r.rect.top();
                    });
            });
        if !self.ui.build_open {
            self.ui.road = None;
        }
    }

    /// The map view switcher: a compact list beside the rail.
    pub(super) fn views_popover(&mut self, ctx: &Context) {
        if !self.ui.views_open {
            return;
        }
        let y = self.ui.views_rect_y;
        let out = egui::Area::new(egui::Id::new("views-popover"))
            .order(egui::Order::Foreground)
            .fixed_pos(egui::pos2(RAIL_W + 8.0, (y - 60.0).max(60.0)))
            .show(ctx, |ui| {
                theme::panel()
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.set_width(300.0);
                        lab_c(ui, "Map view", 12.0, MUTED);
                        ui.add_space(2.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
                            for v in MapView::ALL {
                                let on = self.ui.view == v;
                                let b = egui::Button::new(
                                    RichText::new(v.label()).font(ui_font(12.0)).color(if on {
                                        theme::LINE
                                    } else {
                                        INK
                                    }),
                                )
                                .fill(if on {
                                    ACCENT
                                } else {
                                    RAISED_FILL
                                });
                                if ui.add(b).clicked() {
                                    self.ui.view = v;
                                }
                            }
                        });
                    });
            });
        // a click anywhere else puts it away
        let clicked_out = ctx.input(|i| i.pointer.any_pressed())
            && ctx
                .pointer_interact_pos()
                .is_some_and(|p| !out.response.rect.contains(p) && p.x > RAIL_W);
        if clicked_out {
            self.ui.views_open = false;
        }
    }

    /// The inspector on the right: the selected building or the town hall, in sections.
    pub(super) fn inspector_ui(&mut self, ui: &mut Ui, sel: Sel) {
        let (title, status, built, reason) = match sel {
            Sel::Hall => (
                "Town hall".to_string(),
                "Your seat".to_string(),
                None,
                "You placed it at an old crossroads. The town grows outward from here.".to_string(),
            ),
            Sel::Lot(i) => {
                let y = self.sim.why(i);
                (y.title, y.status, y.built, y.reason)
            }
        };
        let mut open = true;
        let mut close = false;
        egui::Panel::right("inspector")
            .exact_size(INSPECTOR_W)
            .resizable(false)
            .frame(theme::bar().inner_margin(egui::Margin::same(14)))
            .show_collapsible(ui, &mut open, |ui| {
                ui.horizontal(|ui| {
                    ui.label(heading(&title, 17.0));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(icon::CLOSE).font(icon_font(16.0)).color(INK),
                                )
                                .min_size(vec2(30.0, 28.0)),
                            )
                            .on_hover_text("Close")
                            .clicked()
                        {
                            close = true;
                        }
                    });
                });
                ui.label(RichText::new(status).font(ui_font(12.5)).color(ACCENT));
                if let Some(b) = built {
                    ui.label(RichText::new(b).font(ui_font(12.5)).color(MUTED));
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        section(ui, "Why it is here");
                        ui.add(
                            egui::Label::new(RichText::new(reason).size(14.5).color(TEXT)).wrap(),
                        );
                        if let Sel::Lot(i) = sel {
                            self.why_extra(ui, i);
                        }
                    });
            });
        if close || !open {
            self.sel = None;
        }
    }

    /// Distance from the right edge for floating cards: clear of the inspector when it is open.
    pub(super) fn right_inset(&self) -> f32 {
        if self.sel.is_some() && !self.compact {
            INSPECTOR_W + 14.0
        } else {
            14.0
        }
    }
}
