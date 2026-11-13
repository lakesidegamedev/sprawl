//! App state: screens, input, the running town and every piece of UI.

#![allow(deprecated)]

use crate::cam::CamCtl;
use crate::theme::{self, ACCENT, BAD, GOOD, INK, TEXT, heading, pixel};
use crate::world::LiveScene;
use egui::{Align2, Color32, Pos2, RichText, Stroke, vec2};
use glam::Vec3;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use us_gen::{Preset, Pt, Town, generate, pt};
use us_render::{Camera, Scene, Style, pick_ground, project};
use us_sim::{ABANDON_ABOVE_X10, Phase, Sim, hall_sites};
use winit::keyboard::KeyCode;

const DAY_SECONDS: f64 = 1.3;
const SPEEDS: [f64; 4] = [0.0, 1.0, 3.0, 8.0];
const SPEED_LABELS: [&str; 4] = ["II", "1X", "3X", "8X"];

pub struct Settings {
    pub fullscreen: bool,
    pub ui_scale: f32,
    pub high_shadows: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Menu,
    NewTown,
    Settings,
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
    demand: f64,
}

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
    sel: Option<Sel>,
    banner: Option<(String, f32)>,
    menu_open: bool,
    stats: Stats,
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
}

fn random_seed() -> u64 {
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

fn money(v: i64) -> String {
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

fn label(ui: &mut egui::Ui, text: &str, size: f32) {
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
            fps: 0.0,
            fps_acc: (0.0, 0),
            style: Style::default(),
            smoke,
            error: None,
            lakeside: us_lakeside::Lakeside::start(),
            account_note: Arc::new(Mutex::new(String::new())),
            account_busy: Arc::new(AtomicBool::new(false)),
        };
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
                g.select_at(pt(p.x as f64, p.y as f64));
            }
        }
        if g.placed && g.speed > 0 && !g.menu_open {
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
        g.refresh_stats();
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
        if (self.screen == Screen::Game || self.settings_from_game)
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
        self.game = Some(Game {
            town,
            preset: self.preset,
            seed,
            sim,
            live,
            placed: false,
            sites,
            cam,
            speed: 1,
            day_acc: 0.0,
            sel: None,
            banner: None,
            menu_open: false,
            stats: Stats::default(),
        });
        self.screen = Screen::Game;
    }

    fn autopilot(&mut self) {
        let Some(s) = self.smoke.as_mut() else { return };
        let t = s.start.elapsed().as_secs_f32();
        match s.stage {
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
                    if let Some(days) = std::env::var("URBAN_SPRAWL_SMOKE_DAYS")
                        .ok()
                        .and_then(|v| v.parse::<i32>().ok())
                    {
                        for _ in 0..days {
                            g.sim.step_day();
                        }
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
        match self.screen {
            Screen::Menu => self.menu_ui(&ctx),
            Screen::NewTown => self.new_town_ui(&ctx),
            Screen::Settings => self.settings_ui(&ctx),
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
        ui.vertical_centered(|ui| {
            ui.label(heading("URBAN SPRAWL", 56.0));
            ui.label(
                RichText::new(crate::VERSION_LABEL.to_uppercase())
                    .font(pixel(16.0))
                    .color(ACCENT),
            );
            ui.add_space(8.0);
        });
    }

    fn menu_ui(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("menu"))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                theme::panel()
                    .inner_margin(egui::Margin::same(28))
                    .show(ui, |ui| {
                        self.title_block(ui);
                        let status = self.lakeside.status();
                        if let Some(a) = &status.account {
                            ui.vertical_centered(|ui| {
                                label(ui, &format!("SIGNED IN AS {}", a.name.to_uppercase()), 14.0);
                                if status.launcher {
                                    label(ui, "Through the Lakeside launcher, which keeps this copy up to date.", 12.0);
                                }
                                ui.add_space(6.0);
                            });
                        }
                        ui.vertical_centered(|ui| {
                            if theme::button(ui, "NEW TOWN").clicked() {
                                self.screen = Screen::NewTown;
                            }
                            if theme::button(ui, "SETTINGS").clicked() {
                                self.screen = Screen::Settings;
                            }
                            if theme::button(ui, "QUIT").clicked() {
                                self.quit = true;
                            }
                        });
                    });
            });
    }

    fn new_town_ui(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("newtown"))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                theme::panel()
                    .inner_margin(egui::Margin::same(24))
                    .show(ui, |ui| {
                        ui.label(heading("NEW TOWN", 32.0));
                        ui.add_space(6.0);
                        label(ui, "TOWN STYLE", 14.0);
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut self.preset,
                                Preset::Grid,
                                RichText::new("GRID TOWN").font(pixel(16.0)),
                            );
                            ui.selectable_value(
                                &mut self.preset,
                                Preset::Winding,
                                RichText::new("WINDING TOWN").font(pixel(16.0)),
                            );
                        });
                        label(
                            ui,
                            match self.preset {
                                Preset::Grid => {
                                    "Prairie railroad town: straight streets, a grid of old roads."
                                }
                                Preset::Winding => {
                                    "River and hills town: crooked streets that follow the land."
                                }
                            },
                            12.0,
                        );
                        ui.add_space(6.0);
                        label(ui, "SEED", 14.0);
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.seed_text)
                                    .desired_width(160.0)
                                    .font(pixel(16.0)),
                            );
                            if ui
                                .button(RichText::new("RANDOM").font(pixel(14.0)).color(INK))
                                .clicked()
                            {
                                self.seed_text = random_seed().to_string();
                            }
                        });
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            if theme::button(ui, "START").clicked() {
                                self.screen = Screen::Generating;
                                self.gen_wait = 0;
                            }
                            if theme::button(ui, "BACK").clicked() {
                                self.screen = Screen::Menu;
                            }
                        });
                    });
            });
    }

    fn settings_ui(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("settings"))
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                theme::panel()
                    .inner_margin(egui::Margin::same(24))
                    .show(ui, |ui| {
                        ui.label(heading("SETTINGS", 32.0));
                        ui.add_space(6.0);
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
                        label(ui, "UI SCALE", 14.0);
                        ui.add(
                            egui::Slider::new(&mut self.settings.ui_scale, 0.8..=1.8).step_by(0.1),
                        );
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
                        label(ui, "F11 TOGGLES FULLSCREEN IN GAME", 12.0);
                        ui.add_space(8.0);
                        self.account_ui(ui);
                        ui.add_space(10.0);
                        if theme::button(ui, "BACK").clicked() {
                            self.screen = Screen::Menu;
                        }
                    });
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
        let g = self.game.as_mut().unwrap();
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
                    INK,
                );
                painter.rect_filled(
                    egui::Rect::from_center_size(*p, vec2(r * 2.0, r * 2.0)),
                    egui::CornerRadius::ZERO,
                    if hot {
                        ACCENT
                    } else {
                        Color32::from_rgb(250, 246, 234)
                    },
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
                painter.circle_stroke(a, rad + 2.0, Stroke::new(5.0, INK));
                painter.circle_stroke(a, rad, Stroke::new(3.0, ACCENT));
            }
        }

        // top bar
        egui::Panel::top("hud").frame(theme::bar()).show(ui, |ui| {
            ui.horizontal(|ui| {
                let light = Color32::from_rgb(250, 246, 234);
                let t = |s: String| RichText::new(s).font(pixel(16.0)).color(light);
                ui.label(
                    RichText::new("URBAN SPRAWL")
                        .font(pixel(16.0))
                        .color(ACCENT),
                );
                ui.add_space(14.0);
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
                            .font(pixel(16.0))
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
                        .font(pixel(16.0))
                        .color(nc),
                    );
                } else {
                    ui.label(t("PLACE THE TOWN HALL".to_string()));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(RichText::new("MENU").font(pixel(14.0)).color(INK))
                        .clicked()
                    {
                        g.menu_open = true;
                    }
                    ui.label(
                        RichText::new(format!("{fps:.0} FPS"))
                            .font(pixel(12.0))
                            .color(Color32::from_rgb(170, 176, 196)),
                    );
                    if g.placed {
                        for i in (0..4).rev() {
                            let on = g.speed == i;
                            let b = egui::Button::new(
                                RichText::new(SPEED_LABELS[i]).font(pixel(14.0)).color(INK),
                            )
                            .fill(if on {
                                ACCENT
                            } else {
                                Color32::from_rgb(250, 246, 234)
                            })
                            .min_size(vec2(38.0, 26.0));
                            if ui.add(b).clicked() {
                                g.speed = i;
                            }
                        }
                    }
                });
            });
        });

        // banner
        let banner = if !g.placed {
            Some("CLICK A CROSSROADS TO PLACE THE TOWN HALL".to_string())
        } else {
            g.banner.as_ref().map(|b| b.0.clone())
        };
        if let Some(b) = banner {
            egui::Area::new(egui::Id::new("banner"))
                .anchor(Align2::CENTER_TOP, vec2(0.0, 56.0))
                .show(ctx, |ui| {
                    theme::panel().show(ui, |ui| {
                        ui.label(heading(&b, 18.0));
                        if !g.placed {
                            label(ui, "THE ORANGE MARKERS ARE OLD CROSSROADS. YOUR MAIN STREET WILL RUN TO THE HIGHWAY.", 12.0);
                        }
                    });
                });
        }

        if g.placed {
            // tax and demand
            egui::Area::new(egui::Id::new("tax"))
                .anchor(Align2::LEFT_BOTTOM, vec2(14.0, -14.0))
                .show(ctx, |ui| {
                    theme::panel().show(ui, |ui| {
                        ui.set_width(300.0);
                        ui.label(heading("PROPERTY TAX", 18.0));
                        let mut pct = g.sim.tax_pct() as f32;
                        let r = ui.add(
                            egui::Slider::new(&mut pct, 0.0..=20.0)
                                .step_by(0.5)
                                .custom_formatter(|v, _| format!("{v:.1}%")),
                        );
                        if r.changed() {
                            g.sim.set_tax_x10((pct * 10.0).round() as u32);
                        }
                        ui.add_space(2.0);
                        let tax = g.sim.tax_x10();
                        let (msg, col) = if tax > ABANDON_ABOVE_X10 {
                            ("TOO HIGH: BUILDINGS EMPTY OUT", BAD)
                        } else if tax >= 100 {
                            ("HIGH: GROWTH IS SLOW", Color32::from_rgb(190, 120, 30))
                        } else if tax <= 40 {
                            ("LOW: FAST GROWTH, THIN TREASURY", GOOD)
                        } else {
                            ("BALANCED", TEXT)
                        };
                        ui.label(RichText::new(msg).font(pixel(12.0)).color(col));
                        ui.add_space(4.0);
                        label(ui, "DEMAND", 14.0);
                        let (rect, _) =
                            ui.allocate_exact_size(vec2(276.0, 20.0), egui::Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(rect, egui::CornerRadius::ZERO, INK);
                        let inner = rect.shrink(3.0);
                        p.rect_filled(
                            inner,
                            egui::CornerRadius::ZERO,
                            Color32::from_rgb(250, 246, 234),
                        );
                        let mut f = inner;
                        f.set_width(inner.width() * g.stats.demand as f32);
                        let dc = if g.stats.demand < 0.3 {
                            BAD
                        } else if g.stats.demand < 0.55 {
                            Color32::from_rgb(220, 170, 50)
                        } else {
                            GOOD
                        };
                        p.rect_filled(f, egui::CornerRadius::ZERO, dc);
                        ui.add_space(2.0);
                        label(
                            ui,
                            &format!(
                                "BUILT {}  BUILDING {}  EMPTY {}",
                                g.stats.built, g.stats.building, g.stats.empty
                            ),
                            12.0,
                        );
                        label(
                            ui,
                            &format!(
                                "TAX IN {}  COSTS {}",
                                money(g.stats.income),
                                money(g.stats.expense)
                            ),
                            12.0,
                        );
                    });
                });
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

        egui::Area::new(egui::Id::new("help"))
            .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -6.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.add(egui::Label::new(
                    RichText::new("WASD PAN   Q E ROTATE   RIGHT DRAG ORBIT   WHEEL ZOOM   0 1 2 3 SPEED   SPACE PAUSE")
                        .font(pixel(10.0))
                        .color(Color32::from_rgb(40, 44, 56)),
                ).extend());
            });

        if g.menu_open {
            egui::Area::new(egui::Id::new("pause"))
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .show(ctx, |ui| {
                    theme::panel()
                        .inner_margin(egui::Margin::same(24))
                        .show(ui, |ui| {
                            ui.label(heading("PAUSED", 32.0));
                            label(
                                ui,
                                &format!(
                                    "{} TOWN, SEED {}",
                                    match g.preset {
                                        Preset::Grid => "GRID",
                                        Preset::Winding => "WINDING",
                                    },
                                    g.seed
                                ),
                                12.0,
                            );
                            if theme::button(ui, "RESUME").clicked() {
                                g.menu_open = false;
                            }
                            if theme::button(ui, "SETTINGS").clicked() {
                                open_settings = true;
                            }
                            if theme::button(ui, "QUIT TO MENU").clicked() {
                                to_menu = true;
                            }
                            if theme::button(ui, "QUIT GAME").clicked() {
                                quit = true;
                            }
                        });
                });
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
    fn place(&mut self, site: Pt) {
        let mut sim = Sim::new(&self.town, site, self.seed);
        let mut live = LiveScene::new(self.town.clone(), &sim, true);
        live.sync(&mut sim, 1.0e9);
        self.sim = sim;
        self.live = live;
        self.placed = true;
        self.speed = 2;
        self.sel = Some(Sel::Hall);
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
            demand: self.sim.demand(),
        };
    }
}

fn g_menu_close(st: &mut State) {
    if let Some(g) = st.game.as_mut() {
        g.menu_open = false;
    }
}
