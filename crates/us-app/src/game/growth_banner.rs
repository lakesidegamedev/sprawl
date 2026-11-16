//! The growth banner: when water, power, school seats or clinics are full (or nearly), say in
//! plain words what stopped, what to build and what it costs, with a button that opens the Build
//! window on that building. It goes away by itself once the building opens. Wanted key: none.

use super::Game;
use super::panels::{dock_top, lab_c};
use crate::theme::{self, ACCENT, BAD, INK, MUTED, TEXT, ui_font};
use egui::{Align2, Color32, Context, RichText, Stroke, vec2};
use us_sim::GrowthAlert;

/// What the banner remembers between frames: the sim is asked twice a second, not every frame.
#[derive(Default)]
pub struct GrowthUi {
    alerts: Vec<GrowthAlert>,
    asked: f64,
}

/// Seconds between asks.
const REFRESH_S: f64 = 0.5;
/// Alerts shown at once on a desktop; a phone has room for one.
const SHOWN: usize = 2;

impl Game {
    /// Draw the banner above the map (desktop) or above the bottom bar (phone).
    pub(super) fn growth_banner_ui(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        if now - self.ui.growth.asked > REFRESH_S || now < self.ui.growth.asked {
            self.ui.growth.alerts = self.sim.growth_alerts();
            self.ui.growth.asked = now;
        }
        if self.ui.growth.alerts.is_empty() {
            return;
        }
        let compact = theme::is_compact(ctx);
        let n = if compact { 1 } else { SHOWN };
        let shown: Vec<GrowthAlert> = self.ui.growth.alerts.iter().take(n).cloned().collect();
        let others: Vec<&str> = self
            .ui
            .growth
            .alerts
            .iter()
            .skip(n)
            .map(|a| a.capacity.label())
            .collect();
        let mut open_build = None;
        let anchor = if compact {
            let up = ctx.content_rect().bottom() - dock_top(ctx) + 54.0;
            (Align2::CENTER_BOTTOM, vec2(0.0, -up))
        } else {
            // above the key hints, clear of the news toasts at the top
            (Align2::CENTER_BOTTOM, vec2(0.0, -40.0))
        };
        egui::Area::new(egui::Id::new("growth-banner"))
            .anchor(anchor.0, anchor.1)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let width = if compact {
                    ctx.content_rect().width() - 24.0
                } else {
                    460.0
                };
                ui.set_max_width(width);
                if !others.is_empty() {
                    theme::panel()
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            lab_c(
                                ui,
                                format!("Also close to full: {}", others.join(", ")),
                                11.0,
                                MUTED,
                            );
                        });
                    ui.add_space(4.0);
                }
                for a in &shown {
                    let col = if a.blocked { BAD } else { ACCENT };
                    theme::panel()
                        .stroke(Stroke::new(1.5, col))
                        .inner_margin(egui::Margin::symmetric(10, 6))
                        .show(ui, |ui| {
                            ui.set_width(width - 22.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&a.headline)
                                        .font(ui_font(if compact { 14.0 } else { 15.0 }))
                                        .color(if a.blocked { BAD } else { INK }),
                                )
                                .wrap(),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&a.advice).font(ui_font(12.0)).color(TEXT),
                                )
                                .wrap(),
                            );
                            lab_c(
                                ui,
                                format!(
                                    "{} of {} in use",
                                    super::charts::group(a.demand),
                                    super::charts::group(a.supply)
                                ),
                                11.0,
                                MUTED,
                            );
                            if !a.under_construction {
                                let label = format!(
                                    "Build a {} ${}",
                                    a.build.label().to_lowercase(),
                                    us_sim::dollars(a.cost)
                                );
                                let b = ui.add(
                                    egui::Button::new(
                                        RichText::new(label)
                                            .font(ui_font(12.5))
                                            .color(if a.can_afford { theme::LINE } else { INK }),
                                    )
                                    .fill(if a.can_afford { ACCENT } else { theme::RAISED })
                                    .min_size(vec2(0.0, if compact { theme::TOUCH } else { 26.0 })),
                                );
                                if b.clicked() {
                                    open_build = Some(a.build);
                                }
                                if !a.can_afford {
                                    lab_c(
                                        ui,
                                        "Not enough in the treasury yet. Take a bond or wait.",
                                        11.0,
                                        Color32::from_rgb(240, 170, 110),
                                    );
                                }
                            }
                        });
                    ui.add_space(4.0);
                }
            });
        if let Some(kind) = open_build {
            self.open_build_for(kind);
        }
    }

    /// Open the Build window with this civic building picked, as if the player had chosen it.
    pub(super) fn open_build_for(&mut self, kind: us_sim::CivicKind) {
        self.ui.build_open = true;
        self.ui.tool = Some(kind);
        self.ui.road = None;
        self.ui.road_pts.clear();
        self.ui.paint = None;
    }
}
