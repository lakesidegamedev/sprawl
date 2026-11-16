//! The first-year goal card: four to six small goals that teach the loop, each ticked off by
//! the town itself. A card on desktop, a chip and a sheet on the phone. Wanted key: G (the lead
//! assigns keys).

use super::Game;
use super::panels::{dock_top, lab, lab_c, window, wrapped};
use crate::theme::{self, ACCENT, GOOD, INK, MUTED, TEXT, ui_font};
use egui::{Align2, Context, RichText, Stroke, Ui, vec2};
use us_sim::GoalView;

/// What the card remembers between frames.
#[derive(Default)]
pub struct GoalsUi {
    /// The phone sheet is open.
    pub open: bool,
    goals: Vec<GoalView>,
    asked: f64,
}

const REFRESH_S: f64 = 0.5;

pub(super) fn rows(ui: &mut Ui, goals: &[GoalView], touch: bool) -> Option<us_sim::CivicKind> {
    let mut build = None;
    // the first goal not done is spelled out; the rest are one line each
    let next = goals.iter().position(|g| !g.done);
    for (i, g) in goals.iter().enumerate() {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(14.0, 14.0), egui::Sense::hover());
            let c = r.center();
            if g.done {
                ui.painter().circle_filled(c, 6.0, GOOD);
            } else {
                ui.painter().circle_stroke(c, 6.0, Stroke::new(1.5, MUTED));
            }
            let text =
                RichText::new(g.title)
                    .font(ui_font(13.0))
                    .color(if g.done { MUTED } else { INK });
            ui.label(if g.done { text.strikethrough() } else { text });
        });
        if Some(i) == next {
            ui.indent(("goal-hint", i), |ui| {
                wrapped(ui, g.hint, 11.5, TEXT);
                if !g.reward.is_empty() {
                    lab_c(ui, format!("Reward: {}", g.reward), 11.0, ACCENT);
                }
                if let Some(kind) = g.goal.civic() {
                    let b = ui.add(
                        egui::Button::new(
                            RichText::new(format!("Open Build: {}", kind.label().to_lowercase()))
                                .font(ui_font(12.0))
                                .color(theme::LINE),
                        )
                        .fill(ACCENT)
                        .min_size(vec2(0.0, if touch { theme::TOUCH } else { 24.0 })),
                    );
                    if b.clicked() {
                        build = Some(kind);
                    }
                }
            });
        }
    }
    build
}

impl Game {
    pub(super) fn refresh_goals(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        if now - self.ui.goals.asked > REFRESH_S || now < self.ui.goals.asked {
            self.ui.goals.goals = self.sim.goals();
            self.ui.goals.asked = now;
        }
    }

    /// The First goals card, drawn inside the top-left goal slot on a desktop in place of the
    /// milestone goal. Returns whether to hide it and a building to open in Build.
    pub(super) fn first_goals_card(
        &mut self,
        ui: &mut Ui,
        ctx: &Context,
    ) -> Option<us_sim::CivicKind> {
        self.refresh_goals(ctx);
        let goals = self.ui.goals.goals.clone();
        let done = goals.iter().filter(|g| g.done).count();
        let (mut hide, mut build) = (false, None);
        ui.horizontal(|ui| {
            lab(ui, format!("First goals  {done}/{}", goals.len()), 13.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button(RichText::new("Hide").font(ui_font(11.0)).color(MUTED))
                    .clicked()
                {
                    hide = true;
                }
            });
        });
        build = build.or(rows(ui, &goals, false));
        if hide {
            self.sim.dismiss_goals();
            self.ui.goals.open = false;
        }
        build
    }

    /// The phone chip and sheet. (A desktop shows the card in the goal slot instead.)
    pub(super) fn goals_ui(&mut self, ctx: &Context) {
        if !theme::is_compact(ctx) || !self.sim.goals_visible() {
            return;
        }
        self.refresh_goals(ctx);
        let goals = self.ui.goals.goals.clone();
        let done = goals.iter().filter(|g| g.done).count();
        let mut build = None;
        let mut hide = false;
        if theme::is_compact(ctx) {
            let up = ctx.content_rect().bottom() - dock_top(ctx) + 8.0;
            // clear of the growth banner, which sits just above the bottom bar
            let lift = if self.ui.growth.has_alerts() {
                76.0
            } else {
                0.0
            };
            let mut tap = false;
            egui::Area::new(egui::Id::new("goals-chip"))
                .anchor(Align2::RIGHT_BOTTOM, vec2(-8.0, -up - lift))
                .show(ctx, |ui| {
                    let b = ui.add(
                        egui::Button::new(
                            RichText::new(format!("Goals {done}/{}", goals.len()))
                                .font(ui_font(12.5))
                                .color(INK),
                        )
                        .fill(theme::RAISED)
                        .min_size(vec2(0.0, theme::TOUCH)),
                    );
                    tap = b.clicked();
                });
            if tap {
                self.ui.goals.open = !self.ui.goals.open;
            }
            if self.ui.goals.open {
                let closed = window(
                    ctx,
                    "goals",
                    "Goals",
                    360.0,
                    egui::pos2(300.0, 120.0),
                    |ui| {
                        build = rows(ui, &goals, true);
                        if ui.button("Hide the goals").clicked() {
                            hide = true;
                        }
                    },
                );
                if closed {
                    self.ui.goals.open = false;
                }
            }
        }
        // on a desktop the card shows in place of the milestone goal (see `first_goals_card`)
        if hide {
            self.sim.dismiss_goals();
            self.ui.goals.open = false;
        }
        if let Some(k) = build {
            self.open_build_for(k);
        }
    }
}
