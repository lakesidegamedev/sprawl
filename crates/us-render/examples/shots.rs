//! `cargo run --release -p us-render --example shots -- [out_dir]`
//!
//! Generates grid seed 1 and winding seed 2 and renders the standard views of each (whole town,
//! suburb, downtown), then the preview 3 views of grid seed 1: every map view, every building
//! level, night and the seasons, traffic, roadworks, building sites and burned lots.

use glam::Vec3;
use std::path::PathBuf;
use std::time::Instant;
use us_gen::{LotKind, Preset, RoadClass, Town, generate};
use us_render::{
    Calendar, Camera, Frame, LotState, Overlay, OverlayKind, Renderer, Rng, Scene, Style,
    build_lot_level, use_category,
};

const W: u32 = 1600;
const H: u32 = 1000;

/// The middle of the thickest cluster of houses about a kilometre from the hall.
fn suburb_target(t: &Town) -> Vec3 {
    let houses: Vec<_> = t.lots.iter().filter(|l| l.kind == LotKind::House).collect();
    let mut best: Option<(f64, usize)> = None;
    for (i, h) in houses.iter().enumerate() {
        let d = h.centre.dist(t.hall);
        if !(800.0..1500.0).contains(&d) {
            continue;
        }
        let near = houses
            .iter()
            .filter(|o| o.centre.dist(h.centre) < 70.0)
            .count();
        let score = near as f64 * 10.0 - (d - 1100.0).abs() * 0.02;
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, i));
        }
    }
    let h = houses[best.map_or(0, |b| b.1)];
    Vec3::new(h.centre.x as f32, h.centre.y as f32, 0.0)
}

struct Out {
    dir: PathBuf,
    aspect: f32,
}

impl Out {
    fn style(&self, shadow: u32, ink: u32, ss: u32) -> Style {
        Style {
            shadow_size: shadow,
            ink_width: ink,
            supersample: ss,
            ..Style::default()
        }
    }

    fn save(&self, name: &str, f: &Frame) {
        let path = self.dir.join(format!("{name}.png"));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!(
            "  {name}: {:.2}s total, upload {:.2}s, gpu {:.2}s, {}/{} chunks, {} tris",
            f.stats.total.as_secs_f32(),
            f.stats.upload.as_secs_f32(),
            f.stats.gpu_wait.as_secs_f32(),
            f.stats.chunks_drawn,
            f.stats.chunks_total,
            f.stats.triangles,
        );
    }
}

fn standard(r: &mut Renderer, out: &Out) {
    for (preset, seed) in Preset::ALL.map(|p| (p, 1u64)) {
        let t0 = Instant::now();
        let town = generate(preset, seed);
        let t1 = Instant::now();
        let scene = Scene::from_town(&town);
        eprintln!(
            "{} seed {seed}: generate {:.2}s, build scene {:.2}s, {} lots, {} triangles",
            preset.name(),
            (t1 - t0).as_secs_f32(),
            t1.elapsed().as_secs_f32(),
            town.lots.len(),
            scene.triangles()
        );
        let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
        let views = [
            (
                "town",
                Camera::framing(
                    hall + Vec3::new(0.0, -150.0, 0.0),
                    2000.0,
                    out.aspect,
                    30.0,
                    48.0,
                ),
                (8192, 1, 3),
            ),
            (
                "suburb",
                Camera::framing(suburb_target(&town), 250.0, out.aspect, 45.0, 40.0),
                (4096, 2, 2),
            ),
            (
                "downtown",
                Camera::framing(hall, 250.0, out.aspect, 45.0, 40.0),
                (4096, 2, 2),
            ),
        ];
        for (name, cam, (shadow, ink, ss)) in views {
            let f = r.render_to_rgba(&scene, &cam, &out.style(shadow, ink, ss), W, H);
            out.save(&format!("{}-seed{seed}-{name}", preset.name()), &f);
        }
    }
}

fn dist_hall(t: &Town, x: f64, y: f64) -> f64 {
    ((x - t.hall.x).powi(2) + (y - t.hall.y).powi(2)).sqrt()
}

