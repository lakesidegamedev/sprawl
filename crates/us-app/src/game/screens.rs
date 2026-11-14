//! Screens added in preview 4: choosing rules for a new town, saved games, voters and election
//! night, the end of a game, and the phone layout's bottom bar.

use super::panels::{DOCK_H, MapView};
use super::{Game, Screen, State, label, money};
use crate::saves::{self, SaveInfo};
use crate::theme::{
    self, ACCENT, BAD, EDGE, GOOD, INK, LINE, PANEL_DARK, RAISED, TEXT, heading, pixel,
};
use egui::{Align2, Color32, Context, RichText, Sense, Stroke, Ui, vec2};
use us_gen::Preset;
use us_sim::{Difficulty, ElectionResult, GameOverReason, Issue, Rules};

const MUTED: Color32 = Color32::from_rgb(170, 176, 196);
const GOLD: Color32 = ACCENT;

/// How a new town is played.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Standard,
    Sandbox,
    SandboxUnlimited,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::Standard, Mode::Sandbox, Mode::SandboxUnlimited];

    pub fn rules(self) -> Rules {
        match self {
            Mode::Standard => Rules::standard(),
            Mode::Sandbox => Rules::sandbox(),
            Mode::SandboxUnlimited => Rules::sandbox_unlimited(),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Standard => "Standard",
            Mode::Sandbox => "Sandbox",
            Mode::SandboxUnlimited => "Sandbox, unlimited money",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Mode::Standard => {
                "Voters judge you every four years and the state takes over a town that stays broke."
            }
            Mode::Sandbox => "No elections and no bankruptcy. Build as long as you like.",
            Mode::SandboxUnlimited => "A sandbox where the treasury never runs out. Try anything.",
        }
    }
}

/// Where a button on an end screen sends the player.
pub enum Nav {
    NewTown,
    Load,
    Menu,
}

/// Pick-one row: a title and a line of explanation, as large as a finger.
fn choice(ui: &mut Ui, on: bool, title: &str, blurb: &str) -> bool {
    let compact = theme::is_compact(ui.ctx());
    let w = ui.available_width();
    let r = egui::Frame::new()
        .fill(if on {
            Color32::from_rgb(70, 54, 22)
        } else {
            RAISED
        })
        .stroke(Stroke::new(2.0, if on { ACCENT } else { EDGE }))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(w - 24.0);
            ui.set_min_height(if compact { theme::TOUCH - 12.0 } else { 20.0 });
            ui.label(
                RichText::new(title.to_uppercase())
                    .font(pixel(if compact { 14.0 } else { 16.0 }))
                    .color(if on { ACCENT } else { INK }),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(blurb)
                        .size(if compact { 13.0 } else { 14.0 })
                        .color(if on { TEXT } else { MUTED }),
                )
                .wrap(),
            );
        })
        .response
        .interact(Sense::click());
    r.clicked()
}

/// "Grid" becomes "Grid town"; "Lake town" stays as it is.
pub(super) fn town_name(label: &str) -> String {
    if label.to_lowercase().ends_with("town") {
        label.to_string()
    } else {
        format!("{label} town")
    }
}

fn small_button(ui: &mut Ui, text: &str) -> egui::Response {
    let compact = theme::is_compact(ui.ctx());
    ui.add(
        egui::Button::new(RichText::new(text).font(pixel(14.0)).color(INK)).min_size(vec2(
            if compact { 70.0 } else { 0.0 },
            if compact { theme::TOUCH } else { 28.0 },
        )),
    )
}

impl State {
    pub(super) fn refresh_saves(&mut self) {
        let dir = saves::save_dir();
        self.saves = saves::list(&dir);
        self.continue_info = self.saves.iter().find(|i| i.problem.is_none()).cloned();
    }

