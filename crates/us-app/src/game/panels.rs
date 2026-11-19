//! The mayor's screens: tax levers, budget, bonds, civic building tools, graphs, map views and
//! the news feed. Everything here reads and changes the simulation through its public API.

use super::charts::{self, Chart, Fmt, Series};
use super::{Game, Sel, money};
use crate::theme::{
    self, ACCENT, BAD, EDGE, GOOD, INK, LINE, PANEL_DARK, RAISED, TEXT, heading, ui_font,
};
use crate::world::LiveScene;
use egui::{Align2, Color32, Context, Pos2, RichText, Sense, Stroke, Ui, vec2};
use glam::{Vec2, Vec3};
use us_gen::{Pt, RoadClass, pt};
use us_render::{Overlay, OverlayKind, View, project};
use us_sim::{
    ABANDON_ABOVE_X10, BOND_MONTHS, BOND_RATE_X10, BOND_SIZE, BUDGET_MAX_PCT, BUDGET_MIN_PCT,
    CivicKind, Class, Date, Dept, Event, EventKind, LedgerNow, LoanError, MapKind, Phase,
    PlanningUse, RoadError, Service, TAX_MAX_X10, TAX_MIN_X10, class_of,
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

fn budget_service(department: Dept) -> Option<Service> {
    match department {
        Dept::Police => Some(Service::Police),
        Dept::Fire => Some(Service::Fire),
        Dept::Schools => Some(Service::Schools),
        Dept::Health => Some(Service::Health),
        Dept::Parks => Some(Service::Parks),
        Dept::Roads => None,
    }
}

pub(super) fn lab(ui: &mut Ui, text: impl AsRef<str>, size: f32) {
    let size = size.max(12.0);
    ui.label(RichText::new(text.as_ref()).font(ui_font(size)).color(TEXT));
}

pub(super) fn lab_c(ui: &mut Ui, text: impl AsRef<str>, size: f32, col: Color32) {
    let size = size.max(12.0);
    ui.label(RichText::new(text.as_ref()).font(ui_font(size)).color(col));
}

pub(super) fn wrapped(ui: &mut Ui, text: impl AsRef<str>, size: f32, col: Color32) {
    let size = size.max(12.0);
    ui.add(egui::Label::new(RichText::new(text.as_ref()).font(ui_font(size)).color(col)).wrap());
}

pub(super) fn btn(ui: &mut Ui, text: &str, on: bool, size: f32) -> egui::Response {
    let size = size.max(12.0);
    ui.add(
        egui::Button::new(RichText::new(text).font(ui_font(size)).color(if on {
            LINE
        } else {
            INK
        }))
        .fill(if on { ACCENT } else { RAISED }),
    )
}

/// A solid horizontal bar from 0 to 1.
fn bar(ui: &mut Ui, width: f32, frac: f32, col: Color32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 12.0), Sense::hover());
    let frac = theme::ease(ui.ctx(), (resp.id, "bar"), frac.clamp(0.0, 1.0), 0.25);
    let p = ui.painter();
    p.rect_filled(rect, 3.0, EDGE);
    let inner = rect.shrink(2.0);
    p.rect_filled(inner, 3.0, PANEL_DARK);
    let mut f = inner;
    f.set_width(inner.width() * frac.clamp(0.0, 1.0));
    p.rect_filled(f, 3.0, col);
}

/// Height the bottom bar takes on a phone, so sheets and cards sit above it.
pub const DOCK_H: f32 = 52.0;

/// Use two rows only when large UI scaling leaves no room for five touch targets.
pub fn dock_rows(ctx: &Context) -> usize {
    if ctx.content_rect().width() < 248.0 {
        2
    } else {
        1
    }
}

/// Remember where the phone's bottom bar starts this frame (its real top edge, after the home
/// bar inset and however many rows it has), so sheets can sit just above it.
pub fn set_dock_top(ctx: &Context, y: f32) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dock_top"), y));
}

/// Top edge of the phone's bottom bar, or an estimate before it has been laid out.
pub(super) fn dock_top(ctx: &Context) -> f32 {
    let screen = ctx.content_rect();
    ctx.data(|d| d.get_temp::<f32>(egui::Id::new("dock_top")))
        .filter(|y| *y > screen.top() && *y <= screen.bottom())
        .unwrap_or(screen.bottom() - DOCK_H * dock_rows(ctx) as f32)
}

pub(super) fn set_hud_bottom(ctx: &Context, y: f32) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("hud_bottom"), y));
}

/// The icon a window or sheet shows beside its title.
fn title_icon(title: &str) -> Option<&'static str> {
    Some(match title.to_lowercase().as_str() {
        "taxes" => theme::icon::TAXES,
        "budget" => theme::icon::BUDGET,
        "bonds" => theme::icon::BONDS,
        "build" => theme::icon::HAMMER,
        "graphs" => theme::icon::GRAPHS,
        "news" => theme::icon::NEWS,
        "voters" => theme::icon::VOTERS,
        "rivals" => theme::icon::RIVALS,
        "map view" => theme::icon::MAP,
        "districts" => theme::icon::DISTRICTS,
        "ordinances" => theme::icon::ORDINANCES,
        _ => return None,
    })
}

/// A title row: icon, pixel heading.
fn title_row(ui: &mut Ui, title: &str) {
    if let Some(g) = title_icon(title) {
        ui.label(theme::icon_text(g, 20.0, ACCENT));
    }
    ui.label(heading(title, 24.0));
}

/// Fade out on close: the first click starts it, the close is reported when it has finished.
/// Returns the opacity to draw with and whether the close is done.
fn closing(ctx: &Context, id: &str, clicked: bool) -> (f32, bool) {
    let key = egui::Id::new(("closing", id));
    let started = ctx.data(|d| d.get_temp::<bool>(key)).unwrap_or(false) || clicked;
    if started {
        ctx.data_mut(|d| d.insert_temp(key, true));
    }
    let a = ctx.animate_bool_with_time_and_easing(
        egui::Id::new(("closing-a", id)),
        !started,
        theme::dur(0.14),
        egui::emath::easing::cubic_out,
    );
    let done = started && a <= 0.001;
    if done {
        ctx.data_mut(|d| d.remove::<bool>(key));
    }
    (a, done)
}

