//! The growth banner: when water, power, school seats or clinics are full (or nearly), say in
//! plain words what stopped, what to build and what it costs, with a button that opens the Build
//! window on that building. It goes away by itself once the building opens. Wanted key: none.

use super::Game;
use super::panels::{dock_top, lab_c};
use crate::theme::{self, ACCENT, BAD, INK, MUTED, TEXT, ui_font};
use egui::{Align2, Context, RichText, Stroke, vec2};
use us_sim::GrowthAlert;

/// What the banner remembers between frames: the sim is asked twice a second, not every frame.
#[derive(Default)]
pub struct GrowthUi {
    alerts: Vec<GrowthAlert>,
    asked: f64,
    /// Where the top bar ends, set by the game each frame.
    pub top_y: f32,
}

impl GrowthUi {
    /// About how tall the banner is on a phone, so the goals chip can sit above it.
    pub fn height(&self) -> f32 {
        if self.alerts.iter().any(|a| a.blocked) {
            132.0
        } else if self.alerts.is_empty() {
            0.0
        } else {
            84.0
        }
    }
}

/// Seconds between asks.
const REFRESH_S: f64 = 0.5;

impl Game {
    /// One compact banner: top centre under the top bar on a desktop, above the bottom bar on a
    /// phone. It shows the most urgent block with its button; other blocks are one short line
    /// inside it. With nothing blocked, a slim line warns about the capacity nearest to full.
    pub(super) fn growth_banner_ui(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        if now - self.ui.growth.asked > REFRESH_S || now < self.ui.growth.asked {
            self.ui.growth.alerts = self.sim.growth_alerts();
            self.ui.growth.asked = now;
        }
        let alerts = &self.ui.growth.alerts;
        if alerts.is_empty() {
            return;
        }
        let compact = theme::is_compact(ctx);
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
        let width = if compact {
            ctx.content_rect().width() - 16.0
        } else {
            460.0
        };
        let anchor = if compact {
            let up = ctx.content_rect().bottom() - dock_top(ctx) + 8.0;
            (Align2::CENTER_BOTTOM, vec2(0.0, -up))
        } else {
            // under the top bar, and under the placement banner when one is showing
            let y = self.ui.growth.top_y + if self.banner.is_some() { 48.0 } else { 0.0 };
            (Align2::CENTER_TOP, vec2(0.0, y))
        };
        egui::Area::new(egui::Id::new("growth-banner"))
            .anchor(anchor.0, anchor.1)
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                ui.set_max_width(width);
                let col = if blocked { BAD } else { ACCENT };
                theme::panel()
                    .stroke(Stroke::new(1.5, col))
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .show(ui, |ui| {
                        ui.set_max_width(width - 22.0);
                        ui.horizontal_wrapped(|ui| {
                            let text = if blocked {
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
                            if !main.under_construction {
                                let label = format!(
                                    "Build {} ${}",
                                    main.build.label().to_lowercase(),
                                    us_sim::dollars(main.cost)
                                );
                                let b = ui.add(
                                    egui::Button::new(
                                        RichText::new(label)
                                            .font(ui_font(12.0))
                                            .color(if main.can_afford { theme::LINE } else { INK }),
                                    )
                                    .fill(if main.can_afford {
                                        ACCENT
                                    } else {
                                        theme::RAISED
                                    })
                                    .min_size(vec2(0.0, if compact { theme::TOUCH } else { 22.0 })),
                                );
                                if b.clicked() {
                                    open_build = Some(main.build);
                                }
                            } else {
                                lab_c(ui, "One is being built.", 11.0, MUTED);
                            }
                        });
                        let mut extra = Vec::new();
                        if blocked {
                            extra.push(format!(
                                "{} of {} in use.",
                                super::charts::group(main.demand),
                                super::charts::group(main.supply)
                            ));
                            if !main.can_afford && !main.under_construction {
                                extra.push("Not enough money yet: take a bond or wait.".into());
                            }
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
