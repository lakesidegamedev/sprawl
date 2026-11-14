//! App state: screens, input, the running town and every piece of UI.

#![allow(deprecated)]

use crate::cam::CamCtl;
use crate::theme::{self, ACCENT, BAD, INK, TEXT, heading, pixel};
use crate::world::LiveScene;
use egui::{Align2, Color32, Pos2, RichText, Stroke, vec2};
use glam::Vec3;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use us_gen::{LotKind, Preset, Pt, Town, generate, pt};
use us_render::{Calendar, Camera, Scene, Style, pick_ground, project};
use us_sim::{Phase, Sim, hall_sites};
use winit::keyboard::KeyCode;

mod charts;
mod panels;
mod screens;

use screens::{Mode, Nav};
pub mod touch;

const DAY_SECONDS: f64 = 1.3;
const SPEEDS: [f64; 4] = [0.0, 1.0, 3.0, 8.0];
/// Real seconds for one full visual day and night at 1x.
const LIGHT_DAY_SECONDS: f32 = 240.0;
const SPEED_LABELS: [&str; 4] = ["II", "1X", "3X", "8X"];

pub struct Settings {
    pub fullscreen: bool,
    pub ui_scale: f32,
    pub high_shadows: bool,
    /// Day and night cycle (true) or always day (false).
    pub day_night: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Menu,
    NewTown,
    Settings,
    Load,
    Generating,
    Game,
}

#[derive(Clone, Copy)]
enum Sel {
    Lot(usize),
    Hall,
}

struct Press {
    dragged: bool,
    on_ui: bool,
    travel: f32,
}

#[derive(Default, Clone)]
struct Stats {
    pop: i64,
    built: usize,
    empty: usize,
    building: usize,
    income: i64,
    expense: i64,
}

/// First-run guidance and event feedback for a running town.
#[derive(Default)]
struct Coach {
    /// (text, seconds left, is a warning)
    toasts: Vec<(String, f32, bool)>,
    goal_idx: usize,
    tip_hidden: bool,
    /// Shop, school, church, fire station: first one built.
    seen_kind: [bool; 4],
    last_left: Option<usize>,
    reveal_acc: usize,
    reveal_cool: f32,
    warn_cool: f32,
    neg_flagged: bool,
    scan_t: f32,
}

/// Population goals, in order.
const POP_GOALS: [i64; 6] = [100, 250, 500, 1000, 2000, 4000];

struct Game {
    town: Arc<Town>,
    preset: Preset,
    seed: u64,
    sim: Sim,
    live: LiveScene,
    placed: bool,
    sites: Vec<Pt>,
    cam: CamCtl,
    speed: usize,
    day_acc: f64,
    /// Hour of the visual day, 0 to 24. Runs on its own calm clock, not the game date.
    hour: f32,
    /// Screenshot hook: hold the hour.
    hour_frozen: bool,
    sel: Option<Sel>,
    banner: Option<(String, f32)>,
    menu_open: bool,
    stats: Stats,
    coach: Coach,
    ui: panels::UiState,
    rules: us_sim::Rules,
    /// What saves of this town are called.
    name: String,
    /// The file the last Save wrote, so Save again overwrites it.
    save_path: Option<std::path::PathBuf>,
    /// Game year of the last autosave.
    autosave_year: i32,
    voters_open: bool,
    map_open: bool,
    /// Text of the Save as box while it is open.
    save_as: Option<String>,
    menu_note: String,
    /// Election day already shown, and the result being shown now.
    election_seen: i32,
    election: Option<us_sim::ElectionResult>,
    /// The phone layout was on at the last frame.
    compact: bool,
    compact_init: bool,
}

struct Demo {
    live: LiveScene,
    cam: CamCtl,
}

struct Smoke {
    start: Instant,
    secs: f32,
    stage: u8,
    frames: u32,
    worst: f32,
    /// Test hook: set this tax rate (tenths) after this many seconds.
    tax_at: Option<(u32, f32)>,
    sync_max_ms: f32,
    sync_total_ms: f32,
}

pub struct State {
    screen: Screen,
    pub settings: Settings,
    preset: Preset,
    seed_text: String,
    game: Option<Game>,
    demo: Option<Demo>,
    gen_wait: u8,
    pub quit: bool,
    /// Settings were opened from the pause menu, so Back returns to the game.
    settings_from_game: bool,
    keys: HashSet<KeyCode>,
    cursor: (f32, f32),
    left: Option<Press>,
    right: Option<Press>,
    click: Option<(f32, f32)>,
    touches: touch::Touches,
    fps: f32,
    fps_acc: (f32, u32),
    style: Style,
    smoke: Option<Smoke>,
    error: Option<String>,
    /// Lakeside launcher handoff, optional sign-in and update check. The game never needs it to play.
    lakeside: us_lakeside::Lakeside,
    /// Last account or update message, written by worker threads.
    account_note: Arc<Mutex<String>>,
    account_busy: Arc<AtomicBool>,
    mode: Mode,
    saves: Vec<crate::saves::SaveInfo>,
    continue_info: Option<crate::saves::SaveInfo>,
    /// The load screen was opened from a running game, so Back returns to it.
    load_from_game: bool,
    load_note: String,
    delete_ask: Option<std::path::PathBuf>,
}

pub(super) fn random_seed() -> u64 {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    (n ^ (n >> 17)).wrapping_mul(0x9e37_79b9_7f4a_7c15) % 1_000_000
}

fn parse_seed(s: &str) -> u64 {
    let t = s.trim();
    t.parse::<u64>().unwrap_or_else(|_| {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in t.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h % 1_000_000
    })
}

pub(super) fn money(v: i64) -> String {
    let neg = v < 0;
    let s = v.abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    let out: String = out.chars().rev().collect();
    format!("{}${}", if neg { "-" } else { "" }, out)
}

fn to_screen(ndc: (f32, f32), w: u32, h: u32, ppp: f32) -> Pos2 {
    egui::pos2(
        (ndc.0 * 0.5 + 0.5) * w as f32 / ppp,
        (0.5 - ndc.1 * 0.5) * h as f32 / ppp,
    )
}

pub(super) fn label(ui: &mut egui::Ui, text: &str, size: f32) {
    ui.label(RichText::new(text).font(pixel(size)).color(TEXT));
}

impl State {
    pub fn new() -> State {
        let smoke = std::env::var("URBAN_SPRAWL_SMOKE")
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .map(|secs| Smoke {
                start: Instant::now(),
                secs,
                stage: 0,
                frames: 0,
                worst: 0.0,
                sync_max_ms: 0.0,
                sync_total_ms: 0.0,
                tax_at: std::env::var("URBAN_SPRAWL_SMOKE_TAX").ok().and_then(|v| {
                    let (a, b) = v.split_once(':')?;
                    Some((a.parse().ok()?, b.parse().ok()?))
                }),
            });
        let mut st = State {
            screen: Screen::Menu,
            settings: Settings {
                fullscreen: false,
                ui_scale: 1.0,
                high_shadows: true,
                day_night: true,
            },
            preset: Preset::Grid,
            seed_text: random_seed().to_string(),
            game: None,
            demo: None,
            gen_wait: 0,
            quit: false,
            settings_from_game: false,
            keys: HashSet::new(),
            cursor: (0.0, 0.0),
            left: None,
            right: None,
            click: None,
            touches: touch::Touches::default(),
            fps: 0.0,
            fps_acc: (0.0, 0),
            style: Style::default(),
            smoke,
            error: None,
            lakeside: us_lakeside::Lakeside::start(),
            account_note: Arc::new(Mutex::new(String::new())),
            account_busy: Arc::new(AtomicBool::new(false)),
            mode: Mode::Standard,
            saves: Vec::new(),
            continue_info: None,
            load_from_game: false,
            load_note: String::new(),
            delete_ask: None,
        };
        st.refresh_saves();
        if let Some(v) = std::env::var("URBAN_SPRAWL_SMOKE_SCALE")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
        {
            st.settings.ui_scale = v.clamp(0.8, 1.8);
        }
        st.make_demo();
        st
    }