    pub(super) fn new_town_ui(&mut self, ctx: &Context) {
        theme::centered(ctx, "newtown", 20, |ui| {
            let compact = theme::is_compact(ctx);
            ui.set_width(if compact {
                (ctx.content_rect().width() - 70.0).max(220.0)
            } else {
                600.0
            });
            ui.label(heading("NEW TOWN", if compact { 24.0 } else { 32.0 }));
            ui.horizontal_wrapped(|ui| {
                if theme::button(ui, "START").clicked() {
                    self.screen = Screen::Generating;
                    self.gen_wait = 0;
                }
                if theme::button(ui, "BACK").clicked() {
                    self.screen = Screen::Menu;
                }
            });
            label(ui, "HOW YOU PLAY", 14.0);
            for m in Mode::ALL {
                if choice(ui, self.mode == m, m.label(), m.blurb()) {
                    self.mode = m;
                }
            }
            ui.add_space(4.0);
            label(ui, "DIFFICULTY", 14.0);
            for d in Difficulty::ALL {
                if choice(ui, self.difficulty == d, d.label(), d.blurb()) {
                    self.difficulty = d;
                }
            }
            ui.add_space(4.0);
            label(ui, "TOWN TYPE", 14.0);
            for p in Preset::ALL {
                if choice(ui, self.preset == p, &town_name(p.label()), p.blurb()) {
                    self.preset = p;
                }
            }
            ui.add_space(4.0);
            label(ui, "SEED", 14.0);
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.seed_text)
                        .desired_width(150.0)
                        .font(pixel(16.0)),
                );
                if small_button(ui, "RANDOM").clicked() {
                    self.seed_text = super::random_seed().to_string();
                }
            });
        });
    }

    pub(super) fn load_ui(&mut self, ctx: &Context) {
        theme::centered(ctx, "load", 20, |ui| {
            let compact = theme::is_compact(ctx);
            let w = if compact {
                (ctx.content_rect().width() - 70.0).max(220.0)
            } else {
                640.0
            };
            ui.set_width(w);
            ui.label(heading("LOAD A GAME", if compact { 24.0 } else { 32.0 }));
            label(
                ui,
                &format!("SAVES ARE KEPT IN {}", saves::save_dir().display()),
                10.0,
            );
            if self.saves.iter().any(|s| s.pre_streets) {
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Towns saved before 1.0.0 preview 6 cannot be opened: towns now grow \
                             street by street, and the old towns cannot be converted. Start a new \
                             town. Saves from preview 6 on will keep opening in later versions.",
                        )
                        .size(14.0)
                        .color(BAD),
                    )
                    .wrap(),
                );
            }
            if !self.load_note.is_empty() {
                ui.add(
                    egui::Label::new(RichText::new(&self.load_note).size(14.0).color(BAD)).wrap(),
                );
            }
            if self.saves.is_empty() {
                ui.add_space(8.0);
                label(ui, "NO SAVED GAMES YET.", 14.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(
                            "Use Save in the pause menu. The game also saves by itself every game year.",
                        )
                        .size(14.0)
                        .color(MUTED),
                    )
                    .wrap(),
                );
            }
            let mut open: Option<std::path::PathBuf> = None;
            let mut del: Option<std::path::PathBuf> = None;
            let mut ask: Option<std::path::PathBuf> = None;
            for info in self.saves.clone() {
                save_row(
                    ui,
                    &info,
                    w,
                    self.delete_ask.as_ref(),
                    &mut open,
                    &mut ask,
                    &mut del,
                );
            }
            if let Some(p) = open {
                self.load_note.clear();
                self.load_save(&p);
            }
            if let Some(p) = ask {
                self.delete_ask = Some(p);
            }
            if let Some(p) = del {
                self.delete_ask = None;
                match saves::delete(&p) {
                    Ok(()) => self.load_note.clear(),
                    Err(e) => self.load_note = format!("Could not delete that save: {e}"),
                }
                self.refresh_saves();
            }
            ui.add_space(8.0);
            if theme::button(ui, "BACK").clicked() {
                self.delete_ask = None;
                if self.load_from_game && self.game.is_some() {
                    self.load_from_game = false;
                    self.screen = Screen::Game;
                } else {
                    self.load_from_game = false;
                    self.screen = Screen::Menu;
                }
            }
        });
    }

    /// Open a save. Failure leaves the current screen and says why.
    pub(super) fn load_save(&mut self, path: &std::path::Path) {
        match saves::load(path) {
            Ok(sim) => {
                self.start_loaded(sim, Some(path.to_path_buf()));
                self.load_from_game = false;
                self.load_note.clear();
            }
            Err(e) => {
                self.load_note = format!("Could not open that save. {e}");
                self.refresh_saves();
            }
        }
    }
}

