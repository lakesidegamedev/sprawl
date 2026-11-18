//! App state: screens, input, the running town and every piece of UI.

#![allow(deprecated)]

use crate::cam::CamCtl;
use crate::theme::{self, ACCENT, BAD, INK, TEXT, heading, ui_font};
use crate::world::LiveScene;
use egui::{Align2, Color32, Pos2, RichText, Stroke, vec2};
use glam::{Vec2, Vec3};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use us_gen::{LotKind, Preset, Pt, Town, generate, pt};
use us_render::{Calendar, Camera, Scene, Style, project};
use us_sim::{Difficulty, Phase, Sim, hall_sites};
use winit::keyboard::KeyCode;

mod charts;
mod chrome;
mod districts;
mod goals;
mod growth_banner;
mod panels;
mod screens;
mod trams;
mod weather_fx;

use screens::{Mode, Nav};
pub mod touch;

const DAY_SECONDS: f64 = 1.3;
const SPEEDS: [f64; 4] = [0.0, 1.0, 3.0, 8.0];
/// Real seconds for one full visual day and night at 1x.
const LIGHT_DAY_SECONDS: f32 = 240.0;
const SPEED_LABELS: [&str; 4] = ["II", "1x", "3x", "8x"];

pub struct Settings {
    pub fullscreen: bool,
    pub ui_scale: f32,
    pub high_shadows: bool,
    pub balanced_world_resolution: bool,
    pub(crate) audio: crate::audio::Preferences,
    /// Day and night cycle (true) or always day (false).
    pub day_night: bool,
    /// Fewer animations: panels, figures and the camera jump instead of easing.
    pub reduce_motion: bool,
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
    ui_ppp: f32,
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
    difficulty: Difficulty,
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
    ui.label(RichText::new(text).font(ui_font(size)).color(TEXT));
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
                balanced_world_resolution: false,
                audio: crate::audio::Preferences::load(),
                day_night: true,
                reduce_motion: std::env::var("URBAN_SPRAWL_REDUCE_MOTION").is_ok_and(|v| v == "1"),
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
            difficulty: Difficulty::Normal,
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
        cam.look_at(Vec2::new(hall.x as f32, hall.y as f32), 800.0);
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
        if !g.map_input_available() {
            return;
        }
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

    pub fn pointer_pixels(&self) -> (f32, f32) {
        self.cursor
    }

    pub fn mouse_button(&mut self, left: bool, down: bool, on_ui: bool) {
        let slot = if left {
            &mut self.left
        } else {
            &mut self.right
        };
        if down {
            *slot = Some(Press {
                dragged: false,
                on_ui,
                travel: 0.0,
            });
        } else if let Some(p) = slot.take()
            && !p.dragged
            && !p.on_ui
            && !on_ui
            && self.screen == Screen::Game
        {
            if left {
                self.click = Some(self.cursor);
            } else if let Some(g) = self.game.as_mut() {
                // a still right click drops the road being drawn
                g.road_cancel();
            }
        }
    }