/// A phone sheet sits between the HUD and dock. Each panel owns its own scroll
/// position, and the header stays accessible while the body scrolls.
pub(super) fn sheet(ctx: &Context, title: &str, body: impl FnOnce(&mut Ui)) -> bool {
    let screen = ctx.content_rect();
    let bottom = dock_top(ctx) - 4.0;
    let hud = ctx
        .data(|d| d.get_temp::<f32>(egui::Id::new("hud_bottom")))
        .unwrap_or(screen.top());
    let top = (hud + 4.0).min(bottom - 138.0).max(screen.top());
    let bounds = egui::Rect::from_min_max(
        egui::pos2(screen.left() + 6.0, top),
        egui::pos2(screen.right() - 6.0, bottom),
    );
    let width = bounds.width().min(640.0);
    let mut closed = false;
    egui::Area::new(egui::Id::new(("sheet", title)))
        .pivot(Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(bounds.center().x, bottom))
        .constrain_to(bounds)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            ui.set_clip_rect(ui.clip_rect().intersect(bounds));
            theme::panel()
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    let content_width = width - 18.0;
                    ui.set_width(content_width);
                    ui.horizontal(|ui| {
                        let title_width =
                            (content_width - theme::TOUCH - ui.spacing().item_spacing.x).max(10.0);
                        ui.allocate_ui_with_layout(
                            vec2(title_width, theme::TOUCH),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_width(title_width);
                                if let Some(icon) = title_icon(title) {
                                    ui.label(theme::icon_text(icon, 18.0, ACCENT));
                                }
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(title)
                                            .font(ui_font(18.0))
                                            .strong()
                                            .color(INK),
                                    )
                                    .truncate(),
                                );
                            },
                        );
                        closed = ui
                            .add_sized(
                                [theme::TOUCH, theme::TOUCH],
                                egui::Button::new(theme::icon_text(theme::icon::CLOSE, 20.0, INK)),
                            )
                            .on_hover_text("Close")
                            .clicked();
                    });
                    ui.separator();
                    let max_height = (bounds.height() - theme::TOUCH - 37.0).max(1.0);
                    egui::ScrollArea::vertical()
                        .max_height(max_height)
                        .min_scrolled_height(max_height.min(150.0))
                        .auto_shrink([false, true])
                        .show(ui, body);
                });
        });
    closed
}

/// The frame and header every movable window shares. Returns true when closed (after a short
/// fade). Windows fade in when they open.
pub(super) fn window(
    ctx: &Context,
    id: &str,
    title: &str,
    width: f32,
    pos: Pos2,
    body: impl FnOnce(&mut Ui),
) -> bool {
    if theme::is_compact(ctx) {
        return sheet(ctx, title, body);
    }
    let mut clicked = false;
    // at a large UI scale the screen has fewer points: shrink and scroll rather than overflow
    let screen = ctx.content_rect();
    let width = width
        .min(screen.width() - 30.0 - super::chrome::RAIL_W)
        .max(240.0);
    let max_h = (screen.height() - 150.0).max(160.0);
    // first placement: right of the rail, left of the inspector
    let lo = super::chrome::RAIL_W + 12.0;
    let hi = (screen.right() - width - super::chrome::INSPECTOR_W - 24.0).max(lo);
    let pos = Pos2::new(pos.x.clamp(lo, hi), pos.y);
    let (fade, done) = closing(ctx, id, false);
    egui::Window::new(id)
        .title_bar(false)
        .frame(theme::panel().multiply_with_opacity(fade))
        .resizable(false)
        .default_pos(pos)
        .default_width(width)
        .constrain(true)
        .show(ctx, |ui| {
            ui.set_opacity(fade);
            ui.set_width(width);
            ui.horizontal(|ui| {
                title_row(ui, title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let b = egui::Button::new(theme::icon_text(theme::icon::CLOSE, 17.0, INK))
                        .min_size(vec2(30.0, 28.0));
                    if ui.add(b).on_hover_text("Close").clicked() {
                        clicked = true;
                    }
                });
            });
            ui.add_space(2.0);
            egui::ScrollArea::vertical()
                .max_height(max_h)
                .min_scrolled_height(max_h.min(560.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.set_width(width - 14.0);
                    body(ui);
                });
        });
    if clicked {
        closing(ctx, id, true);
    }
    done
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
    District,
}

impl MapView {
    pub const ALL: [MapView; 15] = [
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
        MapView::District,
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
            MapView::District => "Districts".into(),
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
            MapView::District => return None,
        })
    }

    /// One line under the legend where the colours need explaining.
    fn note(self) -> Option<&'static str> {
        match self {
            MapView::Demand(_) => Some("Other kinds of building stay grey"),
            MapView::Level => Some("Empty land shows as level 1"),
            MapView::District => Some("Grey land is in no district"),
            MapView::LandValue => {
                Some("Lots near an open depot and industry beside the railway are worth more")
            }
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
            MapView::District => OverlayKind::District,
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
            MapView::District => &[],
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

/// What the road tool builds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RoadMode {
    Local,
    Avenue,
    Collector,
    /// Widen a street to the next class.
    Upgrade,
}

impl RoadMode {
    fn class(self) -> RoadClass {
        match self {
            RoadMode::Avenue => RoadClass::Arterial,
            RoadMode::Local => RoadClass::Local,
            _ => RoadClass::Collector,
        }
    }
}

/// Everything the mayor's screens remember during a session.
pub struct UiState {
    pub levers_open: bool,
    pub budget_open: bool,
    pub loans_open: bool,
    pub build_open: bool,
    pub stats_open: bool,
    pub news_open: bool,
    pub(super) more_open: bool,
    pub rivals_open: bool,
    pub districts_open: bool,
    pub ordinances_open: bool,
    /// The Tram window (see `trams.rs`).
    pub(super) trams: super::trams::TramUi,
    /// The growth banner and the goal card (see `growth_banner.rs` and `goals.rs`).
    pub(super) growth: super::growth_banner::GrowthUi,
    pub(super) goals: super::goals::GoalsUi,
    /// The map view switcher is open (desktop).
    pub views_open: bool,
    /// Where the Map views button sits, so the switcher opens beside it.
    pub views_rect_y: f32,
    /// The district the paint tool works on, while it is on.
    pub paint: Option<usize>,
    /// The district chosen in the Districts window.
    pub dist_sel: Option<usize>,
    pub dist_stats: Vec<Option<us_sim::DistrictStats>>,
    pub dist_stats_age: f32,
    pub tool: Option<CivicKind>,
    pub(super) civic_site: Option<Pt>,
    /// The road tool, when chosen (only live while the Build window is open).
    pub road: Option<RoadMode>,
    /// Points of the road being drawn.
    pub road_pts: Vec<Pt>,
    /// Zoning land use and the first corner of the current area.
    pub(super) zone: Option<(PlanningUse, Option<Pt>)>,
    /// Screenshot hook: hold the ghost at this spot instead of under the cursor.
    fixed_ghost: Option<Pt>,
    pub view: MapView,
    tab: usize,
    range: Range,
    hidden: std::collections::HashSet<(usize, usize)>,
    pub events: Vec<Event>,
    pub(super) news_seen: usize,
    ledger: LedgerNow,
    /// Seconds since the view was last pushed to the renderer, and which view it showed.
    ov_age: f32,
    ov_view: MapView,
    /// Seconds since the trains were last told what the town offers.
    rail_age: f32,
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
            levers_open: false,
            budget_open: false,
            loans_open: false,
            build_open: false,
            stats_open: false,
            news_open: false,
            more_open: false,
            rivals_open: false,
            districts_open: false,
            ordinances_open: false,
            trams: Default::default(),
            growth: Default::default(),
            goals: Default::default(),
            views_open: false,
            views_rect_y: 300.0,
            paint: None,
            dist_sel: None,
            dist_stats: Vec::new(),
            dist_stats_age: 9.0,
            tool: None,
            civic_site: None,
            road: None,
            road_pts: Vec::new(),
            zone: None,
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
            rail_age: 1.0,
            cover: [0.0; 6],
            cover_age: 9.0,
            sel_cov: Vec::new(),
            sel_cov_for: None,
            sel_cov_age: 9.0,
        }
    }
}

