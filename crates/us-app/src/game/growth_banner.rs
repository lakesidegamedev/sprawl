//! The growth banner: when water, power, school seats or clinics are full (or nearly), say in
//! plain words what stopped, what to build and what it costs, with a button that opens the Build
//! window on that building. It goes away by itself once the building opens. Wanted key: none.

use super::Game;
use super::panels::{dock_top, lab_c};
use crate::theme::{self, ACCENT, BAD, INK, MUTED, TEXT, ui_font};
use egui::{Align2, Context, RichText, Stroke, Ui, vec2};
use us_sim::{Capacity, CapacityStatus, GrowthAlert, Sim};

/// What the banner remembers between frames: the sim is asked twice a second, not every frame.
#[derive(Default)]
pub struct GrowthUi {
    alerts: Vec<GrowthAlert>,
    pub(super) capacities: Option<[CapacityStatus; 4]>,
    pub(super) stalls: Vec<Capacity>,
    asked: f64,
    banner_height: f32,
}

impl GrowthUi {
    pub(super) fn refresh(&mut self, sim: &Sim, now: f64) {
        if self.capacities.is_none() || now - self.asked > REFRESH_S || now < self.asked {
            self.alerts = sim.growth_alerts();
            self.capacities = Some(sim.committed_capacities());
            self.stalls = sim.growth_stalls();
            self.asked = now;
        }
    }

    /// Height of the last banner drawn above the phone dock.
    pub fn height(&self) -> f32 {
        self.banner_height
    }
}

/// Seconds between asks.
const REFRESH_S: f64 = 0.5;

impl Game {
    /// The compact banner sits above the dock, away from the controls at the top.
    pub(super) fn growth_banner_ui(&mut self, ctx: &Context) {
        self.ui.growth.banner_height = 0.0;
        if !self.map_input_available() || self.any_sheet() || self.ui.growth.alerts.is_empty() {
            return;
        }
        let screen = ctx.content_rect();
        let up = screen.bottom() - dock_top(ctx) + 8.0;
        let area = egui::Area::new(egui::Id::new("growth-banner"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -up))
            .order(egui::Order::Middle)
            .show(ctx, |ui| self.growth_panel(ui, screen.width() - 16.0));
        self.ui.growth.banner_height = area.response.rect.height() + 8.0;
    }

    /// Show the most urgent capacity limit and the next action.
    pub(super) fn growth_panel(&mut self, ui: &mut Ui, width: f32) {
        let alerts = &self.ui.growth.alerts;
        if alerts.is_empty() {
            return;
        }
        let compact = self.compact;
        // blocked first; the sim lists them in order of urgency
        let main = alerts
            .iter()
            .find(|a| a.blocked)
            .unwrap_or(&alerts[0])
            .clone();
        let also: Vec<String> = alerts
            .iter()
            .filter(|a| a.capacity != main.capacity && (a.blocked || !main.blocked))
            .map(|a| a.capacity.label().to_lowercase())
            .collect();
        let blocked = main.blocked;
        let mut open_build = None;
        let mut open_bonds = false;
        let col = if blocked { BAD } else { ACCENT };
        theme::panel()
            .stroke(Stroke::new(1.5, col))
            .inner_margin(egui::Margin::symmetric(10, 5))
            .show(ui, |ui| {
                ui.set_max_width(width - 22.0);
                ui.horizontal_wrapped(|ui| {
                    let text = if blocked && main.under_construction {
                        format!("{}. {}", main.headline, main.advice)
                    } else {
                        main.headline.clone()
                    };
                    ui.add(
                        egui::Label::new(
                            RichText::new(text)
                                .font(ui_font(if blocked { 13.0 } else { 12.0 }))
                                .color(if blocked { INK } else { TEXT }),
                        )
                        .wrap(),
                    );
                    if main.under_construction {
                        if !blocked {
                            lab_c(ui, "One is being built.", 11.0, MUTED);
                        }
                    } else if !main.can_afford {
                        if ui
                            .add(
                                egui::Button::new("Review bonds")
                                    .min_size(vec2(0.0, if compact { theme::TOUCH } else { 22.0 })),
                            )
                            .clicked()
                        {
                            open_bonds = true;
                        }
                    } else {
                        let label = format!(
                            "Place {} · ${}",
                            main.build.label().to_lowercase(),
                            us_sim::dollars(main.cost)
                        );
                        let b = ui.add(
                            egui::Button::new(
                                RichText::new(label).font(ui_font(12.0)).color(theme::LINE),
                            )
                            .fill(ACCENT)
                            .min_size(vec2(0.0, if compact { theme::TOUCH } else { 22.0 })),
                        );
                        if b.clicked() {
                            open_build = Some(main.build);
                        }
                    }
                });
                let mut extra = Vec::new();
                if blocked {
                    extra.push(format!(
                        "{} of {} committed.",
                        super::charts::group(main.demand),
                        super::charts::group(main.supply)
                    ));
                }
                if !main.can_afford && !main.under_construction {
                    let short = main.cost.saturating_sub(self.sim.treasury);
                    extra.push(format!(
                        "You need ${} more to build it.",
                        us_sim::dollars(short)
                    ));
                }
                if !also.is_empty() {
                    extra.push(format!(
                        "{}: {}.",
                        if blocked {
                            "Also full"
                        } else {
                            "Also near full"
                        },
                        also.join(", ")
                    ));
                }
                if !extra.is_empty() {
                    lab_c(ui, extra.join(" "), 11.0, MUTED);
                }
            });
        if let Some(kind) = open_build {
            self.open_build_for(kind);
        } else if open_bonds {
            if compact {
                self.close_sheets();
            }
            self.ui.loans_open = true;
        }
    }

    /// Pick a civic building and make the placement controls available.
    pub(super) fn open_build_for(&mut self, kind: us_sim::CivicKind) {
        if self.compact {
            self.close_sheets();
        } else {
            self.ui.build_open = true;
        }
        self.ui.tool = Some(kind);
        self.ui.civic_site = None;
        self.ui.road = None;
        self.ui.road_pts.clear();
        self.ui.paint = None;
    }
}