fn views(r: &mut Renderer, out: &Out) {
    let town = generate(Preset::Grid, 1);
    let mut scene = Scene::from_town(&town);
    let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
    let wide = Camera::framing(
        hall + Vec3::new(0.0, -80.0, 0.0),
        1300.0,
        out.aspect,
        30.0,
        52.0,
    );
    let down = Camera::framing(hall, 260.0, out.aspect, 45.0, 40.0);
    let sub = Camera::framing(suburb_target(&town), 260.0, out.aspect, 45.0, 40.0);
    let hi = out.style(4096, 2, 2);
    let big = out.style(8192, 1, 2);

    // ---- map views ----
    let lot_vals = |f: &dyn Fn(usize, f64) -> f32| -> Vec<f32> {
        town.lots
            .iter()
            .enumerate()
            .map(|(i, l)| f(i, dist_hall(&town, l.centre.x, l.centre.y)))
            .collect()
    };
    let noise = |i: usize| ((i as u64 * 2654435761) % 1000) as f32 / 1000.0;
    let road_vals: Vec<f32> = town
        .roads
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mid = r.points[r.points.len() / 2];
            let d = dist_hall(&town, mid.x, mid.y) as f32;
            let base = match r.class {
                RoadClass::Highway => 0.8,
                RoadClass::Arterial => 0.6,
                RoadClass::Collector => 0.4,
                _ => 0.15,
            };
            (base + (1.0 - d / 1800.0).max(0.0) * 0.4 + noise(i) * 0.1).clamp(0.0, 1.0)
        })
        .collect();
    let overlays: [(&str, Overlay); 6] = [
        (
            "landvalue",
            Overlay {
                kind: OverlayKind::LandValue,
                lots: lot_vals(&|i, d| {
                    ((1.0 - d as f32 / 1700.0) * 0.8 + noise(i) * 0.25).clamp(0.0, 1.0)
                }),
                roads: vec![],
            },
        ),
        (
            "level",
            Overlay {
                kind: OverlayKind::BuildingLevel,
                lots: lot_vals(&|i, d| {
                    let lv = (5.0 - d as f32 / 380.0 + noise(i)).clamp(1.0, 5.0).floor();
                    lv / 5.0 - 0.01
                }),
                roads: vec![],
            },
        ),
        (
            "use",
            Overlay {
                kind: OverlayKind::Use,
                lots: town
                    .lots
                    .iter()
                    .map(|l| use_category(l.kind) as f32)
                    .collect(),
                roads: vec![],
            },
        ),
        (
            "coverage",
            Overlay {
                kind: OverlayKind::ServiceCoverage,
                lots: lot_vals(&|i, d| {
                    ((1.0 - d as f32 / 1400.0) + (noise(i) - 0.5) * 0.3).clamp(0.0, 0.999)
                }),
                roads: vec![],
            },
        ),
        (
            "demand",
            Overlay {
                kind: OverlayKind::Demand,
                lots: lot_vals(&|i, d| {
                    (0.5 + 0.45 * libm_sin(d as f32 / 260.0) + (noise(i) - 0.5) * 0.2)
                        .clamp(0.0, 0.999)
                }),
                roads: vec![],
            },
        ),
        (
            "traffic",
            Overlay {
                kind: OverlayKind::Traffic,
                lots: vec![],
                roads: road_vals.clone(),
            },
        ),
    ];
    for (name, ov) in overlays {
        let t = Instant::now();
        scene.set_overlay(Some(ov));
        let build = t.elapsed();
        let f = r.render_to_rgba(&scene, &wide, &big, W, H);
        eprintln!("  overlay {name} build {:.2}s", build.as_secs_f32());
        out.save(&format!("view-{name}-town"), &f);
        let f = r.render_to_rgba(&scene, &down, &hi, W, H);
        out.save(&format!("view-{name}-downtown"), &f);
    }
    scene.set_overlay(None);

    // ---- traffic on the roads ----
    scene.set_road_volumes(&road_vals);
    eprintln!("  cars: {}", scene.car_count());
    for (name, t) in [("t0", 0.0f32), ("t4", 4.0)] {
        scene.set_time(t);
        let f = r.render_to_rgba(&scene, &down, &hi, W, H);
        out.save(&format!("cars-downtown-{name}"), &f);
    }
    scene.set_time(2.0);
    let f = r.render_to_rgba(&scene, &wide, &big, W, H);
    out.save("cars-town", &f);
    scene.set_road_volumes(&[]);
    scene.set_time(0.0);

    // ---- building levels: every lot at one level ----
    for level in 1..=5u8 {
        let t = Instant::now();
        let all: Vec<_> = (0..town.lots.len())
            .map(|i| (i, Some(level), LotState::Normal))
            .collect();
        scene.set_lots(&all);
        let rebuild = t.elapsed();
        eprintln!("  level {level}: rebuild all {:.2}s", rebuild.as_secs_f32());
        let f = r.render_to_rgba(&scene, &down, &hi, W, H);
        out.save(&format!("level{level}-downtown"), &f);
        let f = r.render_to_rgba(&scene, &sub, &hi, W, H);
        out.save(&format!("level{level}-suburb"), &f);
    }
    // a believable spread: level falls off with distance from the hall
    let spread: Vec<_> = town
        .lots
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let d = dist_hall(&town, l.centre.x, l.centre.y) as f32;
            let lv = (5.0 - d / 380.0 + noise(i)).clamp(1.0, 5.0) as u8;
            (i, Some(lv), LotState::Normal)
        })
        .collect();
    scene.set_lots(&spread);
    let f = r.render_to_rgba(&scene, &down, &hi, W, H);
    out.save("levels-mixed-downtown", &f);

    // ---- time of day and seasons ----
    let times = [
        ("dawn", Calendar::from_date(6, 21, 6.0)),
        ("day", Calendar::from_date(6, 21, 12.0)),
        ("dusk", Calendar::from_date(6, 21, 18.5)),
        ("night", Calendar::from_date(6, 21, 22.0)),
        ("autumn", Calendar::from_date(10, 20, 12.0)),
        ("winter", Calendar::from_date(1, 15, 12.0)),
        ("winter-night", Calendar::from_date(1, 15, 21.0)),
        ("spring", Calendar::from_date(4, 20, 12.0)),
    ];
    scene.set_road_volumes(&road_vals);
    scene.set_time(1.0);
    for (name, c) in times {
        scene.set_calendar(Some(c));
        let f = r.render_to_rgba(&scene, &down, &hi, W, H);
        out.save(&format!("time-{name}-downtown"), &f);
        let f = r.render_to_rgba(&scene, &sub, &hi, W, H);
        out.save(&format!("time-{name}-suburb"), &f);
        if matches!(name, "night" | "winter" | "autumn") {
            let f = r.render_to_rgba(&scene, &wide, &big, W, H);
            out.save(&format!("time-{name}-town"), &f);
        }
    }
    scene.set_calendar(None);
    scene.set_road_volumes(&[]);

    // ---- roadworks and a road upgrade ----
    let near_hall = |class: RoadClass| -> Option<usize> {
        let mut best: Option<(f64, usize)> = None;
        for (i, rd) in town.roads.iter().enumerate() {
            if rd.class != class {
                continue;
            }
            let m = rd.points[rd.points.len() / 2];
            let d = dist_hall(&town, m.x, m.y);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, i));
            }
        }
        best.map(|b| b.1)
    };
    if let Some(i) = near_hall(RoadClass::Collector) {
        let m = town.roads[i].points[town.roads[i].points.len() / 2];
        let cam = Camera::framing(
            Vec3::new(m.x as f32, m.y as f32, 0.0),
            220.0,
            out.aspect,
            40.0,
            45.0,
        );
        let f = r.render_to_rgba(&scene, &cam, &hi, W, H);
        out.save("road-before", &f);
        let t = Instant::now();
        scene.set_roads(&[(i, RoadClass::Collector, Some(0.3))]);
        eprintln!("  roadworks re-mesh {:.3}s", t.elapsed().as_secs_f32());
        let f = r.render_to_rgba(&scene, &cam, &hi, W, H);
        out.save("road-works", &f);
        scene.set_roads(&[(i, RoadClass::Collector, Some(0.8))]);
        let f = r.render_to_rgba(&scene, &cam, &hi, W, H);
        out.save("road-works-late", &f);
        scene.set_roads(&[(i, RoadClass::Arterial, None)]);
        let f = r.render_to_rgba(&scene, &cam, &hi, W, H);
        out.save("road-after-arterial", &f);
    }

    // ---- building site, burned and rebuilt ----
    let mut near: Vec<usize> = (0..town.lots.len())
        .filter(|&i| {
            matches!(
                town.lots[i].kind,
                LotKind::House | LotKind::Shop | LotKind::Apartments | LotKind::Office
            )
        })
        .collect();
    near.sort_by(|&a, &b| {
        dist_hall(&town, town.lots[a].centre.x, town.lots[a].centre.y).total_cmp(&dist_hall(
            &town,
            town.lots[b].centre.x,
            town.lots[b].centre.y,
        ))
    });
    let pick: Vec<usize> = near.iter().copied().take(30).collect();
    let mut changes = Vec::new();
    for (k, &i) in pick.iter().enumerate() {
        let st = match k % 6 {
            0 => LotState::Building(0.15),
            1 => LotState::Building(0.55),
            2 => LotState::Building(0.9),
            3 => LotState::Burned,
            _ => LotState::Normal,
        };
        changes.push((i, Some(3 + (k % 3) as u8), st));
    }
    scene.set_lots(&changes);
    let f = r.render_to_rgba(&scene, &down, &hi, W, H);
    out.save("sites-downtown", &f);
    let f = r.render_to_rgba(
        &scene,
        &Camera::framing(hall, 120.0, out.aspect, 30.0, 38.0),
        &hi,
        W,
        H,
    );
    out.save("sites-close", &f);

    // ---- smoke ----
    let mut scene2 = Scene::from_town(&town);
    let works: Vec<usize> = (0..town.lots.len())
        .filter(|&i| town.lots[i].kind == LotKind::Workshop)
        .collect();
    if let Some(&w) = works.first() {
        let ch: Vec<_> = works
            .iter()
            .map(|&i| (i, Some(4u8), LotState::Normal))
            .collect();
        scene2.set_lots(&ch);
        scene2.set_time(2.5);
        let c = town.lots[w].centre;
        let cam = Camera::framing(
            Vec3::new(c.x as f32, c.y as f32, 8.0),
            90.0,
            out.aspect,
            35.0,
            30.0,
        );
        let f = r.render_to_rgba(&scene2, &cam, &hi, W, H);
        out.save("smoke-factory", &f);
    }
    // people
    scene2.set_time(7.0);
    eprintln!("  people: {}", scene2.instances().people.len());
    let f = r.render_to_rgba(
        &scene2,
        &Camera::framing(hall, 90.0, out.aspect, 20.0, 35.0),
        &hi,
        W,
        H,
    );
    out.save("people-downtown", &f);

    // frame time of a typical moving frame at the game window size
    scene.set_road_volumes(&road_vals);
    let mut total = 0.0;
    let n = 5;
    for k in 0..n {
        scene.set_time(k as f32 * 0.5);
        let f = r.render_to_rgba(&scene, &down, &out.style(2048, 2, 1), 1280, 720);
        total += f.stats.total.as_secs_f32();
    }
    eprintln!(
        "  moving frame 1280x720, {} cars: {:.3}s avg",
        scene.car_count(),
        total / n as f32
    );
}