    fn make_demo(&mut self) {
        let town = Arc::new(generate(Preset::Grid, 4));
        let hall = hall_sites(&town)[0];
        let mut sim = Sim::new(&town, hall, 4);
        for _ in 0..1100 {
            sim.step_day();
        }
        let mut live = LiveScene::new(town, &sim, true);
        live.sync(&mut sim, 1.0e9);
        let mut cam = CamCtl::overview(4096.0, 1.7);
        cam.look_at(Vec3::new(hall.x as f32, hall.y as f32, 0.0), 800.0);
        self.demo = Some(Demo { live, cam });
    }

    // ----- input -----

    pub fn cursor_moved(&mut self, x: f32, y: f32, height_px: u32) {
        let (dx, dy) = (x - self.cursor.0, y - self.cursor.1);
        self.cursor = (x, y);
        let travel = (dx * dx + dy * dy).sqrt();
        let Some(g) = self.game.as_mut().filter(|_| self.screen == Screen::Game) else {
            return;
        };
        if let Some(p) = self.left.as_mut().filter(|p| !p.on_ui) {
            p.travel += travel;
            if p.travel > 5.0 {
                p.dragged = true;
            }
            if p.dragged {
                g.cam.drag_pan(dx, dy, height_px as f32);
            }
        }
        if let Some(p) = self.right.as_mut().filter(|p| !p.on_ui) {
            p.travel += travel;
            p.dragged = true;
            g.cam.drag_orbit(dx, dy);
        }
    }

    pub fn mouse_button(&mut self, left: bool, down: bool, consumed: bool) {
        let slot = if left {
            &mut self.left
        } else {
            &mut self.right
        };
        if down {
            *slot = Some(Press {
                dragged: false,
                on_ui: consumed,
                travel: 0.0,
            });
        } else if let Some(p) = slot.take()
            && left
            && !p.dragged
            && !p.on_ui
            && self.screen == Screen::Game
        {
            self.click = Some(self.cursor);
        }
    }

    pub fn wheel(&mut self, lines: f32) {
        if self.screen == Screen::Game
            && let Some(g) = self.game.as_mut()
        {
            g.cam.zoom(lines);
        }
    }

    pub fn key(&mut self, code: KeyCode, down: bool, repeat: bool, consumed: bool) {
        if !down {
            self.keys.remove(&code);
            return;
        }
        if consumed {
            return;
        }
        self.keys.insert(code);
        if repeat || self.screen != Screen::Game {
            return;
        }
        let Some(g) = self.game.as_mut() else { return };
        if g.placed && g.ui_key(code) {
            return;
        }
        match code {
            KeyCode::Digit0 | KeyCode::Numpad0 => g.speed = 0,
            KeyCode::Digit1 | KeyCode::Numpad1 => g.speed = 1,
            KeyCode::Digit2 | KeyCode::Numpad2 => g.speed = 2,
            KeyCode::Digit3 | KeyCode::Numpad3 => g.speed = 3,
            KeyCode::Space => g.speed = if g.speed == 0 { 1 } else { 0 },
            KeyCode::Escape => g.menu_open = !g.menu_open,
            _ => {}
        }
    }

    pub fn release_all(&mut self) {
        self.touches.clear();
        self.keys.clear();
        self.left = None;
        self.right = None;
    }

    // ----- per frame -----

    fn held(&self, c: KeyCode) -> f32 {
        self.keys.contains(&c) as u8 as f32
    }

