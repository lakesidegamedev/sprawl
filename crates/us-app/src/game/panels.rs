//! The mayor's screens: tax levers, budget, bonds, civic building tools, graphs, map views and
//! the news feed. Everything here reads and changes the simulation through its public API.

use super::charts::{self, Chart, Fmt, Series};
use super::{Game, Sel, money};
use crate::theme::{
    self, ACCENT, BAD, EDGE, GOOD, INK, LINE, PANEL_DARK, RAISED, TEXT, heading, pixel,
};
use egui::{Align2, Color32, Context, Pos2, RichText, Sense, Stroke, Ui, vec2};
use glam::Vec3;
use us_gen::{Pt, pt};
use us_render::{Overlay, OverlayKind, View, project};
use us_sim::{
    ABANDON_ABOVE_X10, BOND_MONTHS, BOND_RATE_X10, BOND_SIZE, BUDGET_MAX_PCT, BUDGET_MIN_PCT,
    CivicKind, Class, Date, Dept, Event, EventKind, LedgerNow, LoanError, MapKind, Phase, Service,
    TAX_MAX_X10, TAX_MIN_X10, class_of,
};

// Series colours: one fixed hue per meaning, readable on the dark navy panels.
const BLUE: Color32 = Color32::from_rgb(90, 169, 230);
const GOLD: Color32 = ACCENT;
const CORAL: Color32 = BAD;
const TEAL: Color32 = GOOD;
const VIOLET: Color32 = Color32::from_rgb(169, 139, 224);
const ORANGE: Color32 = Color32::from_rgb(240, 138, 75);
const LIME: Color32 = Color32::from_rgb(155, 212, 90);
const GREY: Color32 = Color32::from_rgb(159, 179, 200);

fn class_color(c: Class) -> Color32 {
    match c {
        Class::Homes => BLUE,
        Class::Shops => GOLD,
        Class::Industry => CORAL,
    }
}

fn dept_color(d: Dept) -> Color32 {
    match d {
        Dept::Police => BLUE,
        Dept::Fire => CORAL,
        Dept::Schools => GOLD,
        Dept::Health => TEAL,
        Dept::Parks => LIME,
        Dept::Roads => GREY,
    }
}

fn lab(ui: &mut Ui, text: impl AsRef<str>, size: f32) {
    let size = size.max(12.0);
    ui.label(
        RichText::new(text.as_ref().to_uppercase())
            .font(pixel(size))
            .color(TEXT),
    );
}

fn lab_c(ui: &mut Ui, text: impl AsRef<str>, size: f32, col: Color32) {
    let size = size.max(12.0);
    ui.label(
        RichText::new(text.as_ref().to_uppercase())
            .font(pixel(size))
            .color(col),
    );
}

fn wrapped(ui: &mut Ui, text: impl AsRef<str>, size: f32, col: Color32) {
    let size = size.max(12.0);
    ui.add(
        egui::Label::new(
            RichText::new(text.as_ref().to_uppercase())
                .font(pixel(size))
                .color(col),
        )
        .wrap(),
    );
}

fn btn(ui: &mut Ui, text: &str, on: bool, size: f32) -> egui::Response {
    let size = size.max(12.0);
    ui.add(
        egui::Button::new(
            RichText::new(text.to_uppercase())
                .font(pixel(size))
                .color(if on { LINE } else { INK }),
        )
        .fill(if on { ACCENT } else { RAISED }),
    )
}

/// A solid horizontal bar from 0 to 1.
fn bar(ui: &mut Ui, width: f32, frac: f32, col: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, 12.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, EDGE);
    let inner = rect.shrink(2.0);
    p.rect_filled(inner, 0.0, PANEL_DARK);
    let mut f = inner;
    f.set_width(inner.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(f, 0.0, col);
}

/// The frame and header every movable window shares. Returns true when closed.
fn window(
    ctx: &Context,
    id: &str,
    title: &str,
    width: f32,
    pos: Pos2,
    body: impl FnOnce(&mut Ui),
) -> bool {
    let mut closed = false;
    // at a large UI scale the screen has fewer points: shrink and scroll rather than overflow
    let screen = ctx.content_rect();
    let width = width.min(screen.width() - 30.0).max(240.0);
    let max_h = (screen.height() - 150.0).max(160.0);
    egui::Window::new(id)
        .title_bar(false)
        .frame(theme::panel())
        .resizable(false)
        .default_pos(pos)
        .default_width(width)
        .constrain(true)
        .show(ctx, |ui| {
            ui.set_width(width);
            ui.horizontal(|ui| {
                ui.label(heading(&title.to_uppercase(), 18.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if btn(ui, "X", false, 12.0).on_hover_text("Close").clicked() {
                        closed = true;
                    }
                });
            });
            ui.add_space(2.0);
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_width(width - 14.0);
                    body(ui);
                });
        });
    closed
}

// ----- map views -----

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MapView {
    Normal,
    LandValue,
    Level,
    Use,
    Coverage(Service),
    Demand(Class),
    Traffic,
}

impl MapView {
    pub const ALL: [MapView; 14] = [
        MapView::Normal,
        MapView::LandValue,
        MapView::Level,
        MapView::Use,
        MapView::Coverage(Service::Police),
        MapView::Coverage(Service::Fire),
        MapView::Coverage(Service::Schools),
        MapView::Coverage(Service::Health),
        MapView::Coverage(Service::Parks),
        MapView::Coverage(Service::Water),
        MapView::Demand(Class::Homes),
        MapView::Demand(Class::Shops),
        MapView::Demand(Class::Industry),
        MapView::Traffic,
    ];

    pub fn label(self) -> String {
        match self {
            MapView::Normal => "Normal".into(),
            MapView::LandValue => "Land value".into(),
            MapView::Level => "Building level".into(),
            MapView::Use => "Use".into(),
            MapView::Coverage(s) => format!("{} coverage", s.label()),
            MapView::Demand(c) => format!("{} demand", c.label()),
            MapView::Traffic => "Traffic".into(),
        }
    }

    fn kind(self) -> Option<MapKind> {
        Some(match self {
            MapView::Normal => return None,
            MapView::LandValue => MapKind::LandValue,
            MapView::Level => MapKind::BuildingLevel,
            MapView::Use => MapKind::Use,
            MapView::Coverage(s) => MapKind::Coverage(s),
            MapView::Demand(c) => MapKind::Demand(c),
            MapView::Traffic => MapKind::Traffic,
        })
    }

