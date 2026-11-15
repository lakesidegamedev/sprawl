//! The Tram window: pick transit stops in order, see what the line costs, build it, and close
//! lines that are no use. Everything here reads and changes the simulation through its public
//! API. Wanted key: M (not bound; the lead assigns keys).

use super::panels::{lab, lab_c, window, wrapped};
use super::{Game, money};
use crate::theme::{self, ACCENT, BAD, EDGE, GOOD, INK, LINE, RAISED, TEXT, pixel};
use egui::{Color32, Context, Pos2, RichText, Stroke, Ui, vec2};
use us_sim::{TramError, TramState};

const MUTED: Color32 = Color32::from_rgb(170, 176, 196);

/// What the Tram window remembers.
#[derive(Default)]
pub struct TramUi {
    pub open: bool,
    /// Lot indices of the stops picked so far, in order.
    picked: Vec<usize>,
}

fn button(ui: &mut Ui, text: &str, on: bool, touch: bool) -> egui::Response {
    ui.add(
        egui::Button::new(
            RichText::new(text.to_uppercase())
                .font(pixel(12.0))
                .color(if on { LINE } else { INK }),
        )
        .fill(if on { ACCENT } else { RAISED })
        .min_size(vec2(0.0, if touch { theme::TOUCH } else { 22.0 })),
    )
}

impl Game {
    /// Smoke hook `trams`: two transit stops, a line between them, built and open, with the
    /// window and the camera on it.
    pub(super) fn smoke_trams(&mut self) {
        use us_gen::Pt;
        use us_sim::{CivicKind, Phase};
        self.sim.unlock_all();
        self.sim.treasury += 5_000_000;
        let mut spots: Vec<Pt> = self
            .sim
            .lots()
            .iter()
            .filter(|l| l.phase == Phase::Vacant)
            .map(|l| l.pos)
            .filter(|&p| self.sim.check_civic(CivicKind::Transit, p).is_ok())
            .collect();
        let hall = self.sim.hall;
        spots.sort_by(|a, b| a.dist(hall).total_cmp(&b.dist(hall)));
        let Some(&first) = spots.first() else { return };
        let _ = self.sim.place_civic(CivicKind::Transit, first);
        let second = spots
            .iter()
            .copied()
            .find(|p| p.dist(first) > 300.0 && p.dist(first) < 700.0);
        if let Some(p) = second {
            let _ = self.sim.place_civic(CivicKind::Transit, p);
        }
        for _ in 0..90 {
            self.sim.step_day();
        }
        let stops: Vec<usize> = self.sim.tram_stops().iter().map(|s| s.0).collect();
        if self.sim.build_tram(&stops).is_ok() {
            for _ in 0..140 {
                self.sim.step_day();
            }
            if let Some(l) = self.sim.tram_lines().first() {
                let m = l.points[l.points.len() / 2];
                self.cam
                    .look_at(glam::Vec3::new(m.x as f32, m.y as f32, 0.0), 260.0);
            }
        }
        self.ui.trams.open = true;
    }