    pub fn wheel(&mut self, lines: f32) {
        if self.screen == Screen::Game
            && let Some(g) = self.game.as_mut().filter(|g| g.map_input_available())
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
        self.style.window_world_scale = if self.settings.balanced_world_resolution {
            0.8
        } else {
            1.0
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
                d.cam.update(dt, d.live.land());
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
        if g.map_input_available() {
            g.cam.keys(dt, pan, rot, fast);
        }
        g.cam.update(dt, g.live.land());
        if let Some((b, t)) = g.banner.as_mut() {
            *t -= dt;
            if *t <= 0.0 {
                let _ = b;
                g.banner = None;
            }
        }
        if let Some(c) = click.filter(|_| g.map_input_available()) {
            let aspect = w as f32 / h as f32;
            let view = g.cam.camera().view(aspect);
            let ndc = (c.0 / w as f32 * 2.0 - 1.0, 1.0 - c.1 / h as f32 * 2.0);
            if !g.placed {
                let cursor = egui::pos2(c.0 / g.ui_ppp, c.1 / g.ui_ppp);
                let site = g
                    .hall_markers(&view, w, h, g.ui_ppp)
                    .map(|(site, pos)| (site, pos.distance(cursor)))
                    .filter(|&(_, d)| d <= g.hall_hit_radius())
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((site, _)) = site {
                    g.place(site);
                }
            } else if let Some(p) = g.live.scene.pick(&view, ndc) {
                if let Some(kind) = g.ui.tool {
                    let site = g.sim.snap_civic_site(kind, pt(p.x as f64, p.y as f64));
                    if g.compact {
                        g.ui.civic_site = Some(site);
                    } else {
                        g.place_tool(site);
                    }
                } else if g.road_active().is_some() {
                    g.road_click(pt(p.x as f64, p.y as f64));
                } else if g.ui.paint.is_some() {
                    g.paint_at(pt(p.x as f64, p.y as f64));
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
                    Ok(_) => g.toast("Autosaved", false),
                    Err(e) => g.toast(format!("Autosave failed: {e}").to_string(), true),
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

    pub(crate) fn sound_scene(&self) -> crate::audio::SoundScene {
        let Some(game) = self.game.as_ref().filter(|g| g.placed) else {
            return crate::audio::SoundScene::default();
        };
        let camera = game.cam.camera();
        let focus = camera.target.truncate();
        let fade = 1.0 - ((camera.distance - 175.0) / 620.0).clamp(0.0, 1.0);
        let mut traffic: f32 = 0.0;
        let mut pan: f32 = 0.0;
        let mut weight: f32 = 0.0;
        let yaw = camera.yaw_deg.to_radians();
        let right = glam::Vec2::new(yaw.cos(), -yaw.sin());
        if fade > 0.0 {
            for (i, (road, flow)) in game
                .sim
                .road_runs()
                .iter()
                .zip(game.sim.road_traffic())
                .enumerate()
            {
                if flow.volume < 1.0 || !game.sim.road_open(i) {
                    continue;
                }
                for pair in road.points.windows(2) {
                    let a = glam::Vec2::new(pair[0].x as f32, pair[0].y as f32);
                    let b = glam::Vec2::new(pair[1].x as f32, pair[1].y as f32);
                    let edge = b - a;
                    let p = a + edge
                        * ((focus - a).dot(edge) / edge.length_squared().max(0.1)).clamp(0.0, 1.0);
                    let relative = p - focus;
                    let near = (1.0 - relative.length() / 120.0).max(0.0);
                    let level = (flow.volume.max(0.0) / 1900.0).sqrt().min(1.0) * near;
                    traffic = traffic.max(level);
                    pan += relative.dot(right) / 120.0 * level;
                    weight += level;
                }
            }
        }
        let weather = game.sim.weather();
        let night = game.live.scene.calendar.map_or(0.0, |c| c.darkness());
        crate::audio::SoundScene {
            night,
            wind: (weather.wind_speed() / 16.0).clamp(0.0, 1.0) * 0.7 + 0.1,
            rain: game.sim.rain(),
            traffic: traffic * (1.0 - night * 0.58) * fade,
            traffic_pan: (pan / weight.max(0.01)).clamp(-1.0, 1.0),
            built: Some(game.stats.built),
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
            self.mode.rules().with_difficulty(self.difficulty),
            name,
        ));
        self.screen = Screen::Game;
    }

    /// Continue a game read from a save.
    pub(super) fn start_loaded(&mut self, sim: Sim, path: Option<std::path::PathBuf>) {
        let t = sim.town();
        let (preset, seed) = (t.preset, t.seed);
        let town = Arc::new(t.clone());
        let mut sim = sim;
        let mut live = LiveScene::new(town.clone(), &sim, true);
        live.sync(&mut sim, 1.0e9);
        let mut cam = CamCtl::overview(town.size as f32, 1.7);
        cam.look_at(Vec2::new(sim.hall.x as f32, sim.hall.y as f32), 650.0);
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
                if let Ok(v) = std::env::var("URBAN_SPRAWL_SMOKE_SEED") {
                    self.seed_text = v;
                }
                if let Ok(v) = std::env::var("URBAN_SPRAWL_SMOKE_PRESET")
                    && let Some(p) = Preset::ALL.iter().find(|p| p.name() == v)
                {
                    self.preset = *p;
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
                        // URBAN_SPRAWL_SMOKE_TEND=1 looks after the town as it grows (services and
                        // taxes each month), so it gets past its first limits
                        let tend = std::env::var("URBAN_SPRAWL_SMOKE_TEND").is_ok_and(|v| v == "1");
                        let mut round = 0;
                        for d in 0..days {
                            g.sim.step_day();
                            if tend && d % 30 == 5 {
                                us_sim::tend(&mut g.sim, &mut round);
                            }
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
                        "smoke: day {} built {} pop {} treasury {} chunks {} tris {} ground {} fps {:.1} worst-frame {:.0} ms sync max {:.1} ms avg {:.2} ms",
                        g.sim.day,
                        g.sim.built_count(),
                        g.sim.population(),
                        g.sim.treasury,
                        g.live.scene.chunks.len(),
                        g.live.triangles,
                        g.live.ground_triangles(),
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
        theme::set_reduce_motion(self.settings.reduce_motion);
        let anim = if self.settings.reduce_motion {
            0.0
        } else {
            0.15
        };
        if (ctx.global_style().animation_time - anim).abs() > f32::EPSILON {
            ctx.global_style_mut(|s| s.animation_time = anim);
        }
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
                            ui.label(heading("Building the countryside...", 24.0));
                        });
                    });
            }
            // URBAN_SPRAWL_SMOKE_BARE=1: a picture of the town with no interface over it
            Screen::Game
                if self.smoke.as_ref().is_some_and(|s| s.stage >= 2)
                    && std::env::var("URBAN_SPRAWL_SMOKE_BARE").is_ok_and(|v| v == "1") => {}
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
                "Urban sprawl",
                if short {
                    28.0
                } else if compact {
                    36.0
                } else {
                    56.0
                },
            ));
            ui.label(
                RichText::new(crate::VERSION_LABEL.to_string())
                    .font(ui_font(if short { 12.0 } else { 16.0 }))
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
                        label(ui, &format!("Signed in as {}", a.name), 14.0);
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
                    && theme::button(ui, "Continue").clicked()
                {
                    self.load_save(&c.path);
                }
                if theme::button(ui, "New town").clicked() {
                    self.screen = Screen::NewTown;
                }
                if theme::button(ui, "Load game").clicked() {
                    self.refresh_saves();
                    self.load_note.clear();
                    self.screen = Screen::Load;
                }
                if theme::button(ui, "Settings").clicked() {
                    self.screen = Screen::Settings;
                }
                // iOS apps are closed from the home screen, never by the app
                #[cfg(not(target_os = "ios"))]
                if theme::button(ui, "Quit").clicked() {
                    self.quit = true;
                }
            });
        });
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        theme::centered(ctx, "settings", 24, |ui| {
            ui.label(heading("Settings", 32.0));
            ui.add_space(6.0);
            #[cfg(not(target_os = "ios"))]
            {
                label(ui, "Window", 14.0);
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.settings.fullscreen,
                        false,
                        RichText::new("Windowed").font(ui_font(16.0)),
                    );
                    ui.selectable_value(
                        &mut self.settings.fullscreen,
                        true,
                        RichText::new("Fullscreen").font(ui_font(16.0)),
                    );
                });
            }
            label(ui, "UI scale", 14.0);
            ui.add(egui::Slider::new(&mut self.settings.ui_scale, 0.8..=1.8).step_by(0.1));
            label(ui, "Shadows", 14.0);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.settings.high_shadows,
                    true,
                    RichText::new("Crisp").font(ui_font(16.0)),
                );
                ui.selectable_value(
                    &mut self.settings.high_shadows,
                    false,
                    RichText::new("Fast").font(ui_font(16.0)),
                );
            });
            label(ui, "World resolution", 14.0);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.settings.balanced_world_resolution,
                    false,
                    "Native",
                );
                ui.selectable_value(
                    &mut self.settings.balanced_world_resolution,
                    true,
                    "Balanced",
                );
            });
            label(ui, "Day and night", 14.0);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.settings.day_night,
                    true,
                    RichText::new("Cycle").font(ui_font(16.0)),
                );
                ui.selectable_value(
                    &mut self.settings.day_night,
                    false,
                    RichText::new("Always day").font(ui_font(16.0)),
                );
            });
            label(ui, "Reduce motion", 14.0);
            ui.horizontal(|ui| {
                ui.selectable_value(
                    &mut self.settings.reduce_motion,
                    false,
                    RichText::new("Off").font(ui_font(16.0)),
                );
                ui.selectable_value(
                    &mut self.settings.reduce_motion,
                    true,
                    RichText::new("On").font(ui_font(16.0)),
                );
            });
            ui.add_space(8.0);
            if cfg!(target_os = "ios") || theme::is_compact(ctx) {
                ui.collapsing("Map controls", |ui| {
                    label(ui, "Drag to pan, pinch to zoom, and twist to rotate.", 13.0);
                    label(
                        ui,
                        "Drag two fingers together to tilt. Tap to select or hold to inspect.",
                        13.0,
                    );
                });
            }
            label(ui, "Audio", 14.0);
            let prefs = &mut self.settings.audio;
            let mut commit = ui.checkbox(&mut prefs.muted, "Mute").changed();
            for (name, value) in [
                ("Music", &mut prefs.music),
                ("Ambience", &mut prefs.ambience),
                ("Effects", &mut prefs.effects),
            ] {
                let slider = ui.add(egui::Slider::new(value, 0.0..=1.0).step_by(0.05).text(name));
                commit |= (slider.changed() && !slider.dragged()) || slider.drag_stopped();
            }
            if commit && let Err(err) = prefs.save() {
                crate::log(&format!("could not store audio preferences: {err}"));
            }
            #[cfg(not(target_os = "ios"))]
            {
                label(ui, "F11 toggles fullscreen in game", 12.0);
                ui.add_space(8.0);
                self.account_ui(ui);
            }
            ui.add_space(10.0);
            if theme::button(ui, "Back").clicked() {
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
        label(ui, "Lakeside account", 14.0);
        if let Some(a) = &status.account {
            label(ui, &format!("Signed in to Lakeside as {}", a.name), 12.0);
            if theme::button(ui, "Sign out").clicked() {
                self.lakeside.sign_out();
                self.note("Signed out.");
            }
        } else if status.handoff_pending {
            label(ui, "Checking your Lakeside launcher sign-in...", 12.0);
        } else if busy {
            label(ui, "Finish signing in in your browser.", 12.0);
            if theme::button(ui, "Cancel").clicked() {
                self.lakeside.cancel_sign_in();
            }
        } else if theme::button(ui, "Sign in").clicked() {
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
        label(ui, "Updates", 14.0);
        if status.launcher {
            label(
                ui,
                "The Lakeside launcher keeps this copy up to date.",
                12.0,
            );
        } else if status.updater_enabled
            && !busy
            && theme::button(ui, "Check for updates").clicked()
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
        g.ui_ppp = ppp;
        if g.placed {
            g.ui.growth.refresh(&g.sim, ctx.input(|i| i.time));
        }
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
                .filter(|_| !ctx.egui_wants_pointer_input());
            let markers: Vec<_> = g.hall_markers(&view, w, h, ppp).collect();
            let nearest = cursor.and_then(|cursor| {
                markers
                    .iter()
                    .map(|&(_, pos)| (pos, pos.distance(cursor)))
                    .filter(|&(_, distance)| distance <= g.hall_hit_radius())
                    .min_by(|a, b| a.1.total_cmp(&b.1))
                    .map(|(pos, _)| pos)
            });
            for &(_, p) in &markers {
                let hot = nearest == Some(p);
                let r = if hot { 13.0 } else { 8.0 };
                painter.rect_filled(
                    egui::Rect::from_center_size(p, vec2(r * 2.0 + 4.0, r * 2.0 + 4.0)),
                    egui::CornerRadius::ZERO,
                    theme::LINE,
                );
                painter.rect_filled(
                    egui::Rect::from_center_size(p, vec2(r * 2.0, r * 2.0)),
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
            let c = Vec3::new(
                pos.x as f32,
                pos.y as f32,
                g.live.ground_at(pos.x as f32, pos.y as f32),
            );
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
        let hud = if !compact {
            g.desktop_bar(ui, &who, fps)
        } else {
            egui::Panel::top("hud").frame(bar_frame).show(ui, |ui| {
                {
                    let wide = ui.available_width() >= 640.0;
                    let small =
                        |s: String, col: Color32| RichText::new(s).font(ui_font(12.0)).color(col);
                    let row1 = |ui: &mut egui::Ui, g: &Game| {
                        if g.placed {
                            // month and year: the day does not earn its room on a phone
                            let d = g.sim.date().text().to_string();
                            ui.label(small(
                                d.split_once(' ').map_or(d.clone(), |x| x.1.to_string()),
                                INK,
                            ));
                            ui.label(small(format!("Pop {}", g.stats.pop), INK));
                            ui.label(small(
                                if g.rules.unlimited_money {
                                    "Unlimited".into()
                                } else {
                                    format!("${}", short_money(g.sim.treasury))
                                },
                                if !g.rules.unlimited_money && g.sim.treasury < 0 {
                                    BAD
                                } else {
                                    INK
                                },
                            ));
                            let net = g.stats.income - g.stats.expense;
                            ui.label(small(
                                format!(
                                    "{}${}/mo",
                                    if net < 0 { "-" } else { "+" },
                                    short_money(net.abs())
                                ),
                                if net < 0 { BAD } else { theme::GOOD },
                            ));
                        } else {
                            ui.label(small("Place the town hall".into(), INK));
                        }
                    };
                    let narrow = ui.available_width() < 340.0;
                    let tight = ui.available_width() < 256.0;
                    let row2 = |ui: &mut egui::Ui, g: &mut Game| {
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let chip = |ui: &mut egui::Ui, g: &mut Game| {
                            if g.placed && !narrow {
                                g.approval_chip(ui, true);
                            }
                        };
                        split_row(ui, theme::TOUCH, g, chip, |ui, g| {
                            ui.spacing_mut().item_spacing.x = 4.0;
                            if chrome::menu_button(ui, vec2(48.0, theme::TOUCH)).clicked() {
                                g.menu_open = true;
                            }
                            if g.placed && tight {
                                let speed = g.speed.max(1);
                                if chrome::speed_button(
                                    ui,
                                    speed,
                                    g.speed != 0,
                                    vec2(52.0, theme::TOUCH),
                                )
                                .clicked()
                                {
                                    g.speed = if g.speed == 0 { 1 } else { g.speed % 3 + 1 };
                                }
                                if chrome::speed_button(
                                    ui,
                                    0,
                                    g.speed == 0,
                                    vec2(theme::TOUCH, theme::TOUCH),
                                )
                                .clicked()
                                {
                                    g.speed = if g.speed == 0 { 1 } else { 0 };
                                }
                            } else if g.placed {
                                for i in (0..4).rev() {
                                    let on = g.speed == i;
                                    if chrome::speed_button(
                                        ui,
                                        i,
                                        on,
                                        vec2(theme::TOUCH, theme::TOUCH),
                                    )
                                    .clicked()
                                    {
                                        g.speed = i;
                                    }
                                }
                            }
                        });
                    };
                    if wide {
                        split_row(
                            ui,
                            theme::TOUCH,
                            g,
                            |ui, g| {
                                ui.spacing_mut().item_spacing.x = 10.0;
                                row1(ui, g);
                            },
                            |ui, g| {
                                if chrome::menu_button(ui, vec2(48.0, theme::TOUCH)).clicked() {
                                    g.menu_open = true;
                                }
                                if g.placed {
                                    for i in (0..4).rev() {
                                        let on = g.speed == i;
                                        if chrome::speed_button(
                                            ui,
                                            i,
                                            on,
                                            vec2(theme::TOUCH, theme::TOUCH),
                                        )
                                        .clicked()
                                        {
                                            g.speed = i;
                                        }
                                    }
                                    ui.add_space(6.0);
                                    g.approval_chip(ui, true);
                                }
                            },
                        );
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            row1(ui, g);
                            if narrow && g.placed {
                                g.approval_chip(ui, true);
                            }
                        });
                        row2(ui, g);
                    }
                }
            })
        };
        let top_y = hud.response.rect.bottom() + if compact { 6.0 } else { 12.0 };

        if g.placed && compact {
            let dock = egui::Panel::bottom("dock")
                .frame(theme::bar().inner_margin(egui::Margin::symmetric(4, 3)))
                .show(ui, |ui| g.dock_ui(ui));
            // sheets are placed from where the bar really is, so they never cover it
            panels::set_dock_top(ctx, dock.response.rect.top());
        } else if g.placed {
            g.rail_ui(ui);
        }

        if g.placed {
            g.selection_reach(&painter, &view, w, h, ppp);
            g.ghost_ui(ctx, &painter, &view, w, h);
        }

        // banner
        let banner = if !g.placed {
            Some(if compact {
                "Tap a crossroads to place the town hall".to_string()
            } else {
                "Click a crossroads to place the town hall".to_string()
            })
        } else {
            g.banner.as_ref().map(|b| b.0.clone())
        };
        let stacked_banner = banner.as_deref().filter(|_| g.placed);
        if let Some(b) = banner.as_deref().filter(|_| !g.placed) {
            egui::Area::new(egui::Id::new("banner"))
                .anchor(Align2::CENTER_TOP, vec2(0.0, top_y))
                .show(ctx, |ui| {
                    theme::panel().show(ui, |ui| {
                        if compact {
                            ui.set_max_width(ctx.content_rect().width() - 40.0);
                            ui.add(
                                egui::Label::new(RichText::new(b).font(ui_font(14.0)).color(INK)).wrap(),
                            );
                            if !g.placed {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new("Tap an old crossroads (white squares).")
                                            .font(ui_font(11.0))
                                            .color(TEXT),
                                    )
                                    .wrap(),
                                );
                            }
                            return;
                        }
                        ui.set_max_width(440.0);
                        ui.add(
                            egui::Label::new(RichText::new(b).font(ui_font(16.0)).color(INK)).wrap(),
                        );
                        if !g.placed {
                            label(ui, "The white squares are old crossroads. The one nearest your cursor glows orange. Your main street will run to the highway.", 12.0);
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
                    "People move in as buildings finish. Click any building to see why it was built.",
                )
            } else if day < 150 {
                Some("Taxes pay for the town. Watch the treasury and the monthly number beside it.")
            } else if day < 300 {
                Some(
                    "Low tax grows faster but fills the treasury slower. High tax empties buildings. Try the slider.",
                )
            } else {
                None
            };
            let pop = g.stats.pop;
            let unlock_hint = g
                .sim
                .next_unlock()
                .map(|(k, n)| format!("Next: {} at {} people", k.label(), charts::group(n)));
            let mut hide_tip = false;
            // the First goals card takes the place of the milestone goal while it is showing
            let first_goals = g.sim.goals_visible();
            let mut open_goal_build = None;
            if compact && !g.any_sheet() && g.sel.is_none() {
                // Keep the transient map notices clear of open sheets.
                let wmax = (ctx.content_rect().width() - 24.0).min(340.0);
                egui::Area::new(egui::Id::new("goal"))
                    .anchor(Align2::LEFT_TOP, vec2(8.0, top_y))
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.set_max_width(wmax);
                        let mut goal_text = match goal {
                            Some(n) => format!("Goal {n} people ({pop})"),
                            None => "Keep your town growing".to_string(),
                        };
                        if let Some(u) = &unlock_hint {
                            goal_text.push('\n');
                            goal_text.push_str(u);
                        }
                        if let Some(b) = stacked_banner {
                            theme::panel()
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.set_max_width(wmax - 20.0);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(b).font(ui_font(12.0)).color(INK),
                                        )
                                        .wrap(),
                                    );
                                });
                        }
                        if !first_goals {
                            egui::Frame::new()
                                .fill(Color32::from_rgba_unmultiplied(11, 22, 34, 215))
                                .stroke(Stroke::new(1.0, theme::EDGE))
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(goal_text).font(ui_font(12.0)).color(INK),
                                    );
                                });
                        }
                        for (text, _, bad) in g.coach.toasts.iter().rev().take(1) {
                            theme::panel()
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .show(ui, |ui| {
                                    ui.set_max_width(wmax - 20.0);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(text)
                                                .font(ui_font(11.0))
                                                .color(if *bad { BAD } else { TEXT }),
                                        )
                                        .wrap(),
                                    );
                                });
                        }
                    });
            }
            let left = chrome::RAIL_W + 14.0;
            let right = g.right_inset();
            let card_width =
                ((ctx.content_rect().width() - left - right - 16.0) * 0.5).clamp(140.0, 348.0);
            if !compact {
                egui::Area::new(egui::Id::new("goal"))
                    .anchor(Align2::LEFT_TOP, vec2(left, top_y))
                    .show(ctx, |ui| {
                        theme::panel().show(ui, |ui| {
                            ui.set_width(card_width - 28.0);
                            if first_goals {
                                open_goal_build = g.first_goals_card(ui, ctx);
                                return;
                            }
                            let goal_text =
                                |s: String| RichText::new(s).font(ui_font(16.0)).color(INK);
                            label(ui, "Goal", 12.0);
                            match goal {
                                Some(n) => {
                                    ui.label(goal_text(format!("Reach {n} people ({pop} now)")))
                                }
                                None => ui.label(goal_text("Keep your town growing".to_string())),
                            };
                            if let Some(u) = &unlock_hint {
                                label(ui, u, 11.0);
                            }
                            if let Some(t) = tip {
                                ui.add_space(4.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(t).font(ui_font(11.0)).color(TEXT),
                                    )
                                    .wrap(),
                                );
                                if ui
                                    .button(RichText::new("Got it").font(ui_font(11.0)).color(INK))
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
            if let Some(action) = open_goal_build {
                g.open_goal(action);
            }
            if !compact && g.map_input_available() {
                egui::Area::new(egui::Id::new("town-status"))
                    .anchor(Align2::RIGHT_TOP, vec2(-right, top_y))
                    .show(ctx, |ui| {
                        ui.set_max_width(card_width);
                        g.growth_panel(ui, card_width);
                        if let Some(banner) = stacked_banner {
                            theme::panel().show(ui, |ui| {
                                panels::wrapped(ui, banner, 12.0, TEXT);
                            });
                        } else if let Some((text, _, bad)) = g.coach.toasts.last() {
                            theme::panel().show(ui, |ui| {
                                panels::wrapped(ui, text, 12.0, if *bad { BAD } else { TEXT });
                            });
                        }
                    });
            }
            let screen_w = w as f32 / ppp;
            if g.ui.levers_open {
                g.levers_ui(ctx);
            }
            if g.voters_open {
                g.voters_ui(ctx, screen_w);
            }
            if g.map_open {
                g.map_sheet(ctx);
            }
            if g.ui.rivals_open {
                g.rivals_ui(ctx, screen_w);
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
            if g.ui.districts_open {
                g.districts_ui(ctx, screen_w);
            }
            if g.ui.ordinances_open {
                g.ordinances_ui(ctx, screen_w);
            }
            if g.ui.trams.open {
                g.trams_ui(ctx, screen_w);
            }
            if compact && g.ui.more_open {
                g.more_sheet(ctx);
            }
            if g.placed && !g.sim.is_game_over() {
                if compact && g.ui.tool.is_some() {
                    g.civic_placement_ui(ctx);
                } else if compact && g.sel.is_none() {
                    g.growth_banner_ui(ctx);
                    g.goals_ui(ctx);
                }
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
                    let st = chrome::sentence(&status);
                    let bl = built.as_deref().map(chrome::sentence);
                    close |= panels::sheet(ctx, &title, |ui| {
                        ui.label(RichText::new(&st).font(ui_font(12.5)).color(ACCENT));
                        if let Some(b) = &bl {
                            label(ui, b, 12.0);
                        }
                        chrome::section(ui, "Why it is here");
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
                    g.inspector_ui(ui, sel);
                }
            }
            if !compact {
                g.views_popover(ctx);
            }
        }
        let help = if cfg!(target_os = "ios") {
            if compact {
                "Drag pan   pinch zoom   twist rotate   tap select"
            } else {
                "Drag to pan   pinch to zoom   twist to rotate   two fingers up or down to tilt   tap to select   hold to ask why"
            }
        } else if compact {
            "Drag pan   pinch zoom   twist rotate   tap select"
        } else {
            "WASD pan   Q E rotate   right drag orbit   wheel zoom   0 1 2 3 speed   space pause   P K O B G J V I H R F panels   Y roads   M map view"
        };
        let show_help = !compact
            || (!g.placed
                && !g.any_sheet()
                && g.sel.is_none()
                && g.ui.tool.is_none()
                && g.ui.growth.height() <= 0.0);
        if show_help {
            egui::Area::new(egui::Id::new("help"))
                .anchor(
                    Align2::CENTER_BOTTOM,
                    vec2(
                        0.0,
                        if compact {
                            -(ctx.content_rect().bottom() - panels::dock_top(ctx) + 4.0)
                        } else {
                            -6.0
                        },
                    ),
                )
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgba_unmultiplied(11, 22, 34, 190))
                        .corner_radius(6)
                        .inner_margin(egui::Margin::symmetric(10, 4))
                        .show(ui, |ui| {
                            if !compact {
                                ui.set_max_width(
                                    (ctx.content_rect().width()
                                        - chrome::RAIL_W
                                        - chrome::INSPECTOR_W
                                        - 40.0)
                                        .max(300.0),
                                );
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(help)
                                        .font(ui_font(if compact { 11.0 } else { 10.0 }))
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
                ui.label(heading("Paused", 32.0));
                label(
                    ui,
                    &format!("{}, seed {}", screens::town_name(g.preset.label()), g.seed),
                    12.0,
                );
                if g.rules.is_sandbox() {
                    let mut unlimited = g.rules.unlimited_money;
                    if ui.checkbox(&mut unlimited, "Unlimited money").changed() {
                        g.rules.unlimited_money = unlimited;
                        g.sim.set_rules(g.rules);
                    }
                }
                if let Some(n) = &who {
                    label(ui, &format!("Signed in as {}", n), 12.0);
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
                if theme::button(ui, "Resume").clicked() {
                    g.menu_open = false;
                    g.menu_note.clear();
                }
                if g.placed {
                    if theme::button(ui, "Save").clicked() {
                        do_save = Some(None);
                    }
                    let mut cancel_as = false;
                    if let Some(text) = g.save_as.as_mut() {
                        ui.add(
                            egui::TextEdit::singleline(text)
                                .desired_width(220.0)
                                .hint_text("Name this save")
                                .font(ui_font(16.0)),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("Save").font(ui_font(14.0)).color(INK),
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
                                        RichText::new("Cancel").font(ui_font(14.0)).color(INK),
                                    )
                                    .min_size(vec2(100.0, theme::TOUCH)),
                                )
                                .clicked()
                            {
                                cancel_as = true;
                            }
                        });
                    } else if theme::button(ui, "Save as").clicked() {
                        g.save_as = Some(g.name.clone());
                    }
                    if cancel_as {
                        g.save_as = None;
                    }
                }
                if theme::button(ui, "Load").clicked() {
                    open_load = true;
                }
                if theme::button(ui, "Settings").clicked() {
                    open_settings = true;
                }
                if theme::button(ui, "Quit to menu").clicked() {
                    to_menu = true;
                }
                #[cfg(not(target_os = "ios"))]
                if theme::button(ui, "Quit game").clicked() {
                    quit = true;
                }
                label(ui, "The game also saves itself every game year", 10.0);
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
            ui_ppp: 1.0,
        }
    }

    /// Calm visual day: one full day and night every few real minutes at 1x, scaled by game
    /// speed and frozen when paused. Seasons follow the game date.
    fn light_tick(&mut self, dt: f32, cycle: bool) {
        if self.placed && !self.menu_open && !self.hour_frozen {
            self.hour = (self.hour + dt * SPEEDS[self.speed] as f32 * 24.0 / LIGHT_DAY_SECONDS)
                .rem_euclid(24.0);
        }
        let hour = if cycle || self.hour_frozen {
            self.hour
        } else {
            12.0
        };
        let d = self.sim.date();
        self.live.scene.set_traffic_hour(self.hour);
        self.live
            .scene
            .set_calendar(Some(Calendar::from_date(d.month, d.day, hour)));
        self.live.set_emergency(&self.sim, hour);
        let look = self.weather_look(hour);
        self.live.scene.set_weather(look);
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
                    "A new neighbourhood opens: {n} new street{}",
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
            self.toast(format!("Goal reached: {goal} people live here"), false);
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
                "The first shop opens. Shops bring jobs and tax",
                "The first school opens",
                "The first church is built",
                "The fire station is ready",
            ];
            for i in 0..4 {
                if found[i] && !self.coach.seen_kind[i] {
                    self.coach.seen_kind[i] = true;
                    self.toast(MSG[i], false);
                }
            }
        }

        // money
        if self.rules.unlimited_money {
            return;
        }
        let net = self.stats.income - self.stats.expense;
        if self.sim.treasury < 0 {
            if !self.coach.neg_flagged {
                self.coach.neg_flagged = true;
                self.toast(
                    "The treasury is below zero. Raise the tax or slow down",
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
                            "The treasury runs out in about {} month{} at this rate. Raise the tax",
                            months.max(1),
                            if months <= 1 { "" } else { "S" }
                        ),
                        true,
                    );
                }
            }
        }
    }

    fn map_input_available(&self) -> bool {
        !self.menu_open && self.election.is_none() && !self.sim.is_game_over()
    }

    fn hall_hit_radius(&self) -> f32 {
        if self.compact { 24.0 } else { 18.0 }
    }

    fn hall_markers<'a>(
        &'a self,
        view: &'a us_render::View,
        w: u32,
        h: u32,
        ppp: f32,
    ) -> impl Iterator<Item = (Pt, Pos2)> + 'a {
        let land = self.live.land();
        self.sites.iter().copied().filter_map(move |site| {
            let (x, y) = (site.x as f32, site.y as f32);
            let p = Vec3::new(x, y, land.height(x, y));
            let ndc = project(view, p)?;
            if ndc.0.abs() > 1.0 || ndc.1.abs() > 1.0 || !land.segment_above_ground(p, view.eye) {
                return None;
            }
            Some((site, to_screen(ndc, w, h, ppp)))
        })
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
            .look_at(Vec2::new(site.x as f32, site.y as f32), 650.0);
        self.banner = Some((
            "Town hall placed. The main street joins the highway.".into(),
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

/// One row of a bar: `right` is laid out first against the right edge, then `left` gets the room
/// that is left and is clipped to it, so the left side is never drawn under the right one.
fn split_row(
    ui: &mut egui::Ui,
    height: f32,
    g: &mut Game,
    left: impl FnOnce(&mut egui::Ui, &mut Game),
    right: impl FnOnce(&mut egui::Ui, &mut Game),
) {
    let full = ui.available_rect_before_wrap();
    let row = egui::Rect::from_min_size(full.min, vec2(full.width(), height));
    let mut r = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(row)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    right(&mut r, g);
    let used = r.min_rect().width();
    let left_rect = egui::Rect::from_min_max(
        row.min,
        egui::pos2((row.max.x - used - 12.0).max(row.min.x), row.max.y),
    );
    let mut l = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(left_rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    l.set_clip_rect(left_rect.intersect(ui.clip_rect()));
    left(&mut l, g);
    let bottom = r.min_rect().bottom().max(l.min_rect().bottom());
    ui.allocate_rect(
        egui::Rect::from_min_max(full.min, egui::pos2(full.max.x, bottom)),
        egui::Sense::hover(),
    );
}