fn save_row(
    ui: &mut Ui,
    info: &SaveInfo,
    w: f32,
    confirm: Option<&std::path::PathBuf>,
    open: &mut Option<std::path::PathBuf>,
    ask: &mut Option<std::path::PathBuf>,
    del: &mut Option<std::path::PathBuf>,
) {
    let compact = theme::is_compact(ui.ctx());
    egui::Frame::new()
        .fill(RAISED)
        .stroke(Stroke::new(2.0, EDGE))
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.set_width(w - 24.0);
            ui.label(
                RichText::new(info.name.to_uppercase())
                    .font(pixel(if compact { 14.0 } else { 16.0 }))
                    .color(if info.problem.is_some() { MUTED } else { INK }),
            );
            if let Some(p) = &info.problem {
                ui.add(egui::Label::new(RichText::new(p).size(14.0).color(BAD)).wrap());
            } else {
                let mode = if info.sandbox { "  SANDBOX" } else { "" };
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "{}, {}, {} people{mode}. Saved {}.",
                            town_name(&info.town),
                            info.date,
                            info.population,
                            saves::age_text(info.unix)
                        ))
                        .size(14.0)
                        .color(MUTED),
                    )
                    .wrap(),
                );
            }
            ui.horizontal(|ui| {
                ui.add_enabled_ui(info.problem.is_none(), |ui| {
                    let b = egui::Button::new(RichText::new("LOAD").font(pixel(14.0)).color(INK))
                        .min_size(vec2(86.0, if compact { theme::TOUCH } else { 28.0 }));
                    if ui.add(b).clicked() {
                        *open = Some(info.path.clone());
                    }
                });
                if confirm == Some(&info.path) {
                    if small_button(ui, "SURE? DELETE").clicked() {
                        *del = Some(info.path.clone());
                    }
                } else if small_button(ui, "DELETE").clicked() {
                    *ask = Some(info.path.clone());
                }
            });
        });
}

// ----- the town's own screens -----

fn approval_color(pct: i32) -> Color32 {
    if pct >= 60 {
        GOOD
    } else if pct >= 50 {
        GOLD
    } else {
        BAD
    }
}

/// "3 years and 2 months" style distance to the election.
fn distance_text(days: i32) -> String {
    if days <= 0 {
        return "today".into();
    }
    let months = (days as f32 / 30.4).round() as i32;
    if days < 60 {
        format!("in {days} days")
    } else if months < 24 {
        format!("in {months} months")
    } else {
        let y = months / 12;
        let m = months % 12;
        if m == 0 {
            format!("in {y} years")
        } else {
            format!("in {y} years and {m} months")
        }
    }
}

impl Game {
    pub(super) fn is_sandbox(&self) -> bool {
        self.sim.rules().is_sandbox()
    }