fn ring_points(
    live: &LiveScene,
    view: &View,
    c: Pt,
    r: f32,
    w: u32,
    h: u32,
    ppp: f32,
) -> Vec<Pos2> {
    let mut out = Vec::with_capacity(48);
    for i in 0..48 {
        let a = i as f32 / 48.0 * std::f32::consts::TAU;
        let (x, y) = (c.x as f32 + r * a.cos(), c.y as f32 + r * a.sin());
        let p = Vec3::new(x, y, live.ground_at(x, y) + 0.3);
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
        self.refresh_district_stats(dt);
        self.ui.rail_age += dt;
        if self.ui.rail_age > 1.0 {
            self.ui.rail_age = 0.0;
            self.live.set_rail_service(&self.sim);
        }
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
        if code == K::KeyV {
            self.toggle_voters();
            return true;
        }
        if matches!(code, K::Enter | K::NumpadEnter) && self.ui.build_open && self.ui.road.is_some()
        {
            self.road_confirm();
            return true;
        }
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
            K::KeyR => u.rivals_open = !u.rivals_open,
            K::KeyF => u.trams.open = !u.trams.open,
            K::KeyI => {
                u.districts_open = !u.districts_open;
                if !u.districts_open {
                    u.paint = None;
                }
            }
            K::KeyH => u.ordinances_open = !u.ordinances_open,
            K::KeyZ => u.view = MapView::District,
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
            K::KeyY => {
                u.build_open = true;
                u.tool = None;
                u.road = if u.road.is_some() {
                    None
                } else {
                    Some(RoadMode::Avenue)
                };
                u.road_pts.clear();
            }
            K::Escape if u.build_open && u.road.is_some() => {
                if u.road_pts.is_empty() {
                    u.road = None;
                } else {
                    u.road_pts.clear();
                }
            }
            K::Escape if u.tool.is_some() => {
                u.tool = None;
                u.civic_site = None;
            }
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
                "clean" => {
                    // a clear picture: no panels and no goal card
                    self.close_sheets();
                    self.sim.dismiss_goals();
                    self.ui.goals.open = false;
                    // and no election night over the picture; the game is held
                    self.speed = 0;
                    self.election = None;
                    if let Some(r) = self.sim.last_election() {
                        self.election_seen = r.day;
                    }
                }
                "low" => self.cam.set_angle(n as f32, 18.0),
                "tilt" => self.cam.set_tilt(n as f32),
                "pan" => {
                    // look at a point east and north of the town hall in metres, `pan:E:N`
                    // (a leading `m` makes a number negative), from 330 m
                    let num = |t: &str| {
                        t.strip_prefix('m').map_or_else(
                            || t.parse::<f32>().ok(),
                            |r| r.parse::<f32>().ok().map(|x| -x),
                        )
                    };
                    if let Some((e, nn)) = v.split_once(':')
                        && let (Some(e), Some(nn)) = (num(e), num(nn))
                    {
                        let h = self.town.hall;
                        self.cam
                            .look_at(Vec2::new(h.x as f32 + e, h.y as f32 + nn), 330.0);
                    }
                }
                "storm" => {
                    for _ in 0..365 {
                        if self.sim.storm() {
                            break;
                        }
                        self.sim.step_day();
                    }
                    eprintln!(
                        "smoke: weather {:?} on day {}",
                        self.sim.sky_now(),
                        self.sim.day
                    );
                    self.hour = 16.8;
                    self.hour_frozen = true;
                    let hall = self.live.scene.hall.truncate();
                    let distance = if n == 0 { 1320.0 } else { n.max(70) as f32 };
                    self.cam.look_at(hall, distance);
                    self.cam.set_angle(41.0, 26.0);
                }
                "levers" => self.ui.levers_open = true,
                "budget" => self.ui.budget_open = true,
                "loans" => self.ui.loans_open = true,
                "build" => {
                    self.ui.build_open = true;
                    if n > 6 {
                        self.sim.unlock_all();
                    }
                    self.ui.tool = CivicKind::ALL.get(n).copied();
                }
                "stats" => {
                    self.ui.stats_open = true;
                    self.ui.tab = n.min(TABS.len() - 1);
                }
                "whole" => {
                    let c = self.town.size as f32 * 0.5;
                    self.cam
                        .look_at(Vec2::new(c, c), self.town.size as f32 * 1.5);
                }
                "rock" | "harbour" => {
                    // the tallest rock of a red rock town, or the harbour cove of a coastal one
                    let at = match self.town.terrain.rock_coast() {
                        us_gen::RockCoast::Mesa(m) => m
                            .formations
                            .iter()
                            .max_by(|a, b| a.top.total_cmp(&b.top))
                            .map(|f| f.c),
                        us_gen::RockCoast::Coast(c) => {
                            let v = c.coves[0];
                            Some(us_gen::pt(v.c.x - 0.75 * v.rho, v.c.y))
                        }
                        _ => None,
                    };
                    if let Some(p) = at {
                        let d = if k == "rock" { 1100.0 } else { 1200.0 };
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), d);
                    }
                }
                "bridge" => {
                    // a road point over water, for a screenshot
                    let land = self.live.land();
                    let found = self.sim.road_runs().iter().find_map(|r| {
                        r.points
                            .iter()
                            .copied()
                            .find(|&p| land.water_at(p).is_some())
                    });
                    if let Some(p) = found {
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 420.0);
                    }
                }
                "hill" => {
                    // the steepest built lot, for a screenshot
                    let land = self.live.land();
                    let best = self
                        .sim
                        .lots()
                        .iter()
                        .filter(|l| matches!(l.phase, Phase::Built { .. }))
                        .map(|l| (land.slope(l.pos.x as f32, l.pos.y as f32), l.pos))
                        .filter(|(sl, _)| sl.is_finite())
                        .max_by(|a, b| a.0.total_cmp(&b.0));
                    if let Some((_, p)) = best {
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 420.0);
                    }
                }
                "slope" => {
                    // a placement ghost on the steepest vacant lot where the building fits
                    let kind = self.ui.tool.unwrap_or(CivicKind::School);
                    let land = self.live.land();
                    let best = self
                        .sim
                        .lots()
                        .iter()
                        .filter(|l| l.phase == Phase::Vacant)
                        .map(|l| l.pos)
                        .filter(|&p| self.sim.check_civic(kind, p).is_ok())
                        .map(|p| (land.slope(p.x as f32, p.y as f32), p))
                        .filter(|(sl, _)| sl.is_finite())
                        .max_by(|a, b| a.0.total_cmp(&b.0));
                    if let Some((_, p)) = best {
                        self.ui.fixed_ghost = Some(p);
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 420.0);
                    }
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
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 1500.0);
                    }
                }
                "civic" => {
                    self.sim.unlock_all();
                    let kind = CivicKind::ALL[n.min(CivicKind::ALL.len() - 1)];
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
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 1500.0);
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
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 650.0);
                    }
                }
                "year" => self.ui.range = Range::Year,
                "five" => self.ui.range = Range::Five,
                "news" => self.ui.news_open = true,
                "view" => self.ui.view = MapView::ALL[n.min(MapView::ALL.len() - 1)],
                "bond" => {
                    let _ = self.sim.take_loan();
                }
                "hour" => {
                    self.hour = n as f32;
                    self.hour_frozen = true;
                }
                "passage" => {
                    // a hand-placed public passage `n` metres east of the town hall, for pictures
                    let h = self.town.hall;
                    let (x, y) = (h.x as f32 + n as f32, h.y as f32);
                    let (cx, cy) = self.live.add_test_passage(x, y);
                    self.cam.look_at(Vec2::new(cx, cy), 200.0);
                }
                "pause" => self.speed = 0,
                "town" => {
                    // the town hall from `n` metres away, for a screenshot
                    let h = self.town.hall;
                    self.cam
                        .look_at(Vec2::new(h.x as f32, h.y as f32), n.max(300) as f32);
                }
                "zoom" => {
                    if let Some(Sel::Lot(i)) = self.sel {
                        let p = self.sim.lots()[i].pos;
                        self.cam
                            .look_at(Vec2::new(p.x as f32, p.y as f32), n as f32);
                    }
                }
                "upgrade" => {
                    for _ in 0..3000 {
                        if self
                            .sim
                            .lots()
                            .iter()
                            .any(|l| matches!(l.phase, Phase::Building { .. }) && l.next_level > 0)
                        {
                            break;
                        }
                        self.sim.step_day();
                    }
                    let found = self
                        .sim
                        .lots()
                        .iter()
                        .find(|l| matches!(l.phase, Phase::Building { .. }) && l.next_level > 0)
                        .map(|l| l.pos);
                    if let Some(p) = found {
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 220.0);
                    }
                }
                "burned" => {
                    for _ in 0..3000 {
                        if self.sim.lots().iter().any(|l| l.fire.is_some()) {
                            break;
                        }
                        self.sim.step_day();
                    }
                    let found = self
                        .sim
                        .lots()
                        .iter()
                        .find(|l| l.fire.is_some())
                        .map(|l| l.pos);
                    if let Some(p) = found {
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 220.0);
                    }
                }
                "works" => {
                    for _ in 0..3000 {
                        self.live.sync(&mut self.sim, 1.0e9);
                        if self.live.works_place(&self.sim).is_some() {
                            break;
                        }
                        self.sim.step_day();
                    }
                    if let Some(p) = self.live.works_place(&self.sim) {
                        self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 260.0);
                    }
                }
                "districts" | "ordinances" => {
                    // two districts over the first neighbourhoods, with a few ordinances on
                    if self.sim.districts().is_empty() {
                        for _ in 0..2 {
                            self.sim.create_district();
                        }
                        for (k, a) in (0..self.town.additions.len()).take(6).enumerate() {
                            self.sim.toggle_addition(k % 2, a);
                        }
                        self.sim
                            .set_ordinance_district(0, us_sim::Ordinance::Heritage, true);
                        self.sim
                            .set_ordinance_town(us_sim::Ordinance::Recycling, true);
                    }
                    if k == "districts" {
                        self.ui.districts_open = true;
                        self.ui.view = MapView::District;
                    } else {
                        self.ui.ordinances_open = true;
                    }
                }
                "nolevers" => self.ui.levers_open = false,
                "voters" => self.voters_open = true,
                "rivals" => self.ui.rivals_open = true,
                "trams" => self.smoke_trams(),
                "noelection" => {
                    self.election_seen = self.sim.last_election().map_or(-1, |e| e.day);
                }
                "map" => self.map_open = true,
                "views" => self.ui.views_open = true,
                "pausemenu" => self.menu_open = true,
                "saveas" => {
                    self.menu_open = true;
                    self.save_as = Some(self.name.clone());
                }
                "poor" => {
                    // run a town with no services until the first vote, for the election screens
                    for _ in 0..n.max(1) {
                        self.sim.step_day();
                    }
                }
                _ => {}
            }
        }
    }

    // ----- top bar extras -----

    /// Population, treasury and monthly net as small lines. Needs room, so it checks the width.
    pub(super) fn sparks_ui(&self, ui: &mut Ui) {
        // three small charts need about 200 points; leave them out rather than crowd the bar
        if ui.available_width() < 220.0 {
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
        let closed = window(
            ctx,
            "taxes",
            "Taxes",
            320.0,
            Pos2::new(178.0, 125.0),
            |ui| {
                self.levers_body(ui);
            },
        );
        if closed {
            self.ui.levers_open = false;
        }
    }

    fn levers_body(&mut self, ui: &mut Ui) {
        let ledger = self.ui.ledger;
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
                    .map_or(0, |s| s.income.get(c) * 10 / s.tax_x10.get(c) as i64)
            };
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                ui.painter().rect_filled(r, 3.0, class_color(c));
                lab(ui, c.label(), 14.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let d = self.sim.class_demand(c);
                    lab_c(
                        ui,
                        format!("Demand {:.0}%", d * 100.0),
                        11.0,
                        if d < 0.3 {
                            BAD
                        } else if d < 0.55 {
                            GOLD
                        } else {
                            GOOD
                        },
                    );
                });
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    lab(ui, format!("{}/mo", money(income)), 12.0);
                });
            });
            lab_c(
                ui,
                format!("Each point is about {} a month", money(per_point)),
                12.0,
                Color32::from_rgb(170, 176, 196),
            );
            ui.add_space(2.0);
        }
        ui.separator();
        let net = self.stats.income - self.stats.expense;
        lab(
            ui,
            format!(
                "Tax in {}   costs {}",
                money(self.stats.income),
                money(self.stats.expense)
            ),
            12.0,
        );
        lab_c(
            ui,
            format!(
                "Net {}{} a month",
                if net < 0 { "" } else { "+" },
                money(net)
            ),
            12.0,
            if net < 0 { BAD } else { GOOD },
        );
        lab(
            ui,
            format!(
                "Built {}  building {}  empty {}",
                self.stats.built, self.stats.building, self.stats.empty
            ),
            11.0,
        );
    }

    // ----- budget -----

    pub(super) fn budget_ui(&mut self, ctx: &Context) {
        let ledger = self.ui.ledger;
        let cover = self.ui.cover;
        let pos = Pos2::new(360.0, 100.0);
        let closed = window(ctx, "budget", "Budget", 560.0, pos, |ui| {
            super::chrome::section(ui, "Service capacity");
            self.capacity_rows(ui);
            ui.separator();
            wrapped(
                ui,
                "Funding changes service reach. Low road upkeep lowers land value.",
                11.0,
                Color32::from_rgb(170, 176, 196),
            );
            ui.add_space(4.0);
            if self.compact {
                for d in Dept::ALL {
                    ui.push_id(d.index(), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            lab_c(ui, d.label(), 14.0, dept_color(d));
                            lab(
                                ui,
                                format!("{}/mo", money(ledger.spending[d.index()])),
                                12.0,
                            );
                        });
                        self.funding_slider(ui, d);
                        if let Some(service) = budget_service(d) {
                            lab(
                                ui,
                                format!("{:.0}% coverage", cover[service.index()] * 100.0),
                                12.0,
                            );
                        }
                        ui.separator();
                    });
                }
                lab(
                    ui,
                    format!(
                        "Water coverage {:.0}%",
                        cover[Service::Water.index()] * 100.0
                    ),
                    12.0,
                );
            } else {
                egui::Grid::new("budget-grid")
                    .num_columns(4)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        for h in ["Department", "Funding", "Cost a month", "Coverage"] {
                            lab_c(ui, h, 10.0, Color32::from_rgb(170, 176, 196));
                        }
                        ui.end_row();
                        for d in Dept::ALL {
                            ui.horizontal(|ui| {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                                ui.painter().rect_filled(r, 3.0, dept_color(d));
                                lab(ui, d.label(), 12.0);
                            });
                            self.funding_slider(ui, d);
                            lab(ui, money(ledger.spending[d.index()]), 12.0);
                            if let Some(service) = budget_service(d) {
                                let v = cover[service.index()];
                                ui.horizontal(|ui| {
                                    bar(ui, 60.0, v, dept_color(d));
                                    lab(ui, format!("{:.0}%", v * 100.0), 11.0);
                                });
                            } else {
                                lab_c(ui, "Upkeep", 11.0, crate::theme::MUTED);
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
            }
            ui.add_space(2.0);
            ui.separator();
            let spend: i64 = ledger.spending.iter().sum();
            let income: i64 = ledger.income.homes + ledger.income.shops + ledger.income.industry;
            let out = spend + ledger.admin + ledger.interest + ledger.principal + ledger.ordinances;
            let rows = [
                ("Tax income", income, GOOD),
                ("Departments", -spend, TEXT),
                ("General upkeep", -ledger.admin, TEXT),
                ("Ordinances", -ledger.ordinances, TEXT),
                ("Bond interest", -ledger.interest, TEXT),
                ("Bond principal", -ledger.principal, TEXT),
            ];
            if ledger.grants > 0 {
                // a goal reward, paid into the treasury with the last monthly ledger
                ui.horizontal(|ui| {
                    lab(ui, "Goal grant, paid last month", 12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        lab_c(ui, money(ledger.grants), 12.0, GOOD);
                    });
                });
            }
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

    fn funding_slider(&mut self, ui: &mut Ui, department: Dept) {
        let mut pct = self.sim.budget_pct(department);
        let slider = egui::Slider::new(&mut pct, BUDGET_MIN_PCT..=BUDGET_MAX_PCT)
            .custom_formatter(|value, _| format!("{value:.0}%"));
        if ui.add(slider).changed() {
            self.sim.set_budget(department, pct);
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
                lab(ui, format!("Debt {}", money(debt)), 14.0);
                lab(
                    ui,
                    format!(
                        "Credit limit {} (a fifth of what your buildings are worth)",
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
                            RichText::new(format!("Take a {} bond", money(BOND_SIZE)))
                                .font(ui_font(12.0))
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
                            for h in ["Left", "Interest", "Principal", "Months"] {
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
                    let can = self.sim.can_afford(first);
                    let b = ui.add_enabled(
                        can,
                        egui::Button::new(
                            RichText::new(format!("Pay off oldest ({})", money(first)))
                                .font(ui_font(12.0))
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
                        "Paid each month {} ({} interest)",
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
        let mut picked = None;
        let mut zone = None;
        let compact = theme::is_compact(ctx);
        let pos = Pos2::new((screen_w - 340.0).max(10.0), 110.0);
        let closed = window(ctx, "build", "Build", 320.0, pos, |ui| {
            wrapped(
                ui,
                if compact {
                    "Pick a building, then tap near an open street to preview its roadside site."
                } else {
                    "Pick a building, then click near an open street. Press Esc to stop."
                },
                11.0,
                Color32::from_rgb(170, 176, 196),
            );
            ui.add_space(4.0);
            lab(ui, "Roads", 12.0);
            ui.horizontal_wrapped(|ui| {
                for (m, name) in [
                    (RoadMode::Local, "Local"),
                    (RoadMode::Avenue, "Avenue"),
                    (RoadMode::Collector, "Collector"),
                    (RoadMode::Upgrade, "Upgrade"),
                ] {
                    if m == RoadMode::Local && !self.rules.city_planning {
                        continue;
                    }
                    let on = self.ui.road == Some(m);
                    if btn(ui, name, on, 11.0).clicked() {
                        self.ui.road = if on { None } else { Some(m) };
                        self.ui.tool = None;
                        self.ui.road_pts.clear();
                        self.ui.zone = None;
                    }
                }
            });
            self.road_panel(ui);
            if self.rules.city_planning {
                ui.add_space(6.0);
                lab(ui, "Zoning", 12.0);
                ui.horizontal_wrapped(|ui| {
                    for kind in PlanningUse::ALL {
                        let on = self.ui.zone.is_some_and(|(active, _)| active == kind);
                        if btn(ui, kind.label(), on, 11.0).clicked() {
                            zone = Some(kind);
                        }
                    }
                });
            }
            ui.add_space(6.0);
            lab(ui, "Buildings", 12.0);
            self.capacity_rows(ui);
            ui.add_space(4.0);
            for k in CivicKind::ALL {
                let on = self.ui.tool == Some(k);
                if !self.sim.is_unlocked(k) {
                    // locked: shown greyed, with what it takes
                    ui.horizontal(|ui| {
                        ui.add_enabled(
                            false,
                            egui::Button::new(
                                RichText::new(k.label().to_string())
                                    .font(ui_font(12.0))
                                    .color(Color32::from_rgb(110, 118, 132)),
                            )
                            .fill(RAISED)
                            .min_size(vec2(150.0, 26.0)),
                        );
                        lab_c(
                            ui,
                            format!("Unlocks at {} people", charts::group(k.unlock_at())),
                            11.0,
                            Color32::from_rgb(130, 140, 155),
                        );
                    });
                    continue;
                }
                let can = self.sim.can_afford(k.cost());
                ui.horizontal(|ui| {
                    let b = ui.add(
                        egui::Button::new(
                            RichText::new(k.label().to_string())
                                .font(ui_font(12.0))
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
                        if on {
                            self.ui.tool = None;
                        } else {
                            picked = Some(k);
                        }
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
                        "Costs {} now, then about {} a month",
                        money(k.cost()),
                        money(k.upkeep() * (f * 100.0) as i64 / 100)
                    ),
                    10.0,
                );
                if k.covers().is_some() {
                    lab(
                        ui,
                        format!(
                            "Covers about {} m by road at this funding",
                            k.reach_at(f).round() as i64
                        ),
                        10.0,
                    );
                }
                if !k.effect_text().is_empty() {
                    wrapped(ui, k.effect_text(), 10.0, TEXT);
                }
                lab(ui, "Takes 40 to 70 days to build", 10.0);
                if btn(ui, "Stop placing", false, 11.0).clicked() {
                    self.ui.tool = None;
                }
            }
        });
        if closed {
            self.ui.build_open = false;
            self.ui.tool = None;
        } else if let Some(kind) = picked {
            self.ui.zone = None;
            self.open_build_for(kind);
        } else if let Some(kind) = zone {
            self.close_sheets();
            self.ui.road = None;
            self.ui.road_pts.clear();
            self.sel = None;
            self.ui.zone = Some((kind, None));
        }
    }

    pub(super) fn zone_corner(&mut self, p: Pt) {
        let Some((zone, first)) = self.ui.zone else {
            return;
        };
        if let Some(first) = first {
            match self.sim.paint_zone(zone, first, p) {
                Ok(n) => {
                    self.ui.zone = Some((zone, None));
                    self.toast(
                        format!("{n} {} parcels zoned", zone.label().to_lowercase()),
                        false,
                    );
                }
                Err(err) => self.toast(err.message(), true),
            }
        } else {
            self.ui.zone = Some((zone, Some(p)));
        }
    }

    pub(super) fn zone_prompt(&mut self, ctx: &Context) {
        let Some((zone, first)) = self.ui.zone else {
            return;
        };
        let y = if self.compact {
            dock_top(ctx) - ctx.content_rect().bottom() - 7.0
        } else {
            -34.0
        };
        egui::Area::new(egui::Id::new("zone_prompt"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, y))
            .show(ctx, |ui| {
                theme::panel()
                    .inner_margin(egui::Margin::symmetric(8, 4))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let hint = if first.is_some() {
                                "Choose opposite corner"
                            } else {
                                "Choose first corner"
                            };
                            lab(ui, format!("{} · {hint}", zone.label()), 12.0);
                            if first.is_some() && btn(ui, "Restart", false, 11.0).clicked() {
                                self.ui.zone = Some((zone, None));
                            }
                            if btn(ui, "Done", false, 11.0).clicked() {
                                self.ui.zone = None;
                            }
                        });
                    });
            });
    }

    fn capacity_rows(&self, ui: &mut Ui) {
        let Some(capacities) = self.ui.growth.capacities else {
            return;
        };
        for st in capacities {
            let stopped = self.ui.growth.stalls.contains(&st.capacity);
            lab_c(
                ui,
                format!(
                    "{}: {} of {} committed{}",
                    st.capacity.label(),
                    charts::group(st.demand),
                    charts::group(st.supply),
                    if stopped {
                        "  •  growth paused"
                    } else if st.is_full() {
                        "  •  full"
                    } else {
                        ""
                    }
                ),
                12.0,
                if stopped { ORANGE } else { TEXT },
            );
        }
    }

    /// The compact placement controls leave the map free while the player picks a site.
    pub(super) fn civic_placement_ui(&mut self, ctx: &Context) {
        let Some(kind) = self.ui.tool else { return };
        if self.any_sheet() || self.menu_open || self.election.is_some() {
            return;
        }
        let screen = ctx.content_rect();
        let up = screen.bottom() - dock_top(ctx) + 8.0;
        let site = self.ui.civic_site;
        let verdict = site.map(|site| self.sim.check_civic(kind, site));
        let mut confirm = false;
        let mut cancel = false;
        egui::Area::new(egui::Id::new("civic-placement"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -up))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                theme::panel().show(ui, |ui| {
                    ui.set_max_width(screen.width() - 36.0);
                    lab(
                        ui,
                        format!("{}  {}", kind.label(), money(kind.cost())),
                        14.0,
                    );
                    let (message, colour) = match verdict.as_ref() {
                        Some(Ok(())) => ("Site ready".into(), GOOD),
                        Some(Err(error)) => (error.text(), BAD),
                        None => ("Tap near an open street".into(), TEXT),
                    };
                    wrapped(ui, message, 12.0, colour);
                    ui.horizontal_wrapped(|ui| {
                        confirm = ui
                            .add_enabled(
                                matches!(verdict, Some(Ok(()))),
                                egui::Button::new("Start construction")
                                    .min_size(vec2(0.0, theme::TOUCH)),
                            )
                            .clicked();
                        cancel = ui
                            .add(egui::Button::new("Cancel").min_size(vec2(0.0, theme::TOUCH)))
                            .clicked();
                    });
                });
            });
        if confirm && let Some(site) = site {
            self.place_tool(site);
            self.ui.civic_site = None;
            self.ui.tool = None;
        } else if cancel {
            self.ui.civic_site = None;
            self.ui.tool = None;
        }
    }

    /// Whether the road tool is live: chosen, and the Build window open.
    pub(super) fn road_active(&self) -> Option<RoadMode> {
        self.ui.road.filter(|_| self.ui.build_open)
    }

    /// The road tool's text and buttons in the Build window.
    fn road_panel(&mut self, ui: &mut Ui) {
        let Some(mode) = self.ui.road else { return };
        let dim = Color32::from_rgb(170, 176, 196);
        if mode == RoadMode::Upgrade {
            wrapped(
                ui,
                "Click a street to widen it one class: local to collector, collector to arterial. It keeps carrying traffic during the work. Empty lots beside it are given up; a standing building in the way stops it.",
                11.0,
                dim,
            );
            if btn(ui, "Stop", false, 11.0).clicked() {
                self.ui.road = None;
            }
            return;
        }
        wrapped(
            ui,
            "Click points on the map, starting on an open road. End on a road or on open ground. Enter or a double click builds it. Esc or right click cancels. Bridges cost four times as much per metre. Empty lots under the road are given up. A standing building blocks the road: nothing is bought out.",
            11.0,
            dim,
        );
        let n = self.ui.road_pts.len();
        lab(ui, format!("Points: {n}"), 11.0);
        if n >= 2 {
            match self.sim.check_road(mode.class(), &self.ui.road_pts) {
                Ok(p) => {
                    lab_c(
                        ui,
                        format!(
                            "{:.0} m, costs {}, about {} a month",
                            p.length,
                            money(p.cost),
                            money(p.upkeep)
                        ),
                        11.0,
                        GOLD,
                    );
                    if p.bridge_m > 0.0 {
                        lab(ui, format!("Bridge {:.0} m", p.bridge_m), 10.0);
                    }
                    if p.lots_taken > 0 {
                        lab(ui, format!("{} empty lots given up", p.lots_taken), 10.0);
                    }
                }
                Err(e) => lab_c(ui, e.text(), 11.0, BAD),
            }
        }
        lab(ui, "Takes 3 to 12 weeks to build", 10.0);
        ui.horizontal(|ui| {
            if btn(ui, "Build", n >= 2, 12.0).clicked() {
                self.road_confirm();
            }
            if btn(ui, "Undo point", false, 11.0).clicked() {
                self.ui.road_pts.pop();
            }
            if btn(ui, "Cancel", false, 11.0).clicked() {
                self.ui.road_pts.clear();
            }
        });
    }

    /// A click on the map while the road tool is chosen.
    pub(super) fn road_click(&mut self, p: Pt) {
        let Some(mode) = self.road_active() else {
            return;
        };
        if mode == RoadMode::Upgrade {
            match self.sim.upgrade_street(p) {
                Ok(_) => self.toast("Street widening started.", false),
                Err(e) => self.toast(e.text().to_string(), true),
            }
            return;
        }
        if let Some(&last) = self.ui.road_pts.last() {
            if last.dist(p) < 8.0 {
                // a double click ends the road
                self.road_confirm();
                return;
            }
        } else if let Err(e @ RoadError::StartNotOnRoad { .. }) =
            self.sim.check_road(mode.class(), &[p, pt(p.x + 40.0, p.y)])
        {
            self.toast(e.text().to_string(), true);
            return;
        }
        if self.ui.road_pts.len() < 24 {
            self.ui.road_pts.push(p);
        }
    }

    /// Build the road drawn so far.
    pub(super) fn road_confirm(&mut self) {
        let Some(mode) = self.road_active() else {
            return;
        };
        if self.ui.road_pts.len() < 2 {
            self.toast("Click at least two points.", true);
            return;
        }
        let pts = std::mem::take(&mut self.ui.road_pts);
        match self.sim.check_road(mode.class(), &pts) {
            Ok(plan) => match self.sim.build_road(mode.class(), &pts) {
                Ok(_) => self.toast(
                    format!("Work started on a road. It costs {}.", money(plan.cost)),
                    false,
                ),
                Err(e) => {
                    self.ui.road_pts = pts;
                    self.toast(e.text().to_string(), true);
                }
            },
            Err(e) => {
                self.ui.road_pts = pts;
                self.toast(e.text().to_string(), true);
            }
        }
    }

    /// Right click or Esc: drop the road being drawn, or leave the tool.
    pub(super) fn road_cancel(&mut self) {
        if self.road_active().is_none() {
            return;
        }
        if self.ui.road_pts.is_empty() {
            self.ui.road = None;
        } else {
            self.ui.road_pts.clear();
        }
    }

    /// The road being drawn: its path, length, cost, bridges, and why a spot is refused.
    fn road_ghost_ui(
        &mut self,
        ctx: &Context,
        painter: &egui::Painter,
        view: &View,
        w: u32,
        h: u32,
    ) {
        let Some(mode) = self.road_active() else {
            return;
        };
        let ppp = ctx.pixels_per_point();
        let hover = ctx.pointer_hover_pos();
        let cursor = if ctx.egui_wants_pointer_input() {
            None
        } else {
            hover.and_then(|pos| {
                let ndc = (
                    pos.x * ppp / w as f32 * 2.0 - 1.0,
                    1.0 - pos.y * ppp / h as f32 * 2.0,
                );
                self.live
                    .scene
                    .pick(view, ndc)
                    .map(|g| pt(g.x as f64, g.y as f64))
            })
        };
        let screen = |me: &Self, p: Pt| {
            project(
                view,
                Vec3::new(
                    p.x as f32,
                    p.y as f32,
                    me.live.ground_at(p.x as f32, p.y as f32) + 1.0,
                ),
            )
            .map(|s| super::to_screen(s, w, h, ppp))
        };
        if mode == RoadMode::Upgrade {
            let Some(c) = cursor else { return };
            let (text, col, path) = match self.sim.check_upgrade(c) {
                Ok(pl) => (
                    format!(
                        "To {}  {:.0} m  {}",
                        pl.to.name(),
                        pl.length,
                        money(pl.cost)
                    ),
                    GOOD,
                    pl.points,
                ),
                Err(e) => (e.text(), BAD, Vec::new()),
            };
            self.draw_path(painter, view, &path, col, 5.0, w, h, ppp);
            if let Some(a) = screen(self, c) {
                self.label_at(painter, a + vec2(18.0, 18.0), &text.to_string(), col);
            }
            return;
        }
        let mut path = self.ui.road_pts.clone();
        if let Some(c) = cursor {
            path.push(c);
        }
        if path.is_empty() {
            return;
        }
        let verdict = if path.len() >= 2 {
            Some(self.sim.check_road(mode.class(), &path))
        } else {
            None
        };
        let (col, shown) = match &verdict {
            Some(Ok(p)) => (GOOD, p.points.clone()),
            _ => (BAD, path.clone()),
        };
        let col = if verdict.is_none() { GOLD } else { col };
        self.draw_path(painter, view, &shown, col, 5.0, w, h, ppp);
        if let Some(Ok(p)) = &verdict {
            for (a, b) in &p.bridges {
                self.draw_path(painter, view, &[*a, *b], GOLD, 8.0, w, h, ppp);
            }
        }
        for &q in &self.ui.road_pts {
            if let Some(s) = screen(self, q) {
                painter.circle_filled(s, 5.0, INK);
            }
        }
        let text = match &verdict {
            Some(Ok(p)) => {
                let mut t = format!("{:.0} M  {}", p.length, money(p.cost));
                if p.bridge_m > 0.0 {
                    t += &format!("  Bridge {:.0} m", p.bridge_m);
                }
                if p.lots_taken > 0 {
                    t += &format!("  {} lots", p.lots_taken);
                }
                t
            }
            Some(Err(e)) => {
                if let Some(at) = e.at()
                    && let Some(s) = screen(self, at)
                {
                    painter.circle_stroke(s, 12.0, Stroke::new(3.0, BAD));
                    painter.line_segment(
                        [s - vec2(8.0, 8.0), s + vec2(8.0, 8.0)],
                        Stroke::new(3.0, BAD),
                    );
                    painter.line_segment(
                        [s + vec2(-8.0, 8.0), s + vec2(8.0, -8.0)],
                        Stroke::new(3.0, BAD),
                    );
                }
                e.text().to_string()
            }
            None => "Click the next point".into(),
        };
        let at = hover.unwrap_or(Pos2::ZERO) + vec2(18.0, 18.0);
        self.label_at(painter, at, &text, col);
    }

    fn label_at(&self, painter: &egui::Painter, at: Pos2, text: &str, col: Color32) {
        let galley = painter.layout_no_wrap(text.to_string(), ui_font(11.0), INK);
        let r = egui::Rect::from_min_size(at, galley.size()).expand2(vec2(6.0, 3.0));
        painter.rect_filled(r, 3.0, PANEL_DARK);
        painter.rect_stroke(r, 3.0, Stroke::new(1.5, col), egui::StrokeKind::Inside);
        painter.galley(at, galley, INK);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_path(
        &self,
        painter: &egui::Painter,
        view: &View,
        path: &[Pt],
        col: Color32,
        width: f32,
        w: u32,
        h: u32,
        ppp: f32,
    ) {
        let mut line: Vec<Pos2> = Vec::new();
        for seg in path.windows(2) {
            let n = (seg[0].dist(seg[1]) / 8.0).ceil().max(1.0) as usize;
            for k in 0..=n {
                let p = seg[0].lerp(seg[1], k as f64 / n as f64);
                if let Some(s) = project(
                    view,
                    Vec3::new(
                        p.x as f32,
                        p.y as f32,
                        self.live.ground_at(p.x as f32, p.y as f32) + 1.0,
                    ),
                ) {
                    line.push(super::to_screen(s, w, h, ppp));
                }
            }
        }
        if line.len() >= 2 {
            painter.add(egui::Shape::line(line, Stroke::new(width, col)));
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
        if let Some((_, anchor)) = self.ui.zone {
            if let Some(a) = anchor {
                let ppp = ctx.pixels_per_point();
                let marker = ring_points(&self.live, view, a, 4.0, w, h, ppp);
                if marker.len() > 2 {
                    painter.add(egui::Shape::closed_line(marker, Stroke::new(2.0, ACCENT)));
                }
                if !ctx.egui_wants_pointer_input()
                    && let Some(pos) = ctx.pointer_hover_pos()
                    && let Some(b) = self.live.scene.pick(
                        view,
                        (
                            pos.x * ppp / w as f32 * 2.0 - 1.0,
                            1.0 - pos.y * ppp / h as f32 * 2.0,
                        ),
                    )
                {
                    let b = pt(f64::from(b.x), f64::from(b.y));
                    let outline = [a, pt(a.x, b.y), b, pt(b.x, a.y), a];
                    self.draw_path(painter, view, &outline, ACCENT, 2.0, w, h, ppp);
                }
            }
            return;
        }
        if self.road_active().is_some() {
            self.road_ghost_ui(ctx, painter, view, w, h);
            return;
        }
        let Some(kind) = self.ui.tool else { return };
        let ppp = ctx.pixels_per_point();
        let hover = ctx.pointer_hover_pos();
        let pinned = self.ui.fixed_ghost.or(self.ui.civic_site);
        let p = if let Some(p) = pinned {
            p
        } else {
            if self.compact {
                return;
            }
            let Some(pos) = hover else { return };
            if ctx.egui_wants_pointer_input() {
                return;
            }
            let ndc = (
                pos.x * ppp / w as f32 * 2.0 - 1.0,
                1.0 - pos.y * ppp / h as f32 * 2.0,
            );
            let Some(g) = self.live.scene.pick(view, ndc) else {
                return;
            };
            self.sim.snap_civic_site(kind, pt(g.x as f64, g.y as f64))
        };
        let pos = match (
            pinned,
            project(
                view,
                Vec3::new(
                    p.x as f32,
                    p.y as f32,
                    self.live.ground_at(p.x as f32, p.y as f32),
                ),
            ),
        ) {
            (Some(_), Some(s)) => super::to_screen(s, w, h, ppp),
            _ => hover.unwrap_or(Pos2::ZERO),
        };
        let verdict = self.sim.check_civic(kind, p);
        let ok = verdict.is_ok();
        let col = if ok { GOOD } else { BAD };
        let foot = ring_points(&self.live, view, p, kind.radius() as f32, w, h, ppp);
        if foot.len() > 3 {
            painter.add(egui::Shape::convex_polygon(
                foot,
                Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 110),
                Stroke::new(3.0, col),
            ));
        }
        let f = dept_fraction(&self.sim, kind.service());
        let reach = kind.reach_at(f) as f32;
        if reach > 0.0 {
            let ring = ring_points(&self.live, view, p, reach, w, h, ppp);
            if ring.len() > 3 {
                painter.add(egui::Shape::closed_line(ring, Stroke::new(2.0, GOLD)));
            }
        }
        if self.compact && self.ui.fixed_ghost.is_none() {
            return;
        }
        let text = match verdict {
            Ok(()) => format!("{}  {}", kind.label(), money(kind.cost())).to_string(),
            Err(e) => e.text().to_string(),
        };
        let galley = painter.layout(text, ui_font(12.0), INK, 220.0);
        let bounds = ctx.content_rect();
        let pad = vec2(8.0, 6.0);
        let at = (pos + vec2(18.0, 18.0)).clamp(
            bounds.min + pad,
            (bounds.max - galley.size() - pad).max(bounds.min + pad),
        );
        let r = egui::Rect::from_min_size(at, galley.size()).expand2(vec2(6.0, 3.0));
        painter.rect_filled(r, 3.0, PANEL_DARK);
        painter.rect_stroke(r, 3.0, Stroke::new(1.5, col), egui::StrokeKind::Inside);
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
                    .to_string(),
                    false,
                );
            }
            Err(e) => self.toast(e.text().to_string(), true),
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
        let svc = l.civic.map(|c| c.covers()).unwrap_or(match l.kind {
            us_gen::LotKind::School => Some(Service::Schools),
            us_gen::LotKind::Fire => Some(Service::Fire),
            us_gen::LotKind::Water => Some(Service::Water),
            us_gen::LotKind::Park | us_gen::LotKind::Sports => Some(Service::Parks),
            _ => None,
        });
        let Some(s) = svc else { return };
        let reach = (s.reach_m()
            * l.civic.map_or(1.0, |kind| {
                kind.reach_mult() * us_sim::civic_reach_multiplier(l.level)
            })
            * (0.4 + 0.6 * dept_fraction(&self.sim, s))) as f32;
        let ring = ring_points(&self.live, view, l.pos, reach, w, h, ppp);
        if ring.len() > 3 {
            painter.add(egui::Shape::closed_line(ring, Stroke::new(2.0, GOLD)));
        }
    }

    // ----- why panel extras -----

    pub(super) fn civic_upgrade_ui(&mut self, ui: &mut Ui, lot: usize) {
        let Some(site) = self.sim.lots().get(lot).filter(|site| site.civic.is_some()) else {
            return;
        };
        lab(ui, format!("Service tier {} of 3", site.level), 14.0);
        if site.level > 1 {
            let kind = site.civic.expect("civic building");
            let description = match kind {
                CivicKind::Water
                | CivicKind::Power
                | CivicKind::School
                | CivicKind::HighSchool
                | CivicKind::Clinic
                | CivicKind::Hospital => format!(
                    "Capacity {:.1}×",
                    us_sim::civic_capacity_pct(site.level) as f64 / 100.0
                ),
                CivicKind::University => "Expanded town-wide benefits".to_string(),
                _ => format!("Reach {:.2}×", us_sim::civic_reach_multiplier(site.level)),
            };
            lab_c(ui, description, 12.0, theme::MUTED);
        }
        if let Some((tier, cost)) = self.sim.civic_upgrade(lot) {
            let label = if self.sim.rules().unlimited_money {
                format!("Upgrade to tier {tier}")
            } else {
                format!("Upgrade to tier {tier} · {}", super::money(cost))
            };
            if theme::button(ui, &label).clicked() {
                match self.sim.upgrade_civic(lot) {
                    Ok(_) => {
                        self.ui.growth.capacities = None;
                        self.toast("Service expanded", false);
                    }
                    Err(error) => self.toast(error.text(), true),
                }
            }
        }
    }

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
        lab(ui, format!("Land value {:.0}% ({word})", lv * 100.0), 12.0);
        if !l.exempt && class_of(l.kind).is_some() {
            lab(ui, format!("Building level {} of 5", l.level), 12.0);
        }
        for line in self.sim.ordinance_lines(lot) {
            wrapped(ui, line, 10.0, GOOD);
        }
        if let Some(k) = l.civic {
            let cover = if k.covers().is_some() {
                format!(
                    "Covers about {} m by road. ",
                    k.reach_at(1.0).round() as i64
                )
            } else {
                String::new()
            };
            lab(
                ui,
                format!(
                    "{} {cover}upkeep {} a month at full funding.",
                    k.label(),
                    money(k.upkeep())
                ),
                10.0,
            );
            if !k.effect_text().is_empty() {
                wrapped(ui, k.effect_text(), 10.0, TEXT);
            }
        }
        for note in self.sim.civic_notes(lot) {
            wrapped(ui, &note, 10.0, TEXT);
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
                lab(ui, "Covered by", 10.0);
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
            MapView::District => (
                sim.district_lot_colours()
                    .iter()
                    .map(|&c| c as f32)
                    .collect(),
                Vec::new(),
            ),
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
                        if mv == MapView::District {
                            self.district_legend(ui);
                        }
                        ui.horizontal(|ui| {
                            for (col, name) in mv.steps() {
                                let (r, _) =
                                    ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                                ui.painter().rect_filled(r, 3.0, col);
                                ui.painter().rect_stroke(
                                    r,
                                    3.0,
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
                            RichText::new(s.name.to_string())
                                .font(ui_font(12.0))
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
                            EventKind::Election => ("Election", GOLD),
                            EventKind::GameOver => ("Game over", CORAL),
                            EventKind::Limit => ("Limit", ORANGE),
                            EventKind::Weather => ("Weather", BLUE),
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
            self.cam.look_at(Vec2::new(p.x as f32, p.y as f32), 260.0);
        }
        if closed {
            self.ui.news_open = false;
        }
    }
}