    pub(super) fn trams_ui(&mut self, ctx: &Context, _screen_w: f32) {
        let touch = theme::is_compact(ctx);
        let pos = Pos2::new(60.0, 120.0);
        let closed = window(ctx, "trams", "Trams", 440.0, pos, |ui| {
            let stops = self.sim.tram_stops();
            // a stop that is no longer there cannot stay picked
            self.ui
                .trams
                .picked
                .retain(|p| stops.iter().any(|s| s.0 == *p));
            wrapped(
                ui,
                "A tram line joins transit stops along arterials and collectors. Pick the stops in the order the line should run. Trams take trips off the streets beside the line and lift land value near the stops. Each line has a monthly upkeep.",
                11.0,
                MUTED,
            );
            ui.add_space(4.0);
            if stops.len() < 2 {
                wrapped(
                    ui,
                    "You need two finished transit stops. Build them from the Build window.",
                    12.0,
                    TEXT,
                );
            } else {
                lab(ui, "Stops", 12.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                    for (n, &(lot, at)) in stops.iter().enumerate() {
                        let place = self.ui.trams.picked.iter().position(|&p| p == lot);
                        let text = match place {
                            Some(k) => format!("{}: stop {}", k + 1, n + 1),
                            None => format!("Stop {}", n + 1),
                        };
                        let tip = format!("{:.0} m from the town hall", at.dist(self.sim.hall));
                        if button(ui, &text, place.is_some(), touch)
                            .on_hover_text(tip)
                            .clicked()
                        {
                            match place {
                                Some(_) => self.ui.trams.picked.retain(|&p| p != lot),
                                None => self.ui.trams.picked.push(lot),
                            }
                        }
                    }
                });
                ui.add_space(4.0);
                let picked = self.ui.trams.picked.clone();
                if picked.len() < 2 {
                    wrapped(ui, "Pick at least two stops.", 12.0, MUTED);
                } else {
                    match self.sim.check_tram(&picked) {
                        Ok(plan) => {
                            lab(
                                ui,
                                format!(
                                    "{:.0} m, {} stops, {} days to build",
                                    plan.length,
                                    plan.stops.len(),
                                    plan.days
                                ),
                                12.0,
                            );
                            lab(ui, format!("Cost ${}", money(plan.cost)), 12.0);
                            lab(ui, format!("Upkeep ${} a month", money(plan.upkeep)), 12.0);
                            if button(ui, "Build line", true, touch).clicked() {
                                match self.sim.build_tram(&picked) {
                                    Ok(i) => {
                                        self.banner = Some((
                                            format!("WORK STARTED ON TRAM LINE {}.", i + 1),
                                            3.0,
                                        ));
                                        self.ui.trams.picked.clear();
                                    }
                                    Err(e) => self.banner = Some((e.text().to_uppercase(), 3.0)),
                                }
                            }
                        }
                        Err(e) => {
                            let col = if matches!(e, TramError::TooPoor { .. }) {
                                ACCENT
                            } else {
                                BAD
                            };
                            wrapped(ui, e.text(), 12.0, col);
                        }
                    }
                }
                if !self.ui.trams.picked.is_empty() && button(ui, "Clear", false, touch).clicked() {
                    self.ui.trams.picked.clear();
                }
            }
            ui.add_space(8.0);
            let lines = self.sim.tram_lines();
            lab(
                ui,
                format!("Lines ({} of {})", lines.len(), us_sim::MAX_TRAM_LINES),
                12.0,
            );
            if lines.is_empty() {
                wrapped(ui, "No lines yet.", 12.0, MUTED);
            }
            let rate = self.sim.tram_rate();
            let mut close: Option<usize> = None;
            for (k, l) in lines.iter().enumerate() {
                egui::Frame::new()
                    .stroke(Stroke::new(1.0, EDGE))
                    .inner_margin(egui::Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            lab(ui, &l.name, 12.0);
                            let (text, col) = match l.state {
                                TramState::Track => {
                                    (format!("laying track {:.0}%", l.progress * 100.0), ACCENT)
                                }
                                TramState::Wires => {
                                    (format!("hanging wire {:.0}%", l.progress * 100.0), ACCENT)
                                }
                                TramState::Open => ("open".to_string(), GOOD),
                            };
                            lab_c(ui, text, 12.0, col);
                        });
                        lab_c(
                            ui,
                            format!(
                                "{:.0} m, {} stops, upkeep ${} a month",
                                l.length,
                                l.stops.len(),
                                money(l.upkeep)
                            ),
                            12.0,
                            MUTED,
                        );
                        if l.state == TramState::Open {
                            lab_c(ui, format!("{rate:.1} trams an hour each way"), 12.0, MUTED);
                        }
                        if button(ui, "Close line", false, touch)
                            .on_hover_text("Nothing is paid back")
                            .clicked()
                        {
                            close = Some(k);
                        }
                    });
            }
            if let Some(k) = close {
                let _ = self.sim.remove_tram(k);
            }
        });
        if closed {
            self.ui.trams.open = false;
            self.ui.trams.picked.clear();
        }
    }
}