    /// The approval meter for the top bar. Click it for the breakdown.
    pub(super) fn approval_chip(&mut self, ui: &mut Ui, compact: bool) {
        let pct = self.sim.approval_pct();
        let when = if self.is_sandbox() {
            "SANDBOX".to_string()
        } else {
            let t = self.sim.next_election_date().text().to_uppercase();
            if compact {
                // month and year only: the chip is narrow
                format!("VOTE {}", t.split_once(' ').map_or(t.as_str(), |x| x.1))
            } else {
                format!("VOTE {t}")
            }
        };
        let size = vec2(
            if compact { 120.0 } else { 168.0 },
            if compact { theme::TOUCH } else { 26.0 },
        );
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, 0.0, PANEL_DARK);
        let mut f = rect.shrink(2.0);
        f.set_width(f.width() * (pct.clamp(0, 100) as f32 / 100.0));
        p.rect_filled(f, 0.0, approval_color(pct).gamma_multiply(0.55));
        p.rect_stroke(
            rect,
            0.0,
            Stroke::new(2.0, if self.voters_open { ACCENT } else { EDGE }),
            egui::StrokeKind::Inside,
        );
        if compact {
            p.text(
                rect.center_top() + vec2(0.0, 7.0),
                Align2::CENTER_TOP,
                format!("APPROVAL {pct}%"),
                pixel(10.0),
                INK,
            );
            p.text(
                rect.center_bottom() - vec2(0.0, 7.0),
                Align2::CENTER_BOTTOM,
                &when,
                pixel(10.0),
                INK,
            );
        } else {
            p.text(
                rect.center(),
                Align2::CENTER_CENTER,
                format!("APPROVAL {pct}%"),
                pixel(13.0),
                INK,
            );
        }
        if resp.on_hover_text("What voters think (V)").clicked() {
            self.toggle_voters();
        }
        if !compact {
            ui.label(RichText::new(when).font(pixel(13.0)).color(INK));
        }
    }

    pub(super) fn toggle_voters(&mut self) {
        let was = self.voters_open;
        if self.compact {
            self.close_sheets();
        }
        self.voters_open = !was;
    }

    /// Close every panel (phone layout: one sheet at a time).
    pub(super) fn close_sheets(&mut self) {
        let u = &mut self.ui;
        u.levers_open = false;
        u.budget_open = false;
        u.loans_open = false;
        u.build_open = false;
        u.stats_open = false;
        u.news_open = false;
        u.tool = None;
        self.voters_open = false;
        self.map_open = false;
    }

    pub(super) fn any_sheet(&self) -> bool {
        let u = &self.ui;
        u.levers_open
            || u.budget_open
            || u.loans_open
            || u.build_open
            || u.stats_open
            || u.news_open
            || self.voters_open
            || self.map_open
    }

    /// Keep at most one sheet open: the phone layout has room for one.
    pub(super) fn one_sheet_only(&mut self) {
        let order = [
            self.voters_open,
            self.map_open,
            self.ui.build_open,
            self.ui.news_open,
            self.ui.stats_open,
            self.ui.loans_open,
            self.ui.budget_open,
            self.ui.levers_open,
        ];
        let Some(keep) = order.iter().position(|&b| b) else {
            return;
        };
        if order.iter().filter(|&&b| b).count() > 1 {
            let (v, m) = (keep == 0, keep == 1);
            let (b, n, s, l, bu, lv) = (
                keep == 2,
                keep == 3,
                keep == 4,
                keep == 5,
                keep == 6,
                keep == 7,
            );
            self.voters_open = v;
            self.map_open = m;
            if !b {
                self.ui.tool = None;
            }
            self.ui.build_open = b;
            self.ui.news_open = n;
            self.ui.stats_open = s;
            self.ui.loans_open = l;
            self.ui.budget_open = bu;
            self.ui.levers_open = lv;
        }
    }

    /// The phone layout's one bar of panel buttons along the bottom.
    pub(super) fn dock_ui(&mut self, ui: &mut Ui) {
        let unread = self.ui.events.len().saturating_sub(self.ui.news_seen);
        let news = if unread > 0 {
            format!("NEWS {unread}")
        } else {
            "NEWS".to_string()
        };
        let items: [(&str, bool); 8] = [
            ("TAXES", self.ui.levers_open),
            ("BUDGET", self.ui.budget_open),
            ("BONDS", self.ui.loans_open),
            ("BUILD", self.ui.build_open),
            ("GRAPHS", self.ui.stats_open),
            (news.as_str(), self.ui.news_open),
            ("VOTERS", self.voters_open),
            ("MAP", self.map_open),
        ];
        let rows = super::panels::dock_rows(ui.ctx());
        let per = items.len().div_ceil(rows);
        let n = per as f32;
        let gap = 3.0;
        let w = ((ui.available_width() - 12.0 - gap * (n - 1.0)) / n).floor();
        let font = if rows == 1 && w >= 64.0 { 12.0 } else { 11.0 };
        let mut hit: Option<usize> = None;
        ui.spacing_mut().item_spacing = vec2(gap, gap);
        ui.spacing_mut().button_padding = vec2(1.0, 6.0);
        for row in items.chunks(per).enumerate() {
            ui.horizontal(|ui| {
                for (j, (name, on)) in row.1.iter().enumerate() {
                    let i = row.0 * per + j;
                    let b =
                        egui::Button::new(RichText::new(*name).font(pixel(font)).color(if *on {
                            LINE
                        } else {
                            INK
                        }))
                        .fill(if *on { ACCENT } else { RAISED })
                        .min_size(vec2(w, theme::TOUCH));
                    if ui.add(b).clicked() {
                        hit = Some(i);
                    }
                }
            });
        }
        if let Some(i) = hit {
            let was = items[i].1;
            self.close_sheets();
            if !was {
                match i {
                    0 => self.ui.levers_open = true,
                    1 => self.ui.budget_open = true,
                    2 => self.ui.loans_open = true,
                    3 => self.ui.build_open = true,
                    4 => self.ui.stats_open = true,
                    5 => self.ui.news_open = true,
                    6 => self.voters_open = true,
                    _ => self.map_open = true,
                }
            }
        }
    }

    /// Map views as a sheet on phones, where the toolbar's drop-down has no room.
    pub(super) fn map_sheet(&mut self, ctx: &Context) {
        let closed = super::panels::sheet(ctx, "Map view", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for v in MapView::ALL {
                    let on = self.ui.view == v;
                    let b = egui::Button::new(
                        RichText::new(v.label().to_uppercase())
                            .font(pixel(12.0))
                            .color(if on { LINE } else { INK }),
                    )
                    .fill(if on { ACCENT } else { RAISED })
                    .min_size(vec2(0.0, theme::TOUCH));
                    if ui.add(b).clicked() {
                        self.ui.view = v;
                    }
                }
            });
        });
        if closed {
            self.map_open = false;
        }
    }

    // ----- voters -----

    pub(super) fn voters_ui(&mut self, ctx: &Context, screen_w: f32) {
        let a = self.sim.approval();
        let pos = egui::Pos2::new((screen_w - 440.0).max(10.0), 110.0);
        let sandbox = self.is_sandbox();
        let days = self.sim.days_to_election();
        let when = self.sim.next_election_date().text();
        let last = self.sim.last_election().cloned();
        let won = self.sim.elections_won();
        let closed = super::panels::window(ctx, "voters", "Voters", 400.0, pos, |ui| {
            ui.horizontal(|ui| {
                ui.label(heading(&format!("{}%", a.total), 32.0).color(approval_color(a.total)));
                ui.vertical(|ui| {
                    label(ui, "APPROVAL", 12.0);
                    let msg = if sandbox {
                        "SANDBOX: NOBODY CAN VOTE YOU OUT".to_string()
                    } else {
                        format!(
                            "NEXT ELECTION {} ({})",
                            when.to_uppercase(),
                            distance_text(days).to_uppercase()
                        )
                    };
                    ui.add(
                        egui::Label::new(RichText::new(msg).font(pixel(12.0)).color(MUTED)).wrap(),
                    );
                });
            });
            ui.add_space(4.0);
            let worst = worst_line(&a);
            ui.add(egui::Label::new(RichText::new(worst).size(15.0).color(TEXT)).wrap());
            ui.add_space(6.0);
            for (name, v) in [
                ("Taxes", a.taxes),
                ("Services", a.services),
                ("Jobs", a.jobs),
                ("Traffic", a.traffic),
                ("Debt", a.debt),
                ("Fires", a.fires),
            ] {
                points_row(ui, name, v);
            }
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Everyone starts at 70. Each line shows how many points it adds or takes away. You need about half the votes to stay mayor.",
                    )
                    .size(13.0)
                    .color(MUTED),
                )
                .wrap(),
            );
            if let Some(r) = &last {
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(
                        RichText::new(format!(
                            "Last election: {} with {}% of the vote against {}, who ran on {}. Elections won: {won}.",
                            if r.won { "won" } else { "lost" },
                            r.share,
                            r.challenger,
                            r.issue.text()
                        ))
                        .size(13.0)
                        .color(MUTED),
                    )
                    .wrap(),
                );
            }
        });
        if closed {
            self.voters_open = false;
        }
    }

    // ----- election night and game over -----

    /// Election night. Returns true when the player continues.
    pub(super) fn election_ui(&mut self, ctx: &Context) -> bool {
        let Some(r) = self.election.clone() else {
            return false;
        };
        let mut go = false;
        theme::dim(ctx, 120);
        theme::centered(ctx, "election", 24, |ui| {
            ui.set_width(if theme::is_compact(ctx) {
                (ctx.content_rect().width() - 70.0).max(220.0)
            } else {
                460.0
            });
            label(ui, "ELECTION NIGHT", 14.0);
            ui.label(
                heading(if r.won { "YOU WON" } else { "YOU LOST" }, 36.0).color(if r.won {
                    GOOD
                } else {
                    BAD
                }),
            );
            ui.add_space(4.0);
            vote_bars(ui, &r);
            ui.add_space(6.0);
            let line = if r.won {
                format!(
                    "You kept your seat with {}% of the vote. {} ran against you on {}.",
                    r.share,
                    r.challenger,
                    r.issue.text()
                )
            } else {
                format!(
                    "{} won with {}% of the vote by running on {}.",
                    r.challenger,
                    100 - r.share,
                    r.issue.text()
                )
            };
            ui.add(egui::Label::new(RichText::new(line).size(16.0).color(TEXT)).wrap());
            ui.add(
                egui::Label::new(
                    RichText::new(format!("Voter approval going in: {}%.", r.approval))
                        .size(14.0)
                        .color(MUTED),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            if theme::button(ui, "CONTINUE").clicked() {
                go = true;
            }
        });
        if go {
            self.election = None;
        }
        go
    }

    pub(super) fn game_over_ui(&mut self, ctx: &Context) -> Option<Nav> {
        let over = self.sim.game_over()?.clone();
        let mut nav = None;
        theme::dim(ctx, 150);
        theme::centered(ctx, "gameover", 24, |ui| {
            let compact = theme::is_compact(ctx);
            ui.set_width(if compact {
                (ctx.content_rect().width() - 70.0).max(220.0)
            } else {
                520.0
            });
            label(ui, "GAME OVER", 14.0);
            let title = match over.reason {
                GameOverReason::Bankrupt => "THE STATE TOOK OVER",
                GameOverReason::LostElection { .. } => "VOTED OUT",
            };
            ui.label(heading(title, if compact { 26.0 } else { 34.0 }).color(BAD));
            ui.add(egui::Label::new(RichText::new(over.headline()).size(16.0).color(TEXT)).wrap());
            if let GameOverReason::LostElection { issue, .. } = &over.reason {
                ui.add(
                    egui::Label::new(
                        RichText::new(format!("They ran on {}.", issue.text()))
                            .size(14.0)
                            .color(MUTED),
                    )
                    .wrap(),
                );
            }
            let buttons = |ui: &mut Ui, nav: &mut Option<Nav>| {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if theme::button(ui, "NEW TOWN").clicked() {
                        *nav = Some(Nav::NewTown);
                    }
                    if theme::button(ui, "LOAD A SAVE").clicked() {
                        *nav = Some(Nav::Load);
                    }
                    if theme::button(ui, "MENU").clicked() {
                        *nav = Some(Nav::Menu);
                    }
                });
            };
            if compact {
                buttons(ui, &mut nav);
            }
            ui.add_space(6.0);
            label(ui, "YOUR CAREER", 14.0);
            let s = &over.summary;
            egui::Grid::new("career")
                .num_columns(2)
                .spacing([24.0, 4.0])
                .show(ui, |ui| {
                    for (k, v) in [
                        ("Years as mayor", s.years.to_string()),
                        ("People at the end", s.population.to_string()),
                        ("Most people", s.peak_population.to_string()),
                        ("Buildings", s.buildings.to_string()),
                        ("Jobs", s.jobs.to_string()),
                        ("Elections won", s.elections_won.to_string()),
                        ("Treasury", money(s.treasury)),
                        ("Debt", money(s.debt)),
                        ("Fires", s.fires.to_string()),
                        ("Families who left", s.departures.to_string()),
                        ("Approval at the end", format!("{}%", s.approval)),
                    ] {
                        ui.label(
                            RichText::new(k.to_uppercase())
                                .font(pixel(12.0))
                                .color(MUTED),
                        );
                        ui.label(RichText::new(v).font(pixel(14.0)).color(INK));
                        ui.end_row();
                    }
                });
            if !compact {
                buttons(ui, &mut nav);
            }
        });
        nav
    }
}