    pub fn update(&mut self, dt: f32, w: u32, h: u32) {
        self.style.shadow_size = if self.settings.high_shadows {
            4096
        } else {
            2048
        };
        self.autopilot();
        if self.screen == Screen::Load && self.saves.is_empty() && self.smoke.is_some() {
            self.refresh_saves();
        }
        if self.screen == Screen::Generating {
            self.gen_wait += 1;
            if self.gen_wait >= 2 {
                self.generate_now();
            }
        }
        if self.screen != Screen::Game {
            if let Some(d) = self.demo.as_mut() {
                d.cam.keys(dt, (0.0, 0.0), 0.07, false);
                d.cam.update(dt);
            }
            return;
        }
        let pan = (
            self.held(KeyCode::KeyD) - self.held(KeyCode::KeyA),
            self.held(KeyCode::KeyW) - self.held(KeyCode::KeyS),
        );
        let rot = self.held(KeyCode::KeyE) - self.held(KeyCode::KeyQ);
        let fast =
            self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight);
        let click = self.click.take();
        let cycle = self.settings.day_night;
        let Some(g) = self.game.as_mut() else { return };
        g.cam.keys(dt, pan, rot, fast);
        g.cam.update(dt);
        if let Some((b, t)) = g.banner.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                let _ = b;
                g.banner = None;
            }
        }
        if let Some(c) = click {
            let aspect = w as f32 / h as f32;
            let view = g.cam.camera().view(aspect);
            let ndc = (c.0 / w as f32 * 2.0 - 1.0, 1.0 - c.1 / h as f32 * 2.0);
            if !g.placed {
                // snap to the crossroads nearest the click on screen
                let ppp = 1.0;
                let mut best: Option<(f32, Pt)> = None;
                for s in &g.sites {
                    if let Some(p) = project(&view, Vec3::new(s.x as f32, s.y as f32, 0.0)) {
                        let sp = to_screen(p, w, h, ppp);
                        let d = ((sp.x - c.0).powi(2) + (sp.y - c.1).powi(2)).sqrt();
                        if best.is_none_or(|b| d < b.0) {
                            best = Some((d, *s));
                        }
                    }
                }
                if let Some((_, s)) = best {
                    g.place(s);
                }
            } else if let Some(p) = pick_ground(&view, ndc) {
                if g.ui.tool.is_some() {
                    g.place_tool(pt(p.x as f64, p.y as f64));
                } else {
                    g.select_at(pt(p.x as f64, p.y as f64));
                    if g.compact && g.sel.is_some() {
                        g.close_sheets();
                    }
                }
            }
        }
        if g.placed && g.speed > 0 && !g.menu_open && g.election.is_none() {
            g.day_acc += dt as f64 * SPEEDS[g.speed] / DAY_SECONDS;
            let mut steps = 0;
            while g.day_acc >= 1.0 && steps < 16 {
                g.day_acc -= 1.0;
                g.sim.step_day();
                steps += 1;
            }
            if steps == 16 {
                g.day_acc = 0.0;
            }
        }
        let t0 = Instant::now();
        g.live.sync(&mut g.sim, 5.0);
        let ms = t0.elapsed().as_secs_f32() * 1000.0;
        if let Some(s) = self.smoke.as_mut() {
            s.sync_max_ms = s.sync_max_ms.max(ms);
            s.sync_total_ms += ms;
        }
        if g.placed {
            if let Some(r) = g.sim.last_election()
                && r.day != g.election_seen
            {
                g.election_seen = r.day;
                g.election = Some(r.clone());
            }
            let year = g.sim.day / 365;
            if year > g.autosave_year && !g.sim.is_game_over() {
                g.autosave_year = year;
                let town = g.name.clone();
                match crate::saves::write_autosave(&crate::saves::save_dir(), &mut g.sim, &town) {
                    Ok(_) => g.toast("AUTOSAVED", false),
                    Err(e) => g.toast(format!("AUTOSAVE FAILED: {e}").to_uppercase(), true),
                }
                self.refresh_saves();
            }
        }
        let Some(g) = self.game.as_mut() else { return };
        g.refresh_stats();
        g.light_tick(dt, cycle);
        g.ui_tick(dt);
        if g.placed && !g.menu_open {
            g.coach_tick(dt);
        }
    }

    pub fn frame_done(&mut self, dt: f32, _drawn: bool) {
        self.fps_acc.0 += dt;
        self.fps_acc.1 += 1;
        if self.fps_acc.0 >= 0.5 {
            self.fps = self.fps_acc.1 as f32 / self.fps_acc.0;
            self.fps_acc = (0.0, 0);
        }
        if let Some(s) = self.smoke.as_mut() {
            s.frames += 1;
            if s.stage >= 3 {
                s.worst = s.worst.max(dt);
            }
        }
    }

    pub fn scene_camera(&self) -> (&Scene, Camera, Style) {
        if (self.screen == Screen::Game || self.settings_from_game || self.load_from_game)
            && let Some(g) = &self.game
        {
            return (&g.live.scene, g.cam.camera(), self.style.clone());
        }
        let d = self.demo.as_ref().expect("demo town");
        (&d.live.scene, d.cam.camera(), self.style.clone())
    }

    fn generate_now(&mut self) {
        let seed = parse_seed(&self.seed_text);
        let town = Arc::new(generate(self.preset, seed));
        let sites = hall_sites(&town);
        let mut sim = Sim::new_with(&town, sites[0], seed, 0);
        let mut live = LiveScene::new(town.clone(), &sim, false);
        live.sync(&mut sim, 1.0e9);
        let cam = CamCtl::overview(town.size as f32, 1.7);
        let name = format!("{} {}", screens::town_name(self.preset.label()), seed);
        self.game = Some(Game::new(
            town,
            self.preset,
            seed,
            sim,
            live,
            false,
            sites,
            cam,
            self.mode.rules(),
            name,
        ));
        self.screen = Screen::Game;
    }

    /// Continue a game read from a save.
    pub(super) fn start_loaded(&mut self, sim: Sim, path: Option<std::path::PathBuf>) {
        let t = sim.town();
        let (preset, seed) = (t.preset, t.seed);
        let town = Arc::new(generate(preset, seed));
        let mut sim = sim;
        let mut live = LiveScene::new(town.clone(), &sim, true);
        live.sync(&mut sim, 1.0e9);
        let mut cam = CamCtl::overview(town.size as f32, 1.7);
        cam.look_at(Vec3::new(sim.hall.x as f32, sim.hall.y as f32, 0.0), 650.0);
        let rules = sim.rules();
        let name = format!("{} {}", screens::town_name(preset.label()), seed);
        let mut g = Game::new(
            town,
            preset,
            seed,
            sim,
            live,
            true,
            Vec::new(),
            cam,
            rules,
            name,
        );
        g.save_path = path;
        // do not greet a town that already exists with first-run tips and old goals
        g.coach.tip_hidden = true;
        g.coach.seen_kind = [true; 4];
        g.coach.goal_idx = POP_GOALS
            .iter()
            .filter(|&&n| g.sim.population() >= n)
            .count();
        g.speed = 1;
        g.sel = None;
        self.game = Some(g);
        self.screen = Screen::Game;
        self.settings_from_game = false;
    }

    /// Save the running town. Returns the message to show.
    pub(super) fn save_game(&mut self, name: Option<String>) -> (String, bool) {
        let Some(g) = self.game.as_mut() else {
            return ("Nothing to save.".into(), true);
        };
        if !g.placed {
            return ("Place the town hall first.".into(), true);
        }
        let dir = crate::saves::save_dir();
        let name = name.unwrap_or_else(|| g.name.clone());
        match crate::saves::write_named(&dir, &mut g.sim, &name) {
            Ok(p) => {
                g.name = name.clone();
                g.save_path = Some(p);
                self.refresh_saves();
                (format!("Saved as {name}."), false)
            }
            Err(e) => (format!("The game could not be saved: {e}"), true),
        }
    }

    fn autopilot(&mut self) {
        let Some(s) = self.smoke.as_mut() else { return };
        let t = s.start.elapsed().as_secs_f32();
        match s.stage {
            0 if t > 1.5 && std::env::var("URBAN_SPRAWL_SMOKE_SCREEN").is_ok() => {
                match std::env::var("URBAN_SPRAWL_SMOKE_SCREEN").as_deref() {
                    Ok("newtown") => self.screen = Screen::NewTown,
                    Ok("load") => self.screen = Screen::Load,
                    Ok("settings") => self.screen = Screen::Settings,
                    _ => {}
                }
                s.stage = 3;
            }
            0 if t > 1.5 => {
                self.seed_text = "1".into();
                if std::env::var("URBAN_SPRAWL_SMOKE_PRESET").is_ok_and(|v| v == "winding") {
                    self.preset = Preset::Winding;
                }
                self.screen = Screen::Generating;
                self.gen_wait = 0;
                s.stage = 1;
            }
            1 if self.screen == Screen::Game
                && t > 3.0
                && std::env::var("URBAN_SPRAWL_SMOKE_NOPLACE").is_err() =>
            {
                if let Some(g) = self.game.as_mut() {
                    let site = g.sites[0];
                    g.place(site);
                    g.speed = 3;
                    if let Some(t) = std::env::var("URBAN_SPRAWL_SMOKE_PRETAX")
                        .ok()
                        .and_then(|v| v.parse::<u32>().ok())
                    {
                        g.sim.set_tax_x10(t);
                    }
                    if let Some(days) = std::env::var("URBAN_SPRAWL_SMOKE_DAYS")
                        .ok()
                        .and_then(|v| v.parse::<i32>().ok())
                    {
                        for _ in 0..days {
                            g.sim.step_day();
                        }
                    }
                    if let Ok(spec) = std::env::var("URBAN_SPRAWL_SMOKE_UI") {
                        g.ui_hook(&spec);
                    }
                }
                s.stage = 2;
            }
            2 if t > 5.0 => s.stage = 3,
            3 if s.tax_at.is_some_and(|(_, at)| t > at) => {
                if let (Some(g), Some((tax, _))) = (self.game.as_mut(), s.tax_at.take()) {
                    g.sim.set_tax_x10(tax);
                }
            }
            3 if t > s.secs => {
                if let Some(g) = self.game.as_ref() {
                    eprintln!(
                        "smoke: day {} built {} pop {} treasury {} chunks {} tris {} fps {:.1} worst-frame {:.0} ms sync max {:.1} ms avg {:.2} ms",
                        g.sim.day,
                        g.sim.built_count(),
                        g.sim.population(),
                        g.sim.treasury,
                        g.live.scene.chunks.len(),
                        g.live.triangles,
                        self.fps,
                        s.worst * 1000.0,
                        s.sync_max_ms,
                        s.sync_total_ms / s.frames.max(1) as f32
                    );
                }
                self.quit = true;
            }
            _ => {}
        }
    }

    // ----- UI -----

    pub fn ui(&mut self, ui: &mut egui::Ui, w: u32, h: u32) {
        let ctx = ui.ctx().clone();
        theme::set_compact(&ctx, theme::wants_compact(&ctx));
        match self.screen {
            Screen::Menu => self.menu_ui(&ctx),
            Screen::NewTown => self.new_town_ui(&ctx),
            Screen::Settings => self.settings_ui(&ctx),
            Screen::Load => self.load_ui(&ctx),
            Screen::Generating => {
                egui::Area::new(egui::Id::new("gen"))
                    .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                    .show(&ctx, |ui| {
                        theme::panel().show(ui, |ui| {
                            ui.label(heading("BUILDING THE COUNTRYSIDE...", 24.0));
                        });
                    });
            }
            Screen::Game => self.game_ui(ui, &ctx, w, h),
        }
        if let Some(e) = self.error.clone() {
            egui::Area::new(egui::Id::new("err"))
                .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -20.0))
                .show(&ctx, |ui| {
                    theme::panel().show(ui, |ui| label(ui, &e, 14.0));
                });
        }
    }

    fn title_block(&self, ui: &mut egui::Ui) {
        let short = ui.ctx().content_rect().height() < 520.0;
        let compact = theme::is_compact(ui.ctx());
        ui.vertical_centered(|ui| {
            ui.label(heading(
                "URBAN SPRAWL",
                if short {
                    28.0
                } else if compact {
                    36.0
                } else {
                    56.0
                },
            ));
            ui.label(
                RichText::new(crate::VERSION_LABEL.to_uppercase())
                    .font(pixel(if short { 12.0 } else { 16.0 }))
                    .color(ACCENT),
            );
            ui.add_space(if short { 2.0 } else { 8.0 });
        });
    }

    fn menu_ui(&mut self, ctx: &egui::Context) {
        let short = ctx.content_rect().height() < 520.0;
        theme::centered(ctx, "menu", if short { 12 } else { 28 }, |ui| {
            if short {
                ui.spacing_mut().item_spacing.y = 4.0;
            }
            self.title_block(ui);
            // sign-in is desktop only; the App Store keeps the iOS app current
            #[cfg(not(target_os = "ios"))]
            {
                let status = self.lakeside.status();
                if short {
                } else if let Some(a) = &status.account {
                    ui.vertical_centered(|ui| {
                        label(ui, &format!("SIGNED IN AS {}", a.name.to_uppercase()), 14.0);
                        if status.launcher {
                            label(
                                ui,
                                "Through the Lakeside launcher, which keeps this copy up to date.",
                                12.0,
                            );
                        }
                        ui.add_space(6.0);
                    });
                } else if status.handoff_pending {
                    ui.vertical_centered(|ui| {
                        label(ui, "Signing in through the Lakeside launcher...", 12.0);
                        ui.add_space(6.0);
                    });
                } else {
                    ui.vertical_centered(|ui| {
                        label(
                            ui,
                            "Playing signed out. Sign in under Settings to get updates.",
                            12.0,
                        );
                        ui.add_space(6.0);
                    });
                }
            }
            ui.vertical_centered(|ui| {
                if let Some(c) = self.continue_info.clone()
                    && theme::button(ui, "CONTINUE").clicked()
                {
                    self.load_save(&c.path);
                }
                if theme::button(ui, "NEW TOWN").clicked() {
                    self.screen = Screen::NewTown;
                }
                if theme::button(ui, "LOAD GAME").clicked() {
                    self.refresh_saves();
                    self.load_note.clear();
                    self.screen = Screen::Load;
                }
                if theme::button(ui, "SETTINGS").clicked() {
                    self.screen = Screen::Settings;
                }
                // iOS apps are closed from the home screen, never by the app
                #[cfg(not(target_os = "ios"))]
                if theme::button(ui, "QUIT").clicked() {
                    self.quit = true;
                }
            });
        });
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        theme::centered(ctx, "settings", 24, |ui| {
            ui.label(heading("SETTINGS", 32.0));
            ui.add_space(6.0);
            #[cfg(not(target_os = "ios"))]
            {
                label(ui, "WINDOW", 14.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.settings.fullscreen,
                        false,
                        RichText::new("WINDOWED").font(pixel(16.0)),
                    );
                    ui.selectable_value(
                        &mut self.settings.fullscreen,
                        true,
                        RichText::new("FULLSCREEN").font(pixel(16.0)),
                    );
                });
            }
            label(ui, "UI SCALE", 14.0);
            ui.add(egui::Slider::new(&mut self.settings.ui_scale, 0.8..=1.8).step_by(0.1));
            label(ui, "SHADOWS", 14.0);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.settings.high_shadows,
                    true,
                    RichText::new("CRISP").font(pixel(16.0)),
                );
                ui.selectable_value(
                    &mut self.settings.high_shadows,
                    false,
                    RichText::new("FAST").font(pixel(16.0)),
                );
            });
            label(ui, "DAY AND NIGHT", 14.0);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.settings.day_night,
                    true,
                    RichText::new("CYCLE").font(pixel(16.0)),
                );
                ui.selectable_value(
                    &mut self.settings.day_night,
                    false,
                    RichText::new("ALWAYS DAY").font(pixel(16.0)),
                );
            });
            #[cfg(not(target_os = "ios"))]
            {
                label(ui, "F11 TOGGLES FULLSCREEN IN GAME", 12.0);
                ui.add_space(8.0);
                self.account_ui(ui);
            }
            ui.add_space(10.0);
            if theme::button(ui, "BACK").clicked() {
                if self.settings_from_game && self.game.is_some() {
                    // back to the pause menu of the running town
                    self.settings_from_game = false;
                    self.screen = Screen::Game;
                    if let Some(g) = self.game.as_mut() {
                        g.menu_open = true;
                    }
                } else {
                    self.settings_from_game = false;
                    self.screen = Screen::Menu;
                }
            }
        });
    }

    fn note(&self, text: impl Into<String>) {
        if let Ok(mut n) = self.account_note.lock() {
            *n = text.into();
        }
    }

    /// Lakeside account and updates, in Settings. Optional: nothing here is needed to play.
    fn account_ui(&mut self, ui: &mut egui::Ui) {
        let status = self.lakeside.status();
        let busy = self.account_busy.load(Ordering::SeqCst);
        label(ui, "LAKESIDE ACCOUNT", 14.0);
        if let Some(a) = &status.account {
            label(ui, &format!("Signed in to Lakeside as {}", a.name), 12.0);
            if theme::button(ui, "SIGN OUT").clicked() {
                self.lakeside.sign_out();
                self.note("Signed out.");
            }
        } else if status.handoff_pending {
            label(ui, "Checking your Lakeside launcher sign-in...", 12.0);
        } else if busy {
            label(ui, "Finish signing in in your browser.", 12.0);
            if theme::button(ui, "CANCEL").clicked() {
                self.lakeside.cancel_sign_in();
            }
        } else if theme::button(ui, "SIGN IN").clicked() {
            let lk = self.lakeside.clone();
            let note = self.account_note.clone();
            let flag = self.account_busy.clone();
            flag.store(true, Ordering::SeqCst);
            std::thread::spawn(move || {
                let text = match lk.sign_in() {
                    us_lakeside::SignInResult::Done { name } => format!("Signed in as {name}."),
                    us_lakeside::SignInResult::Failed(m) => format!("Sign-in did not finish: {m}"),
                    us_lakeside::SignInResult::Cancelled => "Sign-in cancelled.".to_string(),
                };
                if let Ok(mut n) = note.lock() {
                    *n = text;
                }
                flag.store(false, Ordering::SeqCst);
            });
        }
        label(ui, "UPDATES", 14.0);
        if status.launcher {
            label(
                ui,
                "The Lakeside launcher keeps this copy up to date.",
                12.0,
            );
        } else if status.updater_enabled
            && !busy
            && theme::button(ui, "CHECK FOR UPDATES").clicked()
        {
            let lk = self.lakeside.clone();
            let note = self.account_note.clone();
            let flag = self.account_busy.clone();
            flag.store(true, Ordering::SeqCst);
            std::thread::spawn(move || {
                let text = match lk.check_update() {
                    us_lakeside::UpdateCheck::Disabled => {
                        "The Lakeside launcher keeps this copy up to date.".to_string()
                    }
                    us_lakeside::UpdateCheck::Current => "This is the newest build.".to_string(),
                    us_lakeside::UpdateCheck::Available(o) => format!(
                        "Version {} is out. Install it from the Lakeside launcher.",
                        o.version
                    ),
                    us_lakeside::UpdateCheck::SignInNeeded => {
                        "Sign in to Lakeside to check for updates.".to_string()
                    }
                    us_lakeside::UpdateCheck::NotEntitled => {
                        "This Lakeside account does not include Urban Sprawl.".to_string()
                    }
                    us_lakeside::UpdateCheck::Unavailable(m) => format!("Could not check: {m}"),
                };
                if let Ok(mut n) = note.lock() {
                    *n = text;
                }
                flag.store(false, Ordering::SeqCst);
            });
        }
        let note = self
            .account_note
            .lock()
            .map(|n| n.clone())
            .unwrap_or_default();
        if !note.is_empty() {
            label(ui, &note, 12.0);
        }
    }

    fn game_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, w: u32, h: u32) {
        let ppp = ctx.pixels_per_point();
        let fps = self.fps;
        let mut to_menu = false;
        let mut quit = false;
        let mut open_settings = false;
        let who = self.lakeside.status().account.map(|a| a.name);
        let compact = theme::is_compact(ctx);
        let g = self.game.as_mut().unwrap();
        g.compact = compact;
        if compact {
            if !g.compact_init {
                g.compact_init = true;
                g.close_sheets();
            }
            g.one_sheet_only();
        }
        let view = g.cam.camera().view(w as f32 / h as f32);
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            egui::Id::new("world-overlay"),
        ));

        // placement markers
        if !g.placed {
            let cursor = ctx
                .pointer_hover_pos()
                .unwrap_or(Pos2::new(-9999.0, -9999.0));
            let mut nearest: Option<(f32, usize)> = None;
            let pts: Vec<Option<Pos2>> = g
                .sites
                .iter()
                .map(|s| {
                    project(&view, Vec3::new(s.x as f32, s.y as f32, 0.0))
                        .map(|p| to_screen(p, w, h, ppp))
                })
                .collect();
            for (i, p) in pts.iter().enumerate() {
                if let Some(p) = p {
                    let d = p.distance(cursor);
                    if d < 900.0 && nearest.is_none_or(|n| d < n.0) {
                        nearest = Some((d, i));
                    }
                }
            }
            for (i, p) in pts.iter().enumerate() {
                let Some(p) = p else { continue };
                let hot = nearest.is_some_and(|n| n.1 == i);
                let r = if hot { 13.0 } else { 8.0 };
                painter.rect_filled(
                    egui::Rect::from_center_size(*p, vec2(r * 2.0 + 4.0, r * 2.0 + 4.0)),
                    egui::CornerRadius::ZERO,
                    theme::LINE,
                );
                painter.rect_filled(
                    egui::Rect::from_center_size(*p, vec2(r * 2.0, r * 2.0)),
                    egui::CornerRadius::ZERO,
                    if hot { ACCENT } else { INK },
                );
            }
        } else if let Some(sel) = g.sel {
            let (pos, r) = match sel {
                Sel::Lot(i) => {
                    let l = &g.sim.lots()[i];
                    (l.pos, l.radius as f32)
                }
                Sel::Hall => (g.sim.hall, g.town.hall_radius as f32 + 6.0),
            };
            let c = Vec3::new(pos.x as f32, pos.y as f32, 0.0);
            if let (Some(a), Some(b)) = (
                project(&view, c),
                project(&view, c + Vec3::new(r, 0.0, 0.0)),
            ) {
                let (a, b) = (to_screen(a, w, h, ppp), to_screen(b, w, h, ppp));
                let rad = (a.distance(b)).max(10.0) * 1.15;
                painter.circle_stroke(a, rad + 2.0, Stroke::new(5.0, theme::LINE));
                painter.circle_stroke(a, rad, Stroke::new(3.0, ACCENT));
            }
        }

        // top bar
        let bar_frame = if compact {
            theme::bar().inner_margin(egui::Margin::symmetric(8, 4))
        } else {
            theme::bar()
        };
        let hud = egui::Panel::top("hud").frame(bar_frame).show(ui, |ui| {
            if compact {
                let wide = ui.available_width() >= 640.0;
                let small = |s: String, col: Color32| RichText::new(s).font(pixel(12.0)).color(col);
                let row1 = |ui: &mut egui::Ui, g: &Game| {
                    if g.placed {
                        ui.label(small(g.sim.date().text().to_uppercase(), INK));
                        ui.label(small(format!("POP {}", g.stats.pop), INK));
                        ui.label(small(
                            format!("${}", short_money(g.sim.treasury)),
                            if g.sim.treasury < 0 { BAD } else { INK },
                        ));
                        let net = g.stats.income - g.stats.expense;
                        ui.label(small(
                            format!(
                                "{}${}/MO",
                                if net < 0 { "-" } else { "+" },
                                short_money(net.abs())
                            ),
                            if net < 0 { BAD } else { theme::GOOD },
                        ));
                    } else {
                        ui.label(small("PLACE THE TOWN HALL".into(), INK));
                    }
                };
                let row2 = |ui: &mut egui::Ui, g: &mut Game| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if g.placed {
                        g.approval_chip(ui, true);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let b =
                            egui::Button::new(RichText::new("MENU").font(pixel(12.0)).color(INK))
                                .min_size(vec2(48.0, theme::TOUCH));
                        if ui.add(b).clicked() {
                            g.menu_open = true;
                        }
                        if g.placed {
                            for i in (0..4).rev() {
                                let on = g.speed == i;
                                let b = egui::Button::new(
                                    RichText::new(SPEED_LABELS[i])
                                        .font(pixel(12.0))
                                        .color(if on { theme::LINE } else { INK }),
                                )
                                .fill(if on { ACCENT } else { theme::RAISED })
                                .min_size(vec2(theme::TOUCH, theme::TOUCH));
                                if ui.add(b).clicked() {
                                    g.speed = i;
                                }
                            }
                        }
                    });
                };
                if wide {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        row1(ui, g);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let b = egui::Button::new(
                                RichText::new("MENU").font(pixel(12.0)).color(INK),
                            )
                            .min_size(vec2(48.0, theme::TOUCH));
                            if ui.add(b).clicked() {
                                g.menu_open = true;
                            }
                            if g.placed {
                                for i in (0..4).rev() {
                                    let on = g.speed == i;
                                    let b = egui::Button::new(
                                        RichText::new(SPEED_LABELS[i])
                                            .font(pixel(12.0))
                                            .color(if on { theme::LINE } else { INK }),
                                    )
                                    .fill(if on { ACCENT } else { theme::RAISED })
                                    .min_size(vec2(theme::TOUCH, theme::TOUCH));
                                    if ui.add(b).clicked() {
                                        g.speed = i;
                                    }
                                }
                                ui.add_space(6.0);
                                g.approval_chip(ui, true);
                            }
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        row1(ui, g)
                    });
                    ui.horizontal(|ui| row2(ui, g));
                }
                return;
            }
            // phones in portrait: two rows instead of one
            let narrow = ui.ctx().content_rect().width() < 1100.0;
            let tight = ui.ctx().content_rect().width() < 1500.0;
            let left = |ui: &mut egui::Ui, g: &mut Game| {
                let light = INK;
                let t = |s: String| {
                    RichText::new(s)
                        .font(pixel(if narrow {
                            13.0
                        } else if tight {
                            14.0
                        } else {
                            16.0
                        }))
                        .color(light)
                };
                if !narrow && ui.available_width() > 1500.0 {
                    ui.label(
                        RichText::new("URBAN SPRAWL")
                            .font(pixel(if narrow {
                                13.0
                            } else if tight {
                                14.0
                            } else {
                                16.0
                            }))
                            .color(ACCENT),
                    );
                    ui.add_space(14.0);
                }
                if g.placed {
                    ui.label(t(g.sim.date().text().to_uppercase()));
                    ui.add_space(10.0);
                    ui.label(t(format!("POP {}", g.stats.pop)));
                    ui.add_space(10.0);
                    let tc = if g.sim.treasury < 0 {
                        Color32::from_rgb(255, 130, 120)
                    } else {
                        light
                    };
                    ui.label(
                        RichText::new(format!("TREASURY {}", money(g.sim.treasury)))
                            .font(pixel(if narrow {
                                13.0
                            } else if tight {
                                14.0
                            } else {
                                16.0
                            }))
                            .color(tc),
                    );
                    ui.add_space(10.0);
                    let net = g.stats.income - g.stats.expense;
                    let nc = if net < 0 {
                        Color32::from_rgb(255, 130, 120)
                    } else {
                        Color32::from_rgb(150, 230, 160)
                    };
                    ui.label(
                        RichText::new(format!(
                            "{}{}/MO",
                            if net < 0 { "" } else { "+" },
                            money(net)
                        ))
                        .font(pixel(if narrow {
                            13.0
                        } else if tight {
                            14.0
                        } else {
                            16.0
                        }))
                        .color(nc),
                    );
                    ui.add_space(10.0);
                    g.approval_chip(ui, false);
                    g.sparks_ui(ui);
                } else {
                    ui.label(t("PLACE THE TOWN HALL".to_string()));
                }
            };
            let controls = |ui: &mut egui::Ui, g: &mut Game| {
                if ui
                    .add(
                        egui::Button::new(RichText::new("MENU").font(pixel(14.0)).color(INK))
                            .min_size(if narrow {
                                vec2(70.0, 44.0)
                            } else {
                                vec2(0.0, 0.0)
                            }),
                    )
                    .clicked()
                {
                    g.menu_open = true;
                }
                if let Some(n) = &who {
                    ui.label(
                        RichText::new(format!("SIGNED IN: {}", n.to_uppercase()))
                            .font(pixel(10.0))
                            .color(Color32::from_rgb(170, 176, 196)),
                    );
                }
                if !tight {
                    ui.label(
                        RichText::new(format!("{fps:.0} FPS"))
                            .font(pixel(12.0))
                            .color(Color32::from_rgb(170, 176, 196)),
                    );
                }
                if g.placed {
                    let order: Vec<usize> = if narrow {
                        (0..4).collect()
                    } else {
                        (0..4).rev().collect()
                    };
                    for i in order {
                        let on = g.speed == i;
                        let b = egui::Button::new(
                            RichText::new(SPEED_LABELS[i])
                                .font(pixel(14.0))
                                .color(if on { theme::LINE } else { INK }),
                        )
                        .fill(if on { ACCENT } else { theme::RAISED })
                        .min_size(if narrow {
                            vec2(52.0, 44.0)
                        } else {
                            vec2(38.0, 26.0)
                        });
                        if ui.add(b).clicked() {
                            g.speed = i;
                        }
                    }
                }
            };
            if narrow {
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| left(ui, g));
                    ui.horizontal(|ui| controls(ui, g));
                });
            } else {
                ui.horizontal(|ui| {
                    left(ui, g);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        controls(ui, g)
                    });
                });
            }
        });
        let top_y = if compact {
            hud.response.rect.bottom() + 6.0
        } else {
            96.0
        };

        if g.placed && compact {
            egui::Panel::bottom("dock")
                .frame(theme::bar().inner_margin(egui::Margin::symmetric(4, 3)))
                .show(ui, |ui| g.dock_ui(ui));
        } else if g.placed {
            egui::Panel::top("tools")
                .frame(theme::bar())
                .show(ui, |ui| g.toolbar_ui(ui));
        }

        if g.placed {
            g.selection_reach(&painter, &view, w, h, ppp);
            g.ghost_ui(ctx, &painter, &view, w, h);
        }

        // banner
        let banner = if !g.placed {
            Some("CLICK A CROSSROADS TO PLACE THE TOWN HALL".to_string())
        } else {
            g.banner.as_ref().map(|b| b.0.clone())
        };
        let stacked_banner = if compact && g.placed {
            banner.clone()
        } else {
            None
        };
        if let Some(b) = banner.filter(|_| !(compact && g.placed)) {
            egui::Area::new(egui::Id::new("banner"))
                .anchor(Align2::CENTER_TOP, vec2(0.0, top_y))
                .show(ctx, |ui| {
                    theme::panel().show(ui, |ui| {
                        if compact {
                            ui.set_max_width(ctx.content_rect().width() - 40.0);
                            ui.add(
                                egui::Label::new(heading(&b, 14.0)).wrap(),
                            );
                            if !g.placed {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new("TAP AN OLD CROSSROADS (WHITE SQUARES).")
                                            .font(pixel(11.0))
                                            .color(TEXT),
                                    )
                                    .wrap(),
                                );
                            }
                            return;
                        }
                        ui.label(heading(&b, 18.0));
                        if !g.placed {
                            label(ui, "THE WHITE SQUARES ARE OLD CROSSROADS. THE ONE NEAREST YOUR CURSOR GLOWS ORANGE. YOUR MAIN STREET WILL RUN TO THE HIGHWAY.", 12.0);
                        }
                    });
                });
        }

        if g.placed {
            // goal and first-run tips
            let goal = POP_GOALS.get(g.coach.goal_idx).copied();
            let day = g.sim.day;
            let tip = if g.coach.tip_hidden {
                None
            } else if day < 60 {
                Some(
                    "PEOPLE MOVE IN AS BUILDINGS FINISH. CLICK ANY BUILDING TO SEE WHY IT WAS BUILT.",
                )
            } else if day < 150 {
                Some("TAXES PAY FOR THE TOWN. WATCH THE TREASURY AND THE MONTHLY NUMBER BESIDE IT.")
            } else if day < 300 {
                Some(
                    "LOW TAX GROWS FASTER BUT FILLS THE TREASURY SLOWER. HIGH TAX EMPTIES BUILDINGS. TRY THE SLIDER.",
                )
            } else {
                None
            };
            let pop = g.stats.pop;
            let mut hide_tip = false;
            if compact {
                // one short stack: the goal on one line, then up to two news toasts
                let wmax = (ctx.content_rect().width() - 24.0).min(340.0);
                egui::Area::new(egui::Id::new("goal"))
                    .anchor(Align2::LEFT_TOP, vec2(8.0, top_y))
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.set_max_width(wmax);
                        let goal_text = match goal {
                            Some(n) => format!("GOAL {n} PEOPLE ({pop})"),
                            None => "KEEP YOUR TOWN GROWING".to_string(),
                        };
                        if let Some(b) = &stacked_banner {
                            theme::panel()
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.set_max_width(wmax - 20.0);
                                    ui.add(egui::Label::new(heading(b, 12.0)).wrap());
                                });
                        }
                        egui::Frame::new()
                            .fill(Color32::from_rgba_unmultiplied(11, 22, 34, 215))
                            .stroke(Stroke::new(2.0, theme::EDGE))
                            .inner_margin(egui::Margin::symmetric(8, 4))
                            .show(ui, |ui| {
                                ui.label(RichText::new(goal_text).font(pixel(12.0)).color(INK));
                            });
                        for (text, _, bad) in g.coach.toasts.iter().rev().take(
                            if ctx.content_rect().width() > 700.0 {
                                1
                            } else {
                                2
                            },
                        ) {
                            theme::panel()
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.set_max_width(wmax - 20.0);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(text).font(pixel(11.0)).color(if *bad {
                                                BAD
                                            } else {
                                                TEXT
                                            }),
                                        )
                                        .wrap(),
                                    );
                                });
                        }
                    });
            }
            if !compact {
                egui::Area::new(egui::Id::new("goal"))
                    .anchor(Align2::LEFT_TOP, vec2(14.0, 96.0))
                    .show(ctx, |ui| {
                        theme::panel().show(ui, |ui| {
                            ui.set_width(320.0);
                            label(ui, "GOAL", 12.0);
                            match goal {
                                Some(n) => ui
                                    .label(heading(&format!("REACH {n} PEOPLE ({pop} NOW)"), 16.0)),
                                None => ui.label(heading("KEEP YOUR TOWN GROWING", 16.0)),
                            };
                            if let Some(t) = tip {
                                ui.add_space(4.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(t).font(pixel(11.0)).color(TEXT),
                                    )
                                    .wrap(),
                                );
                                if ui
                                    .button(RichText::new("GOT IT").font(pixel(11.0)).color(INK))
                                    .clicked()
                                {
                                    hide_tip = true;
                                }
                            }
                        });
                    });
            }
            if hide_tip {
                g.coach.tip_hidden = true;
            }
            // event feed
            if !compact && !g.coach.toasts.is_empty() {
                egui::Area::new(egui::Id::new("toasts"))
                    .anchor(Align2::RIGHT_TOP, vec2(-14.0, 96.0))
                    .interactable(false)
                    .show(ctx, |ui| {
                        for (text, _, bad) in g.coach.toasts.iter().rev() {
                            theme::panel().show(ui, |ui| {
                                ui.set_width(300.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(text).font(pixel(12.0)).color(if *bad {
                                            BAD
                                        } else {
                                            TEXT
                                        }),
                                    )
                                    .wrap(),
                                );
                            });
                        }
                    });
            }
            let screen_w = w as f32 / ppp;
            if !compact || g.ui.levers_open {
                g.levers_ui(ctx);
            }
            if g.voters_open {
                g.voters_ui(ctx, screen_w);
            }
            if g.map_open {
                g.map_sheet(ctx);
            }
            if g.ui.budget_open {
                g.budget_ui(ctx);
            }
            if g.ui.loans_open {
                g.loans_ui(ctx);
            }
            if g.ui.build_open {
                g.build_ui(ctx, screen_w);
            }
            if g.ui.stats_open {
                g.stats_ui(ctx, screen_w);
            }
            if g.ui.news_open {
                g.news_ui(ctx, screen_w);
            }
            g.legend_ui(ctx);
            // why panel
            if let Some(sel) = g.sel {
                let (title, status, built, reason) = match sel {
                    Sel::Hall => (
                        "Town hall".to_string(),
                        "your seat".to_string(),
                        None,
                        "You placed it at an old crossroads. The town grows outward from here."
                            .to_string(),
                    ),
                    Sel::Lot(i) => {
                        let y = g.sim.why(i);
                        (y.title, y.status, y.built, y.reason)
                    }
                };
                let modal = g.menu_open || g.election.is_some() || g.sim.is_game_over();
                if modal || (compact && g.any_sheet()) {
                    // a sheet or a modal screen is open: the phone has room for one
                } else if compact {
                    let mut close = false;
                    let st = status.to_uppercase();
                    let bl = built.map(|b| b.to_uppercase());
                    close |= panels::sheet(ctx, &title, |ui| {
                        label(ui, &st, 12.0);
                        if let Some(b) = &bl {
                            label(ui, b, 12.0);
                        }
                        ui.add(
                            egui::Label::new(RichText::new(&reason).size(15.0).color(TEXT)).wrap(),
                        );
                        if let Sel::Lot(i) = sel {
                            g.why_extra(ui, i);
                        }
                    });
                    if close {
                        g.sel = None;
                    }
                } else {
                    egui::Area::new(egui::Id::new("why"))
                        .anchor(Align2::RIGHT_BOTTOM, vec2(-14.0, -14.0))
                        .show(ctx, |ui| {
                            theme::panel().show(ui, |ui| {
                                ui.set_width(300.0);
                                ui.label(heading(&title.to_uppercase(), 18.0));
                                label(ui, &status.to_uppercase(), 12.0);
                                if let Some(b) = built {
                                    label(ui, &b.to_uppercase(), 12.0);
                                }
                                ui.add_space(4.0);
                                ui.add(
                                    egui::Label::new(RichText::new(reason).size(15.0).color(TEXT))
                                        .wrap(),
                                );
                                if let Sel::Lot(i) = sel {
                                    g.why_extra(ui, i);
                                }
                                ui.add_space(4.0);
                                if ui
                                    .button(RichText::new("CLOSE").font(pixel(12.0)).color(INK))
                                    .clicked()
                                {
                                    g.sel = None;
                                }
                            });
                        });
                }
            }
        }

        let help = if cfg!(target_os = "ios") {
            "DRAG TO PAN   PINCH TO ZOOM   TWIST TO ROTATE   TWO FINGERS UP OR DOWN TO TILT   TAP TO SELECT   HOLD TO ASK WHY"
        } else if compact {
            "DRAG PAN   WHEEL ZOOM   TAP SELECT"
        } else {
            "WASD PAN   Q E ROTATE   RIGHT DRAG ORBIT   WHEEL ZOOM   0 1 2 3 SPEED   SPACE PAUSE   P K O B G J V PANELS   M MAP VIEW"
        };
        if !(compact && g.any_sheet()) {
            egui::Area::new(egui::Id::new("help"))
                .anchor(
                    Align2::CENTER_BOTTOM,
                    vec2(
                        0.0,
                        if compact {
                            -(panels::DOCK_H + 4.0)
                        } else {
                            -6.0
                        },
                    ),
                )
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(11, 22, 34, 190))
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(help)
                                        .font(pixel(10.0))
                                        .color(Color32::from_rgb(200, 208, 222)),
                                )
                                .wrap(),
                            );
                        });
                });
        }

        let mut do_save: Option<Option<String>> = None;
        let mut open_load = false;
        let mut over_nav: Option<Nav> = None;
        if g.election.is_some() {
            g.election_ui(ctx);
        } else if g.sim.is_game_over() {
            over_nav = g.game_over_ui(ctx);
        }
        if g.menu_open && g.election.is_none() && !g.sim.is_game_over() {
            theme::centered(ctx, "pause", 24, |ui| {
                ui.label(heading("PAUSED", 32.0));
                label(
                    ui,
                    &format!(
                        "{}, SEED {}",
                        screens::town_name(g.preset.label()).to_uppercase(),
                        g.seed
                    ),
                    12.0,
                );
                if g.rules.is_sandbox() {
                    label(
                        ui,
                        if g.rules.unlimited_money {
                            "SANDBOX, UNLIMITED MONEY"
                        } else {
                            "SANDBOX"
                        },
                        12.0,
                    );
                }
                if let Some(n) = &who {
                    label(ui, &format!("SIGNED IN AS {}", n.to_uppercase()), 12.0);
                }
                if !g.menu_note.is_empty() {
                    ui.add(
                        egui::Label::new(RichText::new(&g.menu_note).size(14.0).color(
                            if g.menu_note.starts_with("Saved") {
                                theme::GOOD
                            } else {
                                BAD
                            },
                        ))
                        .wrap(),
                    );
                }
                if theme::button(ui, "RESUME").clicked() {
                    g.menu_open = false;
                    g.menu_note.clear();
                }
                if g.placed {
                    if theme::button(ui, "SAVE").clicked() {
                        do_save = Some(None);
                    }
                    let mut cancel_as = false;
                    if let Some(text) = g.save_as.as_mut() {
                        ui.add(
                            egui::TextEdit::singleline(text)
                                .desired_width(220.0)
                                .hint_text("Name this save")
                                .font(pixel(16.0)),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("SAVE").font(pixel(14.0)).color(INK),
                                    )
                                    .min_size(vec2(100.0, theme::TOUCH)),
                                )
                                .clicked()
                                && !text.trim().is_empty()
                            {
                                do_save = Some(Some(text.trim().to_string()));
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("CANCEL").font(pixel(14.0)).color(INK),
                                    )
                                    .min_size(vec2(100.0, theme::TOUCH)),
                                )
                                .clicked()
                            {
                                cancel_as = true;
                            }
                        });
                    } else if theme::button(ui, "SAVE AS").clicked() {
                        g.save_as = Some(g.name.clone());
                    }
                    if cancel_as {
                        g.save_as = None;
                    }
                }
                if theme::button(ui, "LOAD").clicked() {
                    open_load = true;
                }
                if theme::button(ui, "SETTINGS").clicked() {
                    open_settings = true;
                }
                if theme::button(ui, "QUIT TO MENU").clicked() {
                    to_menu = true;
                }
                #[cfg(not(target_os = "ios"))]
                if theme::button(ui, "QUIT GAME").clicked() {
                    quit = true;
                }
                label(ui, "THE GAME ALSO SAVES ITSELF EVERY GAME YEAR", 10.0);
            });
        }
        if let Some(name) = do_save {
            let (msg, bad) = self.save_game(name);
            if let Some(g) = self.game.as_mut() {
                g.menu_note = msg;
                if !bad {
                    g.save_as = None;
                }
            }
        }
        if open_load {
            self.refresh_saves();
            self.load_note.clear();
            self.load_from_game = true;
            self.screen = Screen::Load;
        }
        match over_nav {
            Some(Nav::NewTown) => {
                self.game = None;
                self.screen = Screen::NewTown;
            }
            Some(Nav::Load) => {
                self.refresh_saves();
                self.load_note.clear();
                self.load_from_game = true;
                self.screen = Screen::Load;
            }
            Some(Nav::Menu) => {
                self.game = None;
                self.screen = Screen::Menu;
            }
            None => {}
        }
        if open_settings {
            g_menu_close(self);
            self.settings_from_game = true;
            self.screen = Screen::Settings;
        }
        if to_menu {
            self.game = None;
            self.screen = Screen::Menu;
        }
        if quit {
            self.quit = true;
        }
    }
}