    /// One line under the legend where the colours need explaining.
    fn note(self) -> Option<&'static str> {
        match self {
            MapView::Demand(_) => Some("Other kinds of building stay grey"),
            MapView::Level => Some("Empty land shows as level 1"),
            _ => None,
        }
    }

    fn overlay_kind(self) -> Option<OverlayKind> {
        Some(match self {
            MapView::Normal => return None,
            MapView::LandValue => OverlayKind::LandValue,
            MapView::Level => OverlayKind::BuildingLevel,
            MapView::Use => OverlayKind::Use,
            MapView::Coverage(_) => OverlayKind::ServiceCoverage,
            MapView::Demand(_) => OverlayKind::Demand,
            MapView::Traffic => OverlayKind::Traffic,
        })
    }

    /// The renderer's own steps for this view, low to high, with their words.
    fn steps(self) -> Vec<(Color32, &'static str)> {
        let Some(k) = self.overlay_kind() else {
            return vec![];
        };
        let words: &[&str] = match self {
            MapView::Normal => &[],
            MapView::LandValue => &["Very low", "Low", "Fair", "Good", "High", "Very high"],
            MapView::Level => &["Level 1", "Level 2", "Level 3", "Level 4", "Level 5"],
            MapView::Use => &[
                "Homes",
                "Shops",
                "Industry",
                "Civic",
                "Open land",
                "Farm",
                "Other",
            ],
            MapView::Coverage(_) => &["None", "Thin", "Partial", "Good", "Full"],
            MapView::Demand(_) => &[
                "Very low", "Low", "Soft", "Balanced", "Firm", "High", "Crowded",
            ],
            MapView::Traffic => &["Free", "Light", "Busy", "Heavy", "Jammed"],
        };
        k.palette()
            .iter()
            .zip(words)
            .map(|(c, w)| (Color32::from_rgb(c[0], c[1], c[2]), *w))
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Range {
    Year,
    Five,
    All,
}

const TABS: [&str; 9] = [
    "People",
    "Money",
    "Income",
    "Spending",
    "Taxes",
    "Demand",
    "Buildings",
    "Land value",
    "Traffic",
];

/// Everything the mayor's screens remember during a session.
pub struct UiState {
    pub levers_open: bool,
    pub budget_open: bool,
    pub loans_open: bool,
    pub build_open: bool,
    pub stats_open: bool,
    pub news_open: bool,
    pub tool: Option<CivicKind>,
    /// Screenshot hook: hold the ghost at this spot instead of under the cursor.
    fixed_ghost: Option<Pt>,
    pub view: MapView,
    tab: usize,
    range: Range,
    hidden: std::collections::HashSet<(usize, usize)>,
    pub events: Vec<Event>,
    news_seen: usize,
    ledger: LedgerNow,
    /// Seconds since the view was last pushed to the renderer, and which view it showed.
    ov_age: f32,
    ov_view: MapView,
    clock: f32,
    cover: [f32; 6],
    cover_age: f32,
    /// Coverage of the selected lot, by service.
    sel_cov: Vec<(Service, f32)>,
    sel_cov_for: Option<usize>,
    sel_cov_age: f32,
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            levers_open: true,
            budget_open: false,
            loans_open: false,
            build_open: false,
            stats_open: false,
            news_open: false,
            tool: None,
            fixed_ghost: None,
            view: MapView::Normal,
            tab: 0,
            range: Range::All,
            hidden: Default::default(),
            events: Vec::new(),
            news_seen: 0,
            ledger: LedgerNow::default(),
            ov_age: 9.0,
            ov_view: MapView::Normal,
            clock: 0.0,
            cover: [0.0; 6],
            cover_age: 9.0,
            sel_cov: Vec::new(),
            sel_cov_for: None,
            sel_cov_age: 9.0,
        }
    }
}

fn ring_points(view: &View, c: Pt, r: f32, w: u32, h: u32, ppp: f32) -> Vec<Pos2> {
    let mut out = Vec::with_capacity(48);
    for i in 0..48 {
        let a = i as f32 / 48.0 * std::f32::consts::TAU;
        let p = Vec3::new(c.x as f32 + r * a.cos(), c.y as f32 + r * a.sin(), 0.0);
        if let Some(s) = project(view, p) {
            out.push(super::to_screen(s, w, h, ppp));
        }
    }
    out
}

fn dept_fraction(sim: &us_sim::Sim, s: Service) -> f64 {
    s.dept().map_or(1.0, |d| sim.budget_pct(d) as f64 / 100.0)
}

impl Game {
    /// Per frame: collect new events, refresh the cached numbers the screens show.
    pub(super) fn ui_tick(&mut self, dt: f32) {
        let new = self.sim.take_events();
        self.ui.events.extend(new);
        if self.ui.events.len() > 400 {
            let cut = self.ui.events.len() - 400;
            self.ui.events.drain(..cut);
            self.ui.news_seen = self.ui.news_seen.saturating_sub(cut);
        }
        if self.ui.news_open {
            self.ui.news_seen = self.ui.events.len();
        }
        self.ui.ledger = self.sim.ledger_now();
        self.ui.clock += dt;
        self.live.scene.set_time(self.ui.clock);
        self.ui.ov_age += dt;
        if self.ui.view != self.ui.ov_view
            || (self.ui.view != MapView::Normal && self.ui.ov_age > 0.5)
        {
            self.ui.ov_age = 0.0;
            self.ui.ov_view = self.ui.view;
            match self.overlay_values() {
                Some(ov) => {
                    self.live.ensure_geometry(&self.sim);
                    self.live.scene.set_overlay(Some(ov));
                }
                None => self.live.scene.set_overlay(None),
            }
        }
        self.ui.cover_age += dt;
        if self.ui.budget_open && self.ui.cover_age > 1.0 {
            self.ui.cover_age = 0.0;
            for s in Service::ALL {
                let v = self.sim.lot_values(MapKind::Coverage(s));
                let (mut sum, mut n) = (0.0f64, 0u32);
                for (l, c) in self.sim.lots().iter().zip(&v) {
                    if matches!(l.phase, Phase::Built { .. })
                        && !l.exempt
                        && class_of(l.kind).is_some()
                    {
                        sum += *c as f64;
                        n += 1;
                    }
                }
                self.ui.cover[s.index()] = if n == 0 { 0.0 } else { (sum / n as f64) as f32 };
            }
        }
        self.ui.sel_cov_age += dt;
        if let Some(Sel::Lot(i)) = self.sel {
            if self.ui.sel_cov_for != Some(i) || self.ui.sel_cov_age > 1.0 {
                self.ui.sel_cov_for = Some(i);
                self.ui.sel_cov_age = 0.0;
                self.ui.sel_cov = [
                    Service::Police,
                    Service::Fire,
                    Service::Schools,
                    Service::Health,
                    Service::Parks,
                    Service::Water,
                ]
                .iter()
                .map(|&s| {
                    let v = self.sim.lot_values(MapKind::Coverage(s));
                    (s, v.get(i).copied().unwrap_or(0.0))
                })
                .collect();
            }
        } else {
            self.ui.sel_cov_for = None;
        }
    }

