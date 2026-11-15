//! The Districts and Ordinances windows and the paint tool. Everything here reads and changes
//! the simulation through its public API.

use super::panels::{lab, lab_c, window, wrapped};
use super::{Game, money};
use crate::theme::{self, ACCENT, BAD, EDGE, GOOD, INK, LINE, RAISED, TEXT, ui_font};
use egui::{Color32, Context, Pos2, RichText, Stroke, Ui, vec2};
use us_gen::Pt;
use us_sim::{MAX_DISTRICTS, OrdScope, Ordinance};

const MUTED: Color32 = Color32::from_rgb(170, 176, 196);

fn swatch(ui: &mut Ui, rgb: [u8; 3]) {
    let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(r, 3.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    ui.painter()
        .rect_stroke(r, 3.0, Stroke::new(1.0, LINE), egui::StrokeKind::Inside);
}

/// A toggle button that is big enough to touch on a phone.
fn toggle(ui: &mut Ui, text: &str, on: bool, touch: bool) -> egui::Response {
    let b = egui::Button::new(
        RichText::new(text.to_string())
            .font(ui_font(12.0))
            .color(if on { LINE } else { INK }),
    )
    .fill(if on { ACCENT } else { RAISED })
    .min_size(vec2(0.0, if touch { theme::TOUCH } else { 22.0 }));
    ui.add(b)
}

impl Game {
    /// Keep the district numbers fresh while the window is open (they scan every lot).
    pub(super) fn refresh_district_stats(&mut self, dt: f32) {
        self.ui.dist_stats_age += dt;
        if !self.ui.districts_open || self.ui.dist_stats_age < 1.0 {
            return;
        }
        self.ui.dist_stats_age = 0.0;
        let n = self.sim.districts().len();
        self.ui.dist_stats = (0..n).map(|k| self.sim.district_stats(k)).collect();
    }

    /// Paint tool: a click adds the neighbourhood under it to the district, or takes it out.
    pub(super) fn paint_at(&mut self, p: Pt) {
        let Some(d) = self.ui.paint else { return };
        let mut best: Option<(f64, usize)> = None;
        for l in self.sim.lots() {
            if l.addition == usize::MAX || l.civic.is_some() {
                continue;
            }
            let dist = l.pos.dist(p);
            if dist < 60.0 && best.is_none_or(|b| dist < b.0) {
                best = Some((dist, l.addition));
            }
        }
        let Some((_, a)) = best else { return };
        let name = self.sim.districts().get(d).map(|x| x.name.clone());
        let Some(name) = name else {
            self.ui.paint = None;
            return;
        };
        let was = self.sim.district_of_addition(a);
        let now = self.sim.toggle_addition(d, a);
        let text = if now {
            format!("Added to {}.", name)
        } else if was == Some(d) {
            format!("Taken out of {}.", name)
        } else {
            "Farmland cannot join a district.".to_string()
        };
        self.banner = Some((text, 3.0));
        self.ui.dist_stats_age = 9.0;
    }

    /// Names of the districts with their colours, for the map legend.
    pub(super) fn district_legend(&self, ui: &mut Ui) {
        if self.sim.districts().is_empty() {
            lab_c(
                ui,
                "No districts yet. Open Districts to make one.",
                12.0,
                MUTED,
            );
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(520.0);
            for d in self.sim.districts() {
                swatch(ui, d.rgb());
                lab(ui, &d.name, 10.0);
            }
        });
    }

    pub(super) fn districts_ui(&mut self, ctx: &Context, _screen_w: f32) {
        let touch = theme::is_compact(ctx);
        let pos = Pos2::new(40.0, 110.0);
        let closed = window(ctx, "districts", "Districts", 440.0, pos, |ui| {
            let n = self.sim.districts().len();
            ui.horizontal(|ui| {
                if n < MAX_DISTRICTS && toggle(ui, "New district", true, touch).clicked() {
                    self.ui.dist_sel = self.sim.create_district();
                }
                lab(ui, format!("{n} of {MAX_DISTRICTS}"), 12.0);
            });
            wrapped(
                ui,
                "Pick a district, press Paint, then click a neighbourhood on the map to add it or take it out. A neighbourhood is in one district at a time. Farmland cannot join.",
                11.0,
                MUTED,
            );
            ui.add_space(4.0);
            let mut delete: Option<usize> = None;
            for k in 0..n {
                let (name, rgb, count) = {
                    let d = &self.sim.districts()[k];
                    (d.name.clone(), d.rgb(), d.additions.len())
                };
                let sel = self.ui.dist_sel == Some(k);
                egui::Frame::new()
                    .stroke(Stroke::new(if sel { 2.0 } else { 1.0 }, if sel { ACCENT } else { EDGE }))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            swatch(ui, rgb);
                            let id = egui::Id::new(("district-name", k));
                            let mut buf: String = ui
                                .data(|d| d.get_temp::<String>(id))
                                .unwrap_or_else(|| name.clone());
                            let r = ui.add(
                                egui::TextEdit::singleline(&mut buf)
                                    .desired_width(150.0)
                                    .char_limit(24),
                            );
                            if r.changed() {
                                self.sim.rename_district(k, &buf);
                                ui.data_mut(|d| d.insert_temp(id, buf.clone()));
                            }
                            if r.lost_focus() {
                                ui.data_mut(|d| d.remove_temp::<String>(id));
                            }
                            if r.clicked() || r.gained_focus() {
                                self.ui.dist_sel = Some(k);
                            }
                            let painting = self.ui.paint == Some(k);
                            if toggle(ui, "Paint", painting, touch)
                                .on_hover_text("Click neighbourhoods on the map to add or remove them")
                                .clicked()
                            {
                                self.ui.dist_sel = Some(k);
                                if painting {
                                    self.ui.paint = None;
                                } else {
                                    self.ui.paint = Some(k);
                                    self.ui.view = super::panels::MapView::District;
                                    self.banner = Some((
                                        format!(
                                            "Painting {}: click a neighbourhood to add or remove it.",
                                            name
                                        ),
                                        6.0,
                                    ));
                                }
                            }
                            if toggle(ui, "Delete", false, touch).clicked() {
                                delete = Some(k);
                            }
                        });
                        match self.ui.dist_stats.get(k).and_then(|s| s.as_ref()) {
                            Some(s) if count > 0 => {
                                lab(
                                    ui,
                                    format!(
                                        "{count} neighbourhoods  {} people  {} jobs  {} buildings",
                                        s.people, s.jobs, s.buildings
                                    ),
                                    11.0,
                                );
                                lab(
                                    ui,
                                    format!(
                                        "Land value {:.0}%  building level {:.1}",
                                        s.avg_land_value * 100.0,
                                        s.avg_level
                                    ),
                                    11.0,
                                );
                                lab(
                                    ui,
                                    format!(
                                        "Tax {} a month  ordinances {} a month",
                                        money(s.tax_income),
                                        money(s.ordinance_cost)
                                    ),
                                    11.0,
                                );
                                wrapped(
                                    ui,
                                    format!("Biggest problem: {}", s.problem),
                                    11.0,
                                    if s.problem.starts_with("No big") || s.problem.starts_with("Nothing") {
                                        GOOD
                                    } else {
                                        BAD
                                    },
                                );
                            }
                            _ => {
                                lab_c(ui, "No neighbourhoods yet. Press Paint.", 11.0, MUTED);
                            }
                        }
                    });
                ui.add_space(3.0);
            }
            if let Some(k) = delete {
                self.sim.delete_district(k);
                self.ui.paint = None;
                self.ui.dist_sel = None;
                self.ui.dist_stats_age = 9.0;
            }
        });
        if closed {
            self.ui.districts_open = false;
            self.ui.paint = None;
        }
    }

    pub(super) fn ordinances_ui(&mut self, ctx: &Context, _screen_w: f32) {
        let touch = theme::is_compact(ctx);
        let pos = Pos2::new(360.0, 110.0);
        let closed = window(ctx, "ordinances", "Ordinances", 540.0, pos, |ui| {
            wrapped(
                ui,
                "Policies you switch on for the whole town or for one district. Fees come out of the treasury every month. Switching one off stops its effect at once.",
                11.0,
                MUTED,
            );
            ui.add_space(4.0);
            let districts: Vec<(String, [u8; 3])> = self
                .sim
                .districts()
                .iter()
                .map(|d| (d.name.clone(), d.rgb()))
                .collect();
            for o in Ordinance::ALL {
                egui::Frame::new()
                    .stroke(Stroke::new(1.0, EDGE))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            lab_c(ui, o.label(), 13.0, ACCENT);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| lab_c(ui, o.scope().text(), 11.0, MUTED),
                            );
                        });
                        ui.add(
                            egui::Label::new(RichText::new(o.effect()).size(14.0).color(TEXT))
                                .wrap(),
                        );
                        let est = self.sim.ordinance_estimate(o, None);
                        let (text, col) = if o == Ordinance::BusinessBreak {
                            (format!("Loses about {} a month in tax", money(-est)), BAD)
                        } else if est < 0 {
                            (
                                format!("Costs about {} a month town wide", money(-est)),
                                BAD,
                            )
                        } else {
                            ("No monthly fee".to_string(), GOOD)
                        };
                        lab_c(ui, text, 11.0, col);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                            if o.scope() != OrdScope::District {
                                let on = self.sim.ordinance_town(o);
                                if toggle(ui, if on { "Town: on" } else { "Town: off" }, on, touch)
                                    .clicked()
                                {
                                    self.sim.set_ordinance_town(o, !on);
                                }
                            }
                            if o.scope() != OrdScope::Town {
                                if districts.is_empty() {
                                    lab_c(ui, "Make a district to use this one.", 11.0, MUTED);
                                }
                                for (k, (name, rgb)) in districts.iter().enumerate() {
                                    let on = self.sim.ordinance_in_district(k, o);
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 3.0;
                                        swatch(ui, *rgb);
                                        let est = self.sim.ordinance_estimate(o, Some(k));
                                        let tip = if est < 0 {
                                            format!("About {} a month here", money(-est))
                                        } else {
                                            "No fee here".to_string()
                                        };
                                        if toggle(ui, name, on, touch).on_hover_text(tip).clicked()
                                        {
                                            self.sim.set_ordinance_district(k, o, !on);
                                        }
                                    });
                                }
                            }
                        });
                    });
                ui.add_space(3.0);
            }
            let cost = self.sim.ordinance_costs();
            lab_c(
                ui,
                format!("Ordinances cost {} a month now", money(cost)),
                12.0,
                if cost > 0 { BAD } else { GOOD },
            );
        });
        if closed {
            self.ui.ordinances_open = false;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_and_window_colours_agree() {
        let pal = us_render::OverlayKind::District.palette();
        assert_eq!(pal.len(), us_sim::MAX_DISTRICTS + 1);
        for (k, c) in us_sim::DISTRICT_COLOURS.iter().enumerate() {
            assert_eq!(&pal[k], c, "district colour {k}");
        }
    }
}