impl Game {
    #[allow(clippy::too_many_arguments)]
    fn new(
        town: Arc<Town>,
        preset: Preset,
        seed: u64,
        sim: Sim,
        live: LiveScene,
        placed: bool,
        sites: Vec<Pt>,
        cam: CamCtl,
        rules: us_sim::Rules,
        name: String,
    ) -> Game {
        let year = sim.day / 365;
        let election_seen = sim.last_election().map_or(-1, |e| e.day);
        Game {
            town,
            preset,
            seed,
            sim,
            live,
            placed,
            sites,
            cam,
            speed: 1,
            day_acc: 0.0,
            hour: 9.0,
            hour_frozen: false,
            sel: None,
            banner: None,
            menu_open: false,
            stats: Stats::default(),
            coach: Coach::default(),
            ui: panels::UiState::default(),
            rules,
            name,
            save_path: None,
            autosave_year: year,
            voters_open: false,
            map_open: false,
            save_as: None,
            menu_note: String::new(),
            election_seen,
            election: None,
            compact: false,
            compact_init: false,
        }
    }

    /// Calm visual day: one full day and night every few real minutes at 1x, scaled by game
    /// speed and frozen when paused. Seasons follow the game date.
    fn light_tick(&mut self, dt: f32, cycle: bool) {
        if cycle && self.placed && !self.menu_open && !self.hour_frozen {
            self.hour = (self.hour + dt * SPEEDS[self.speed] as f32 * 24.0 / LIGHT_DAY_SECONDS)
                .rem_euclid(24.0);
        }
        let hour = if cycle || self.hour_frozen {
            self.hour
        } else {
            12.0
        };
        let d = self.sim.date();
        self.live
            .scene
            .set_calendar(Some(Calendar::from_date(d.month, d.day, hour)));
    }