    /// Keys for the mayor's screens. Returns true when the key was used.
    pub(super) fn ui_key(&mut self, code: winit::keyboard::KeyCode) -> bool {
        use winit::keyboard::KeyCode as K;
        let u = &mut self.ui;
        match code {
            K::KeyP => u.levers_open = !u.levers_open,
            K::KeyK => u.budget_open = !u.budget_open,
            K::KeyO => u.loans_open = !u.loans_open,
            K::KeyB => {
                u.build_open = !u.build_open;
                if !u.build_open {
                    u.tool = None;
                }
            }
            K::KeyG => u.stats_open = !u.stats_open,
            K::KeyJ => u.news_open = !u.news_open,
            K::KeyN => u.view = MapView::Normal,
            K::KeyL => u.view = MapView::LandValue,
            K::KeyV => u.view = MapView::Level,
            K::KeyU => u.view = MapView::Use,
            K::KeyT => u.view = MapView::Traffic,
            K::KeyC => {
                u.view = match u.view {
                    MapView::Coverage(s) => {
                        let i = Service::ALL.iter().position(|&x| x == s).unwrap_or(0);
                        MapView::Coverage(Service::ALL[(i + 1) % 6])
                    }
                    _ => MapView::Coverage(Service::Police),
                }
            }
            K::KeyX => {
                u.view = match u.view {
                    MapView::Demand(c) => MapView::Demand(Class::ALL[(c.index() + 1) % 3]),
                    _ => MapView::Demand(Class::Homes),
                }
            }
            K::KeyM => {
                let i = MapView::ALL.iter().position(|&x| x == u.view).unwrap_or(0);
                u.view = MapView::ALL[(i + 1) % MapView::ALL.len()];
            }
            K::Escape if u.tool.is_some() => u.tool = None,
            _ => return false,
        }
        true
    }

    /// Test hook for screenshots: open screens from a comma list such as `stats:2,view:1,bond`.
    pub(super) fn ui_hook(&mut self, spec: &str) {
        for tok in spec.split(',') {
            let (k, v) = tok.split_once(':').unwrap_or((tok, ""));
            let n = v.parse::<usize>().unwrap_or(0);
            match k {
                "levers" => self.ui.levers_open = true,
                "budget" => self.ui.budget_open = true,
                "loans" => self.ui.loans_open = true,
                "build" => {
                    self.ui.build_open = true;
                    self.ui.tool = CivicKind::ALL.get(n).copied();
                }
                "stats" => {
                    self.ui.stats_open = true;
                    self.ui.tab = n.min(TABS.len() - 1);
                }
                "spot" => {
                    // first empty lot where the chosen building fits, for a screenshot
                    let kind = self.ui.tool.unwrap_or(CivicKind::School);
                    let found = self
                        .sim
                        .lots()
                        .iter()
                        .filter(|l| l.phase == Phase::Vacant)
                        .map(|l| l.pos)
                        .filter(|&p| self.sim.check_civic(kind, p).is_ok())
                        .nth(n);
                    if let Some(p) = found {
                        self.ui.fixed_ghost = Some(p);
                        self.cam
                            .look_at(Vec3::new(p.x as f32, p.y as f32, 0.0), 1500.0);
                    }
                }
                "civic" => {
                    let kind = CivicKind::ALL[n.min(5)];
                    let found = self
                        .sim
                        .lots()
                        .iter()
                        .filter(|l| l.phase == Phase::Vacant)
                        .map(|l| l.pos)
                        .find(|&p| self.sim.check_civic(kind, p).is_ok());
                    if let Some(p) = found
                        && let Ok(i) = self.sim.place_civic(kind, p)
                    {
                        for _ in 0..90 {
                            self.sim.step_day();
                        }
                        self.sel = Some(Sel::Lot(i));
                        self.cam
                            .look_at(Vec3::new(p.x as f32, p.y as f32, 0.0), 1500.0);
                    }
                }
                "sel" | "selsvc" => {
                    let want_svc = k == "selsvc";
                    let found = self.sim.lots().iter().position(|l| {
                        matches!(l.phase, Phase::Built { .. })
                            && !l.exempt
                            && if want_svc {
                                l.kind == us_gen::LotKind::School
                            } else {
                                class_of(l.kind).is_some()
                            }
                    });
                    if let Some(i) = found {
                        let p = self.sim.lots()[i].pos;
                        self.sel = Some(Sel::Lot(i + if want_svc { 0 } else { n }));
                        self.cam
                            .look_at(Vec3::new(p.x as f32, p.y as f32, 0.0), 650.0);
                    }
                }
                "year" => self.ui.range = Range::Year,
                "five" => self.ui.range = Range::Five,
                "news" => self.ui.news_open = true,
                "view" => self.ui.view = MapView::ALL[n.min(MapView::ALL.len() - 1)],
                "bond" => {
                    let _ = self.sim.take_loan();
                }
                "nolevers" => self.ui.levers_open = false,
                _ => {}
            }
        }
    }

    // ----- toolbar and top bar extras -----