fn worst_line(a: &us_sim::Approval) -> String {
    let parts = [a.taxes, a.services, a.jobs, a.traffic, a.debt, a.fires];
    if parts.iter().all(|&p| p >= 0) {
        return "Nothing is bothering voters right now.".into();
    }
    let what = match a.worst {
        Issue::Taxes => "taxes are too high".to_string(),
        other => format!("they are most upset about {}", other.text()),
    };
    format!("Voters say {what}. That is the issue a challenger would run on.")
}

fn points_row(ui: &mut Ui, name: &str, v: i32) {
    ui.horizontal(|ui| {
        ui.allocate_ui(vec2(92.0, 18.0), |ui| {
            ui.label(
                RichText::new(name.to_uppercase())
                    .font(pixel(12.0))
                    .color(TEXT),
            );
        });
        let (rect, _) = ui.allocate_exact_size(vec2(150.0, 14.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 0.0, PANEL_DARK);
        let mid = rect.center().x;
        let half = rect.width() / 2.0 - 2.0;
        let len = (v.abs() as f32 / 30.0).min(1.0) * half;
        let r = if v >= 0 {
            egui::Rect::from_min_max(
                egui::pos2(mid, rect.top() + 2.0),
                egui::pos2(mid + len, rect.bottom() - 2.0),
            )
        } else {
            egui::Rect::from_min_max(
                egui::pos2(mid - len, rect.top() + 2.0),
                egui::pos2(mid, rect.bottom() - 2.0),
            )
        };
        p.rect_filled(r, 0.0, if v >= 0 { GOOD } else { BAD });
        p.line_segment(
            [egui::pos2(mid, rect.top()), egui::pos2(mid, rect.bottom())],
            Stroke::new(1.0, EDGE),
        );
        ui.label(
            RichText::new(format!("{v:+}"))
                .font(pixel(13.0))
                .color(if v >= 0 { GOOD } else { BAD }),
        );
    });
}

/// Your share against the challenger's, as two bars.
fn vote_bars(ui: &mut Ui, r: &ElectionResult) {
    let w = ui.available_width().min(420.0);
    let (rect, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, PANEL_DARK);
    let mine = r.share.clamp(0, 100) as f32 / 100.0;
    let mut a = rect.shrink(2.0);
    let total = a.width();
    a.set_width(total * mine);
    p.rect_filled(a, 0.0, GOLD);
    let mut b = rect.shrink(2.0);
    b.min.x = b.max.x - total * (1.0 - mine);
    p.rect_filled(b, 0.0, Color32::from_rgb(120, 140, 170));
    p.text(
        rect.left_center() + vec2(8.0, 0.0),
        Align2::LEFT_CENTER,
        format!("YOU {}%", r.share),
        pixel(13.0),
        LINE,
    );
    p.text(
        rect.right_center() - vec2(8.0, 0.0),
        Align2::RIGHT_CENTER,
        format!("{} {}%", r.challenger.to_uppercase(), 100 - r.share),
        pixel(13.0),
        LINE,
    );
    p.rect_stroke(rect, 0.0, Stroke::new(2.0, EDGE), egui::StrokeKind::Inside);
    let _ = DOCK_H;
}