fn libm_sin(x: f32) -> f32 {
    x.sin()
}

/// Every lot kind at every level on one sheet, five levels left to right.
fn sheets(r: &mut Renderer, out: &Out) {
    use us_gen::LotKind::*;
    let groups: [(&str, &[LotKind]); 3] = [
        ("homes-shops", &[House, Row, Shop, Apartments, Office, Gas]),
        (
            "civic-industry",
            &[Workshop, Church, School, Fire, Water, Warehouse],
        ),
        (
            "open-retail",
            &[Strip, BigBox, FastFood, Farm, Parking, Park, Sports],
        ),
    ];
    for (name, kinds) in groups {
        let cell = 52.0f32;
        let rows = kinds.len() as f32;
        let size = cell * 7.0;
        let mut scene = Scene::dynamic(
            size.max(rows * cell + cell),
            us_render::Mesh::default(),
            Vec3::ZERO,
        );
        let mut mesh = us_render::Mesh::default();
        let mut id = 1u32;
        for (ri, &k) in kinds.iter().enumerate() {
            for level in 1..=5u8 {
                let mut rng = Rng::new(11, ri as u64 * 9 + level as u64);
                let o = build_lot_level(k, level, &mut rng);
                o.emit(
                    &mut mesh,
                    &mut id,
                    cell * (level as f32),
                    cell * (ri as f32 + 1.0),
                    0.0,
                    1.0,
                );
            }
        }
        // grass under the sheet
        let mut g = us_render::ground::Ground::default();
        g.rect(
            -50.0,
            -50.0,
            cell * 7.0,
            cell * (rows + 2.0),
            us_render::ground::layer::BASE,
            us_render::GRASS,
        );
        scene.ground = g.mesh;
        scene.chunks.push(us_render::Chunk {
            rev: 0,
            mesh,
            lo: Vec3::new(-50.0, -50.0, 0.0),
            hi: Vec3::new(cell * 7.0, cell * (rows + 2.0), 90.0),
        });
        let target = Vec3::new(cell * 3.0, cell * (rows + 1.0) / 2.0 - cell * 0.1, 0.0);
        let cam = Camera::framing(target, cell * 8.0, out.aspect, 0.0, 52.0);
        let f = r.render_to_rgba(&scene, &cam, &out.style(8192, 1, 2), W, H);
        out.save(&format!("kit-levels-{name}"), &f);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out_dir: PathBuf = args
        .first()
        .map_or("target/shots".into(), |s| s.clone())
        .into();
    let only = args.get(1).cloned();
    std::fs::create_dir_all(&out_dir).expect("create output dir");
    let mut r = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    eprintln!("adapter: {:?}", r.adapter());
    let out = Out {
        dir: out_dir,
        aspect: W as f32 / H as f32,
    };
    let want = |n: &str| only.as_deref().is_none_or(|o| o == n);
    if want("standard") {
        standard(&mut r, &out);
    }
    if want("sheets") {
        sheets(&mut r, &out);
    }
    if want("views") {
        views(&mut r, &out);
    }
}