    fn toast(&mut self, text: impl Into<String>, bad: bool) {
        self.coach.toasts.push((text.into(), 9.0, bad));
        if self.coach.toasts.len() > 4 {
            self.coach.toasts.remove(0);
        }
    }

    /// Watches the town and speaks up when something worth knowing happens.
    fn coach_tick(&mut self, dt: f32) {
        for t in self.coach.toasts.iter_mut() {
            t.1 -= dt;
        }
        self.coach.toasts.retain(|t| t.1 > 0.0);
        self.coach.reveal_cool -= dt;
        self.coach.warn_cool -= dt;
        self.coach.scan_t -= dt;

        // new streets opening up
        let left = self.sim.additions_left();
        if let Some(prev) = self.coach.last_left
            && left < prev
        {
            self.coach.reveal_acc += prev - left;
        }
        self.coach.last_left = Some(left);
        if self.coach.reveal_acc > 0 && self.coach.reveal_cool <= 0.0 {
            let n = self.coach.reveal_acc;
            self.coach.reveal_acc = 0;
            self.coach.reveal_cool = 30.0;
            self.toast(
                format!(
                    "A NEW NEIGHBOURHOOD OPENS: {n} NEW STREET{}",
                    if n == 1 { "" } else { "S" }
                ),
                false,
            );
        }

        // population goals
        while self.coach.goal_idx < POP_GOALS.len()
            && self.stats.pop >= POP_GOALS[self.coach.goal_idx]
        {
            let goal = POP_GOALS[self.coach.goal_idx];
            self.coach.goal_idx += 1;
            self.toast(format!("GOAL REACHED: {goal} PEOPLE LIVE HERE"), false);
        }

        // first of each landmark kind
        if self.coach.scan_t <= 0.0 {
            self.coach.scan_t = 0.5;
            let mut found = [false; 4];
            for l in self.sim.lots() {
                if l.exempt || !matches!(l.phase, Phase::Built { .. }) {
                    continue;
                }
                match l.kind {
                    LotKind::Shop => found[0] = true,
                    LotKind::School => found[1] = true,
                    LotKind::Church => found[2] = true,
                    LotKind::Fire => found[3] = true,
                    _ => {}
                }
            }
            const MSG: [&str; 4] = [
                "THE FIRST SHOP OPENS. SHOPS BRING JOBS AND TAX",
                "THE FIRST SCHOOL OPENS",
                "THE FIRST CHURCH IS BUILT",
                "THE FIRE STATION IS READY",
            ];
            for i in 0..4 {
                if found[i] && !self.coach.seen_kind[i] {
                    self.coach.seen_kind[i] = true;
                    self.toast(MSG[i], false);
                }
            }
        }

        // money
        let net = self.stats.income - self.stats.expense;
        if self.sim.treasury < 0 {
            if !self.coach.neg_flagged {
                self.coach.neg_flagged = true;
                self.toast(
                    "THE TREASURY IS BELOW ZERO. RAISE THE TAX OR SLOW DOWN",
                    true,
                );
            }
        } else {
            self.coach.neg_flagged = false;
            if net < 0 && self.coach.warn_cool <= 0.0 {
                let months = self.sim.treasury / (-net).max(1);
                if months < 6 {
                    self.coach.warn_cool = 60.0;
                    self.toast(
                        format!(
                            "THE TREASURY RUNS OUT IN ABOUT {} MONTH{} AT THIS RATE. RAISE THE TAX",
                            months.max(1),
                            if months <= 1 { "" } else { "S" }
                        ),
                        true,
                    );
                }
            }
        }
    }