    /// The second strip under the top bar.
    pub(super) fn toolbar_ui(&mut self, ui: &mut Ui) {
        let unread = self.ui.events.len().saturating_sub(self.ui.news_seen);
        let u = &mut self.ui;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let t = |ui: &mut Ui, name: &str, on: &mut bool, tip: &str| {
                if btn(ui, name, *on, 12.0).on_hover_text(tip).clicked() {
                    *on = !*on;
                }
            };
            t(ui, "Taxes", &mut u.levers_open, "Tax rates and demand (P)");
            t(ui, "Budget", &mut u.budget_open, "Department funding (K)");
            t(ui, "Bonds", &mut u.loans_open, "Borrow and repay (O)");
            let was = u.build_open;
            t(ui, "Build", &mut u.build_open, "Place civic buildings (B)");
            if was && !u.build_open {
                u.tool = None;
            }
            t(ui, "Graphs", &mut u.stats_open, "Charts over time (G)");
            let news = if unread > 0 {
                format!("News {unread}")
            } else {
                "News".into()
            };
            t(ui, &news, &mut u.news_open, "What happened in town (J)");
            ui.add_space(10.0);
            lab_c(ui, "Map view", 11.0, Color32::from_rgb(170, 176, 196));
            egui::ComboBox::from_id_salt("mapview")
                .selected_text(RichText::new(u.view.label().to_uppercase()).font(pixel(12.0)))
                .width(190.0)
                .show_ui(ui, |ui| {
                    for v in MapView::ALL {
                        ui.selectable_value(
                            &mut u.view,
                            v,
                            RichText::new(v.label().to_uppercase()).font(pixel(12.0)),
                        );
                    }
                });
        });
    }

    /// Population, treasury and monthly net as small lines. Needs room, so it checks the width.
    pub(super) fn sparks_ui(&self, ui: &mut Ui) {
        if ui.available_width() < 640.0 {
            return;
        }
        let h = self.sim.history();
        if h.len() < 2 {
            return;
        }
        let tail = &h[h.len().saturating_sub(24)..];
        let pop: Vec<f64> = tail.iter().map(|s| s.population as f64).collect();
        let tre: Vec<f64> = tail.iter().map(|s| s.treasury as f64).collect();
        let net: Vec<f64> = tail
            .iter()
            .map(|s| (s.income_total() - s.spending_total()) as f64)
            .collect();
        let sz = vec2(56.0, 20.0);
        ui.add_space(8.0);
        charts::spark(ui, sz, &pop, BLUE, "Population, last 2 years");
        charts::spark(ui, sz, &tre, GOLD, "Treasury, last 2 years");
        charts::spark(ui, sz, &net, TEAL, "Monthly net, last 2 years");
    }

    // ----- taxes -----

    pub(super) fn levers_ui(&mut self, ctx: &Context) {
        egui::Area::new(egui::Id::new("tax"))
            .anchor(Align2::LEFT_BOTTOM, vec2(14.0, -34.0))
            .show(ctx, |ui| {
                theme::panel().show(ui, |ui| {
                    ui.set_width(320.0);
                    let open = self.ui.levers_open;
                    ui.horizontal(|ui| {
                        ui.label(heading("TAXES", 18.0));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if btn(ui, if open { "-" } else { "+" }, false, 12.0).clicked() {
                                self.ui.levers_open = !open;
                            }
                        });
                    });
                    let ledger = self.ui.ledger;
                    if !open {
                        let net = self.stats.income - self.stats.expense;
                        lab(
                            ui,
                            format!(
                                "NET {}{} A MONTH",
                                if net < 0 { "" } else { "+" },
                                money(net)
                            ),
                            12.0,
                        );
                        return;
                    }
                    let max_h = (ctx.content_rect().height() - 330.0).max(120.0);
                    egui::ScrollArea::vertical()
                        .max_height(max_h)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            for c in Class::ALL {
                                let rate = self.sim.tax_rates().get(c);
                                let income = ledger.income.get(c);
                                let per_point = if rate > 0 {
                                    income * 10 / rate as i64
                                } else {
                                    self.sim
                                        .history()
                                        .last()
                                        .filter(|s| s.tax_x10.get(c) > 0)
                                        .map_or(0, |s| {
                                            s.income.get(c) * 10 / s.tax_x10.get(c) as i64
                                        })
                                };
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                                    ui.painter().rect_filled(r, 0.0, class_color(c));
                                    lab(ui, c.label(), 14.0);
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let d = self.sim.class_demand(c);
                                            lab_c(
                                                ui,
                                                format!("DEMAND {:.0}%", d * 100.0),
                                                11.0,
                                                if d < 0.3 {
                                                    BAD
                                                } else if d < 0.55 {
                                                    GOLD
                                                } else {
                                                    GOOD
                                                },
                                            );
                                        },
                                    );
                                });
                                let mut pct = rate as f32 / 10.0;
                                let r = ui.add(
                                    egui::Slider::new(
                                        &mut pct,
                                        TAX_MIN_X10 as f32 / 10.0..=TAX_MAX_X10 as f32 / 10.0,
                                    )
                                    .step_by(0.5)
                                    .custom_formatter(|v, _| format!("{v:.1}%")),
                                );
                                if r.changed() {
                                    self.sim.set_tax_class(c, (pct * 10.0).round() as u32);
                                }
                                let (msg, col) = if rate > ABANDON_ABOVE_X10 {
                                    ("Too high: buildings empty out", BAD)
                                } else if rate >= 100 {
                                    ("High: growth is slow", ORANGE)
                                } else if rate <= 40 {
                                    ("Low: fast growth, thin income", GOOD)
                                } else {
                                    ("Balanced", TEXT)
                                };
                                ui.horizontal(|ui| {
                                    lab_c(ui, msg, 12.0, col);
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            lab(ui, format!("{}/MO", money(income)), 12.0);
                                        },
                                    );
                                });
                                lab_c(
                                    ui,
                                    format!("Each point is about {} a month", money(per_point)),
                                    12.0,
                                    Color32::from_rgb(170, 176, 196),
                                );
                                ui.add_space(2.0);
                            }
                        });
                    ui.separator();
                    let net = self.stats.income - self.stats.expense;
                    lab(
                        ui,
                        format!(
                            "TAX IN {}   COSTS {}",
                            money(self.stats.income),
                            money(self.stats.expense)
                        ),
                        12.0,
                    );
                    lab_c(
                        ui,
                        format!(
                            "NET {}{} A MONTH",
                            if net < 0 { "" } else { "+" },
                            money(net)
                        ),
                        12.0,
                        if net < 0 { BAD } else { GOOD },
                    );
                    lab(
                        ui,
                        format!(
                            "BUILT {}  BUILDING {}  EMPTY {}",
                            self.stats.built, self.stats.building, self.stats.empty
                        ),
                        11.0,
                    );
                });
            });
    }

    // ----- budget -----

    pub(super) fn budget_ui(&mut self, ctx: &Context) {
        let ledger = self.ui.ledger;
        let cover = self.ui.cover;
        let pos = Pos2::new(360.0, 100.0);
        let closed = window(ctx, "budget", "Budget", 560.0, pos, |ui| {
            wrapped(
                ui,
                "Funding below 100 percent shortens how far a building reaches and weakens it. Above 100 it reaches further. Road upkeep below 100 lowers land value.",
                11.0,
                Color32::from_rgb(170, 176, 196),
            );
            ui.add_space(4.0);
            egui::Grid::new("budget-grid")
                .num_columns(4)
                .spacing([14.0, 8.0])
                .show(ui, |ui| {
                    for h in ["DEPARTMENT", "FUNDING", "COST A MONTH", "COVERAGE"] {
                        lab_c(ui, h, 10.0, Color32::from_rgb(170, 176, 196));
                    }
                    ui.end_row();
                    for d in Dept::ALL {
                        let mut pct = self.sim.budget_pct(d);
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                            ui.painter().rect_filled(r, 0.0, dept_color(d));
                            lab(ui, d.label(), 12.0);
                        });
                        let r = ui.add(
                            egui::Slider::new(&mut pct, BUDGET_MIN_PCT..=BUDGET_MAX_PCT)
                                .custom_formatter(|v, _| format!("{v:.0}%")),
                        );
                        if r.changed() {
                            self.sim.set_budget(d, pct);
                        }
                        lab(ui, money(ledger.spending[d.index()]), 12.0);
                        match d {
                            Dept::Roads => {
                                lab_c(ui, "UPKEEP", 11.0, Color32::from_rgb(170, 176, 196))
                            }
                            _ => {
                                let s = match d {
                                    Dept::Police => Service::Police,
                                    Dept::Fire => Service::Fire,
                                    Dept::Schools => Service::Schools,
                                    Dept::Health => Service::Health,
                                    _ => Service::Parks,
                                };
                                let v = cover[s.index()];
                                ui.horizontal(|ui| {
                                    bar(ui, 60.0, v, dept_color(d));
                                    lab(ui, format!("{:.0}%", v * 100.0), 11.0);
                                });
                            }
                        }
                        ui.end_row();
                    }
                    lab(ui, "Water", 12.0);
                    lab_c(ui, "No budget line", 10.0, Color32::from_rgb(170, 176, 196));
                    lab(ui, "In upkeep", 12.0);
                    ui.horizontal(|ui| {
                        let v = cover[Service::Water.index()];
                        bar(ui, 60.0, v, TEAL);
                        lab(ui, format!("{:.0}%", v * 100.0), 11.0);
                    });
                    ui.end_row();
                });
            ui.add_space(2.0);
            ui.separator();
            let spend: i64 = ledger.spending.iter().sum();
            let income: i64 = ledger.income.homes + ledger.income.shops + ledger.income.industry;
            let out = spend + ledger.admin + ledger.interest + ledger.principal;
            let rows = [
                ("Tax income", income, GOOD),
                ("Departments", -spend, TEXT),
                ("General upkeep", -ledger.admin, TEXT),
                ("Bond interest", -ledger.interest, TEXT),
                ("Bond principal", -ledger.principal, TEXT),
            ];
            for (name, v, col) in rows {
                ui.horizontal(|ui| {
                    lab(ui, name, 12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        lab_c(ui, money(v), 12.0, col);
                    });
                });
            }
            ui.horizontal(|ui| {
                lab(ui, "Net a month", 14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let net = income - out;
                    lab_c(ui, money(net), 14.0, if net < 0 { BAD } else { GOOD });
                });
            });
        });
        if closed {
            self.ui.budget_open = false;
        }
    }

    // ----- bonds -----

    pub(super) fn loans_ui(&mut self, ctx: &Context) {
        let mut msg: Option<(String, bool)> = None;
        let closed = window(
            ctx,
            "loans",
            "Bonds",
            400.0,
            Pos2::new(420.0, 150.0),
            |ui| {
                let debt = self.sim.debt();
                let limit = self.sim.credit_limit();
                wrapped(
                    ui,
                    format!(
                        "A bond is {} cash now. You pay {:.1} percent a year on what is left plus an equal part of it each month for {} months.",
                        money(BOND_SIZE),
                        BOND_RATE_X10 as f64 / 10.0,
                        BOND_MONTHS
                    ),
                    11.0,
                    Color32::from_rgb(170, 176, 196),
                );
                ui.add_space(4.0);
                lab(ui, format!("DEBT {}", money(debt)), 14.0);
                lab(
                    ui,
                    format!(
                        "CREDIT LIMIT {} (A FIFTH OF WHAT YOUR BUILDINGS ARE WORTH)",
                        money(limit)
                    ),
                    10.0,
                );
                bar(
                    ui,
                    360.0,
                    debt as f32 / limit.max(1) as f32,
                    if debt * 10 > limit * 8 { BAD } else { GOLD },
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let can = debt + BOND_SIZE <= limit;
                    let b = ui.add_enabled(
                        can,
                        egui::Button::new(
                            RichText::new(format!("TAKE A {} BOND", money(BOND_SIZE)))
                                .font(pixel(12.0))
                                .color(INK),
                        ),
                    );
                    if b.clicked() {
                        match self.sim.take_loan() {
                            Ok(()) => {
                                msg = Some((
                                    format!("A {} bond is in the treasury.", money(BOND_SIZE)),
                                    false,
                                ))
                            }
                            Err(LoanError::LimitReached) => {
                                msg = Some(("The town is at its credit limit.".into(), true))
                            }
                            Err(_) => {}
                        }
                    }
                    if !can {
                        b.on_hover_text("At the credit limit. Build more or pay a bond off.");
                    }
                });
                ui.add_space(2.0);
                if self.sim.loans().is_empty() {
                    lab_c(
                        ui,
                        "No bonds outstanding",
                        11.0,
                        Color32::from_rgb(170, 176, 196),
                    );
                } else {
                    egui::Grid::new("bond-grid")
                        .num_columns(4)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            for h in ["LEFT", "INTEREST", "PRINCIPAL", "MONTHS"] {
                                lab_c(ui, h, 10.0, Color32::from_rgb(170, 176, 196));
                            }
                            ui.end_row();
                            for l in self.sim.loans() {
                                let months = (l.principal_left + l.monthly_principal - 1)
                                    / l.monthly_principal.max(1);
                                lab(ui, money(l.principal_left), 12.0);
                                lab(
                                    ui,
                                    money(l.principal_left * BOND_RATE_X10 / 1000 / 12),
                                    12.0,
                                );
                                lab(ui, money(l.monthly_principal.min(l.principal_left)), 12.0);
                                lab(ui, months.to_string(), 12.0);
                                ui.end_row();
                            }
                        });
                    let first = self.sim.loans()[0].principal_left;
                    let can = self.sim.treasury >= first;
                    let b = ui.add_enabled(
                        can,
                        egui::Button::new(
                            RichText::new(format!("PAY OFF OLDEST ({})", money(first)))
                                .font(pixel(12.0))
                                .color(INK),
                        ),
                    );
                    if b.clicked() {
                        match self.sim.repay_loan() {
                            Ok(v) => {
                                msg = Some((format!("Bond paid off for {}.", money(v)), false))
                            }
                            Err(_) => msg = Some(("Not enough cash to pay it off.".into(), true)),
                        }
                    }
                    if !can {
                        b.on_hover_text("Needs that much in the treasury.");
                    }
                }
                ui.add_space(2.0);
                let ledger = self.ui.ledger;
                lab(
                    ui,
                    format!(
                        "PAID EACH MONTH {} ({} INTEREST)",
                        money(ledger.interest + ledger.principal),
                        money(ledger.interest)
                    ),
                    11.0,
                );
            },
        );
        if closed {
            self.ui.loans_open = false;
        }
        if let Some((m, bad)) = msg {
            self.toast(m, bad);
        }
    }

    // ----- building tools -----

    pub(super) fn build_ui(&mut self, ctx: &Context, screen_w: f32) {
        let mut closed = false;
        let pos = Pos2::new((screen_w - 340.0).max(10.0), 110.0);
        let c = window(ctx, "build", "Build", 320.0, pos, |ui| {
            wrapped(
                ui,
                "Pick a building, then click empty land next to a road the town already has. Esc to stop.",
                11.0,
                Color32::from_rgb(170, 176, 196),
            );
            ui.add_space(4.0);
            for k in CivicKind::ALL {
                let on = self.ui.tool == Some(k);
                let can = self.sim.treasury >= k.cost();
                ui.horizontal(|ui| {
                    let b = ui.add(
                        egui::Button::new(
                            RichText::new(k.label().to_uppercase())
                                .font(pixel(12.0))
                                .color(if on {
                                    LINE
                                } else if can {
                                    INK
                                } else {
                                    Color32::from_rgb(130, 140, 155)
                                }),
                        )
                        .fill(if on { ACCENT } else { RAISED })
                        .min_size(vec2(150.0, 26.0)),
                    );
                    if b.clicked() {
                        self.ui.tool = if on { None } else { Some(k) };
                    }
                    lab_c(ui, money(k.cost()), 12.0, if can { GOLD } else { BAD });
                });
            }
            if let Some(k) = self.ui.tool {
                ui.separator();
                lab(ui, k.label(), 14.0);
                let f = dept_fraction(&self.sim, k.service());
                lab(
                    ui,
                    format!(
                        "COSTS {} NOW, THEN ABOUT {} A MONTH",
                        money(k.cost()),
                        money(k.upkeep() * (f * 100.0) as i64 / 100)
                    ),
                    10.0,
                );
                lab(
                    ui,
                    format!(
                        "COVERS ABOUT {} M BY ROAD AT THIS FUNDING",
                        (k.service().reach_m() * (0.4 + 0.6 * f)).round() as i64
                    ),
                    10.0,
                );
                lab(ui, "TAKES 40 TO 70 DAYS TO BUILD", 10.0);
                if btn(ui, "Stop placing", false, 11.0).clicked() {
                    self.ui.tool = None;
                }
            }
        });
        closed |= c;
        if closed {
            self.ui.build_open = false;
            self.ui.tool = None;
        }
    }

    /// Ghost of the chosen building under the cursor, with its reach and whether it fits.
    pub(super) fn ghost_ui(
        &mut self,
        ctx: &Context,
        painter: &egui::Painter,
        view: &View,
        w: u32,
        h: u32,
    ) {
        let Some(kind) = self.ui.tool else { return };
        let ppp = ctx.pixels_per_point();
        let hover = ctx.pointer_hover_pos();
        let p = if let Some(p) = self.ui.fixed_ghost {
            p
        } else {
            let Some(pos) = hover else { return };
            if ctx.egui_wants_pointer_input() {
                return;
            }
            let ndc = (
                pos.x * ppp / w as f32 * 2.0 - 1.0,
                1.0 - pos.y * ppp / h as f32 * 2.0,
            );
            let Some(g) = us_render::pick_ground(view, ndc) else {
                return;
            };
            pt(g.x as f64, g.y as f64)
        };
        let pos = match (
            self.ui.fixed_ghost,
            project(view, Vec3::new(p.x as f32, p.y as f32, 0.0)),
        ) {
            (Some(_), Some(s)) => super::to_screen(s, w, h, ppp),
            _ => hover.unwrap_or(Pos2::ZERO),
        };
        let verdict = self.sim.check_civic(kind, p);
        let ok = verdict.is_ok();
        let col = if ok { GOOD } else { BAD };
        let foot = ring_points(view, p, kind.radius() as f32, w, h, ppp);
        if foot.len() > 3 {
            painter.add(egui::Shape::convex_polygon(
                foot,
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 110),
                Stroke::new(3.0, col),
            ));
        }
        let f = dept_fraction(&self.sim, kind.service());
        let reach = (kind.service().reach_m() * (0.4 + 0.6 * f)) as f32;
        let ring = ring_points(view, p, reach, w, h, ppp);
        if ring.len() > 3 {
            painter.add(egui::Shape::closed_line(ring, Stroke::new(2.0, GOLD)));
        }
        let text = match verdict {
            Ok(()) => format!("{}  {}", kind.label(), money(kind.cost())).to_uppercase(),
            Err(e) => e.text().to_uppercase(),
        };
        let at = pos + vec2(18.0, 18.0);
        let galley = painter.layout_no_wrap(text, pixel(11.0), INK);
        let r = egui::Rect::from_min_size(at, galley.size()).expand2(vec2(6.0, 3.0));
        painter.rect_filled(r, 0.0, PANEL_DARK);
        painter.rect_stroke(r, 0.0, Stroke::new(2.0, col), egui::StrokeKind::Inside);
        painter.galley(at, galley, INK);
    }

    /// A click on the map while a building tool is chosen.
    pub(super) fn place_tool(&mut self, p: Pt) {
        let Some(kind) = self.ui.tool else { return };
        match self.sim.place_civic(kind, p) {
            Ok(_) => {
                self.toast(
                    format!(
                        "Work started on a {}. It costs {}.",
                        kind.label().to_lowercase(),
                        money(kind.cost())
                    )
                    .to_uppercase(),
                    false,
                );
            }
            Err(e) => self.toast(e.text().to_uppercase(), true),
        }
    }

    /// Reach ring for a selected service building.
    pub(super) fn selection_reach(
        &self,
        painter: &egui::Painter,
        view: &View,
        w: u32,
        h: u32,
        ppp: f32,
    ) {
        let Some(Sel::Lot(i)) = self.sel else { return };
        let l = &self.sim.lots()[i];
        let svc = l.civic.map(CivicKind::service).or(match l.kind {
            us_gen::LotKind::School => Some(Service::Schools),
            us_gen::LotKind::Fire => Some(Service::Fire),
            us_gen::LotKind::Water => Some(Service::Water),
            us_gen::LotKind::Park | us_gen::LotKind::Sports => Some(Service::Parks),
            _ => None,
        });
        let Some(s) = svc else { return };
        let reach = (s.reach_m() * (0.4 + 0.6 * dept_fraction(&self.sim, s))) as f32;
        let ring = ring_points(view, l.pos, reach, w, h, ppp);
        if ring.len() > 3 {
            painter.add(egui::Shape::closed_line(ring, Stroke::new(2.0, GOLD)));
        }
    }

    // ----- why panel extras -----

    pub(super) fn why_extra(&self, ui: &mut Ui, lot: usize) {
        let l = &self.sim.lots()[lot];
        if l.phase == Phase::Hidden {
            return;
        }
        let lv = self.sim.land_value(lot);
        let word = if lv < 0.25 {
            "low"
        } else if lv < 0.5 {
            "fair"
        } else if lv < 0.75 {
            "good"
        } else {
            "high"
        };
        ui.add_space(2.0);
        lab(ui, format!("LAND VALUE {:.0}% ({word})", lv * 100.0), 12.0);
        if !l.exempt && class_of(l.kind).is_some() {
            lab(ui, format!("BUILDING LEVEL {} OF 5", l.level), 12.0);
        }
        if let Some(k) = l.civic {
            lab(
                ui,
                format!(
                    "{} COVERS ABOUT {} M BY ROAD. UPKEEP {} A MONTH AT FULL FUNDING.",
                    k.label(),
                    k.service().reach_m().round() as i64,
                    money(k.upkeep())
                ),
                10.0,
            );
        }
        if matches!(l.phase, Phase::Built { .. }) && class_of(l.kind).is_some() && !l.exempt {
            let mut parts = String::new();
            for (s, v) in &self.ui.sel_cov {
                if !parts.is_empty() {
                    parts.push_str("  ");
                }
                parts.push_str(&format!("{} {:.0}%", s.label(), v * 100.0));
            }
            if !parts.is_empty() {
                lab(ui, "COVERED BY", 10.0);
                wrapped(ui, parts, 10.0, TEXT);
            }
        }
    }

    // ----- map overlay -----

    /// What the renderer needs to tint the town for the chosen view.
    fn overlay_values(&self) -> Option<Overlay> {
        let mv = self.ui.view;
        let kind = mv.overlay_kind()?;
        let sim = &self.sim;
        let (lots, roads) = match mv {
            MapView::Normal => return None,
            MapView::Traffic => (Vec::new(), sim.road_values(MapKind::Traffic)),
            MapView::Use => {
                // the renderer numbers its categories differently from the simulation
                let v = sim.lot_values(MapKind::Use);
                (
                    v.iter()
                        .map(|&c| match c.round() as i32 {
                            1 => 0.0,
                            2 => 1.0,
                            3 => 2.0,
                            4 => 3.0,
                            5 => 5.0,
                            _ => 4.0,
                        })
                        .collect(),
                    Vec::new(),
                )
            }
            MapView::Demand(c) => {
                // own class: how much the town wants more; everything else stays at balanced
                let d = sim.class_demand(c);
                (
                    sim.lots()
                        .iter()
                        .map(|l| if class_of(l.kind) == Some(c) { d } else { 0.5 })
                        .collect(),
                    Vec::new(),
                )
            }
            other => (sim.lot_values(other.kind()?), Vec::new()),
        };
        Some(Overlay { kind, lots, roads })
    }

    pub(super) fn legend_ui(&self, ctx: &Context) {
        let mv = self.ui.view;
        if mv == MapView::Normal {
            return;
        }
        egui::Area::new(egui::Id::new("legend"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -34.0))
            .interactable(false)
            .show(ctx, |ui| {
                theme::panel()
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        lab_c(ui, mv.label(), 12.0, ACCENT);
                        ui.horizontal(|ui| {
                            for (col, name) in mv.steps() {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                ui.painter().rect_filled(r, 0.0, col);
                                ui.painter().rect_stroke(
                                    r,
                                    0.0,
                                    Stroke::new(1.0, LINE),
                                    egui::StrokeKind::Inside,
                                );
                                lab(ui, name, 10.0);
                            }
                        });
                        if let Some(note) = mv.note() {
                            lab_c(ui, note, 12.0, Color32::from_rgb(170, 176, 196));
                        }
                    });
            });
    }

    // ----- graphs -----

    pub(super) fn stats_ui(&mut self, ctx: &Context, screen_w: f32) {
        let pos = Pos2::new(((screen_w - 800.0) / 2.0).max(10.0), 96.0);
        let hist = self.sim.history();
        let keep = match self.ui.range {
            Range::Year => 12,
            Range::Five => 60,
            Range::All => usize::MAX,
        };
        let h = &hist[hist.len().saturating_sub(keep)..];
        let days: Vec<i32> = h.iter().map(|s| s.day).collect();
        let dates: Vec<(i32, u32)> = h.iter().map(|s| (s.year, s.month)).collect();
        let col = |f: &dyn Fn(&us_sim::Sample) -> f64| -> Vec<f64> { h.iter().map(f).collect() };
        let tab = self.ui.tab;
        let (series, stacked, fmt): (Vec<Series>, bool, Fmt) = match tab {
            0 => (
                vec![
                    Series {
                        name: "Population".into(),
                        color: BLUE,
                        vals: col(&|s| s.population as f64),
                    },
                    Series {
                        name: "Jobs".into(),
                        color: GOLD,
                        vals: col(&|s| s.jobs as f64),
                    },
                ],
                false,
                Fmt::Count,
            ),
            1 => (
                vec![
                    Series {
                        name: "Treasury".into(),
                        color: GOLD,
                        vals: col(&|s| s.treasury as f64),
                    },
                    Series {
                        name: "Debt".into(),
                        color: CORAL,
                        vals: col(&|s| s.debt as f64),
                    },
                ],
                false,
                Fmt::Money,
            ),
            2 => (
                Class::ALL
                    .iter()
                    .map(|&c| Series {
                        name: format!("{} tax", c.label()),
                        color: class_color(c),
                        vals: col(&|s| s.income.get(c) as f64),
                    })
                    .collect(),
                true,
                Fmt::Money,
            ),
            3 => {
                let mut v: Vec<Series> = Dept::ALL
                    .iter()
                    .map(|&d| Series {
                        name: d.label().into(),
                        color: dept_color(d),
                        vals: col(&|s| s.spending[d.index()] as f64),
                    })
                    .collect();
                v.push(Series {
                    name: "General upkeep".into(),
                    color: VIOLET,
                    vals: col(&|s| s.admin as f64),
                });
                v.push(Series {
                    name: "Bond payments".into(),
                    color: ORANGE,
                    vals: col(&|s| s.debt_service as f64),
                });
                (v, true, Fmt::Money)
            }
            4 => (
                Class::ALL
                    .iter()
                    .map(|&c| Series {
                        name: c.label().into(),
                        color: class_color(c),
                        vals: col(&|s| s.tax_x10.get(c) as f64 / 10.0),
                    })
                    .collect(),
                false,
                Fmt::Percent,
            ),
            5 => (
                Class::ALL
                    .iter()
                    .map(|&c| Series {
                        name: c.label().into(),
                        color: class_color(c),
                        vals: col(&|s| s.demand.get(c) as f64),
                    })
                    .collect(),
                false,
                Fmt::Fraction,
            ),
            6 => (
                vec![
                    Series {
                        name: "Built".into(),
                        color: TEAL,
                        vals: col(&|s| s.built as f64),
                    },
                    Series {
                        name: "Building".into(),
                        color: GOLD,
                        vals: col(&|s| s.building as f64),
                    },
                    Series {
                        name: "Empty".into(),
                        color: CORAL,
                        vals: col(&|s| s.empty as f64),
                    },
                ],
                true,
                Fmt::Count,
            ),
            7 => (
                vec![Series {
                    name: "Average land value".into(),
                    color: TEAL,
                    vals: col(&|s| s.avg_land_value as f64),
                }],
                false,
                Fmt::Fraction,
            ),
            _ => (
                vec![Series {
                    name: "Congestion".into(),
                    color: ORANGE,
                    vals: col(&|s| s.congestion as f64),
                }],
                false,
                Fmt::Fraction,
            ),
        };
        let visible: Vec<bool> = (0..series.len())
            .map(|i| !self.ui.hidden.contains(&(tab, i)))
            .collect();
        let mut toggle: Option<usize> = None;
        let mut new_tab = tab;
        let mut new_range = self.ui.range;
        let n_months = days.len();
        let screen = ctx.content_rect();
        let ww = 800.0f32.min(screen.width() - 30.0).max(240.0);
        let ch = (screen.height() - 360.0).clamp(150.0, 330.0);
        let closed = window(ctx, "stats", "Graphs", 800.0, pos, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for (i, t) in TABS.iter().enumerate() {
                    if btn(ui, t, i == tab, 11.0).clicked() {
                        new_tab = i;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                lab_c(ui, "Range", 11.0, Color32::from_rgb(170, 176, 196));
                for (r, name) in [
                    (Range::Year, "1 year"),
                    (Range::Five, "5 years"),
                    (Range::All, "All"),
                ] {
                    if btn(ui, name, new_range == r, 11.0).clicked() {
                        new_range = r;
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    lab_c(
                        ui,
                        format!("{n_months} months shown"),
                        10.0,
                        Color32::from_rgb(170, 176, 196),
                    );
                });
            });
            lab(ui, TABS[tab], 14.0);
            let size = vec2(ww - 30.0, ch);
            charts::draw(
                ui,
                size,
                &Chart {
                    series: &series,
                    visible: &visible,
                    days: &days,
                    dates: &dates,
                    stacked,
                    fmt,
                },
            );
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                for (i, s) in series.iter().enumerate() {
                    let on = visible[i];
                    let b = ui.add(
                        egui::Button::new(
                            RichText::new(s.name.to_uppercase())
                                .font(pixel(12.0))
                                .color(if on {
                                    LINE
                                } else {
                                    Color32::from_rgb(130, 140, 155)
                                }),
                        )
                        .fill(if on { s.color } else { RAISED })
                        .stroke(Stroke::new(2.0, s.color)),
                    );
                    if b.on_hover_text("Show or hide").clicked() {
                        toggle = Some(i);
                    }
                }
            });
            if tab == 8 && series[0].vals.iter().all(|&v| v == 0.0) {
                lab_c(
                    ui,
                    "No traffic has been counted yet.",
                    10.0,
                    Color32::from_rgb(170, 176, 196),
                );
            }
        });
        self.ui.tab = new_tab;
        self.ui.range = new_range;
        if let Some(i) = toggle
            && !self.ui.hidden.remove(&(tab, i))
        {
            self.ui.hidden.insert((tab, i));
        }
        if closed {
            self.ui.stats_open = false;
        }
    }

    // ----- news -----

    pub(super) fn news_ui(&mut self, ctx: &Context, screen_w: f32) {
        let pos = Pos2::new((screen_w - 420.0).max(10.0), 130.0);
        let mut fly: Option<Pt> = None;
        let closed = window(ctx, "news", "News", 380.0, pos, |ui| {
            if self.ui.events.is_empty() {
                lab_c(
                    ui,
                    "Nothing yet. News appears as the town grows.",
                    11.0,
                    Color32::from_rgb(170, 176, 196),
                );
            }
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for e in self.ui.events.iter().rev() {
                        let (tag, col) = match e.kind {
                            EventKind::Neighborhood => ("Growth", BLUE),
                            EventKind::Milestone => ("Milestone", GOLD),
                            EventKind::Budget => ("Budget", ORANGE),
                            EventKind::Civic => ("Civic", TEAL),
                            EventKind::Loan => ("Bond", VIOLET),
                            EventKind::Upgrade => ("Upgrade", LIME),
                            EventKind::Road => ("Road", GREY),
                            EventKind::Fire => ("Fire", CORAL),
                            EventKind::Rival => ("Rival", ORANGE),
                            EventKind::Decline => ("Decline", CORAL),
                        };
                        ui.horizontal(|ui| {
                            lab_c(
                                ui,
                                Date::from_days(e.day).text(),
                                10.0,
                                Color32::from_rgb(170, 176, 196),
                            );
                            lab_c(ui, tag, 10.0, col);
                        });
                        let r = ui.add(
                            egui::Label::new(RichText::new(&e.text).size(14.0).color(TEXT))
                                .wrap()
                                .sense(if e.place.is_some() {
                                    Sense::click()
                                } else {
                                    Sense::hover()
                                }),
                        );
                        if let Some(p) = e.place {
                            let r = r
                                .on_hover_cursor(egui::CursorIcon::PointingHand)
                                .on_hover_text("Click to look there");
                            if r.clicked() {
                                fly = Some(p);
                            }
                        }
                        ui.separator();
                    }
                });
        });
        if let Some(p) = fly {
            self.cam
                .look_at(Vec3::new(p.x as f32, p.y as f32, 0.0), 260.0);
        }
        if closed {
            self.ui.news_open = false;
        }
    }
}