    fn place(&mut self, site: Pt) {
        let mut sim = Sim::new_with_rules(&self.town, site, self.seed, 2, self.rules);
        let mut live = LiveScene::new(self.town.clone(), &sim, true);
        live.sync(&mut sim, 1.0e9);
        self.sim = sim;
        self.live = live;
        self.placed = true;
        self.speed = 2;
        self.sel = if self.compact { None } else { Some(Sel::Hall) };
        self.cam
            .look_at(Vec3::new(site.x as f32, site.y as f32, 0.0), 650.0);
        self.banner = Some((
            "TOWN HALL PLACED. THE MAIN STREET JOINS THE HIGHWAY.".into(),
            7.0,
        ));
    }

    fn select_at(&mut self, p: Pt) {
        if p.dist(self.sim.hall) < self.town.hall_radius + 10.0 {
            self.sel = Some(Sel::Hall);
            return;
        }
        let mut best: Option<(f64, usize)> = None;
        for (i, l) in self.sim.lots().iter().enumerate() {
            if l.phase == Phase::Hidden {
                continue;
            }
            let d = l.pos.dist(p) - l.radius;
            if d < 3.0 && best.is_none_or(|b| d < b.0) {
                best = Some((d, i));
            }
        }
        self.sel = best.map(|b| Sel::Lot(b.1));
    }

    fn refresh_stats(&mut self) {
        let (income, expense) = self.sim.monthly_flow();
        self.stats = Stats {
            pop: self.sim.population(),
            built: self.sim.built_count(),
            empty: self.sim.empty_count(),
            building: self.sim.building_count(),
            income,
            expense,
        };
    }
}

/// A short money figure for the narrow top bar: 1.2M, 340K, 950.
fn short_money(v: i64) -> String {
    let (a, sign) = (v.abs(), if v < 0 { "-" } else { "" });
    if a >= 10_000_000 {
        format!("{sign}{}M", a / 1_000_000)
    } else if a >= 1_000_000 {
        format!("{sign}{:.1}M", a as f64 / 1e6)
    } else if a >= 10_000 {
        format!("{sign}{}K", a / 1000)
    } else if a >= 1000 {
        format!("{sign}{:.1}K", a as f64 / 1e3)
    } else {
        format!("{sign}{a}")
    }
}

fn g_menu_close(st: &mut State) {
    if let Some(g) = st.game.as_mut() {
        g.menu_open = false;
    }
}
