//! Repeatable pictures of junctions and of the land beyond the map, drawn from the same
//! live scene the game shows. Run with a graphics adapter:
//!
//! `URBAN_SPRAWL_CAPTURE_DIR=out cargo test -p us-app --release capture -- --ignored --nocapture`
//!
//! Set `URBAN_SPRAWL_CAPTURE_SAVE` to also draw an existing save.

use crate::world::LiveScene;
use glam::Vec3;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use us_gen::{Preset, Pt, generate};
use us_render::{Calendar, Camera, Renderer, Style, WeatherLook};
use us_sim::{Sim, hall_sites};

const W: u32 = 720;
const H: u32 = 480;

fn out_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("URBAN_SPRAWL_CAPTURE_DIR")?);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn grown(preset: Preset, seed: u64, days: i32) -> Sim {
    let town = Arc::new(generate(preset, seed));
    let hall = hall_sites(&town)[0];
    let mut sim = Sim::new_with(&town, hall, seed, 0);
    let mut round = 0;
    for d in 0..days {
        sim.step_day();
        if d % 30 == 5 {
            us_sim::tend(&mut sim, &mut round);
        }
    }
    sim
}

/// Junctions worth inspecting: nearest to the hall, those whose streets belong to different
/// neighbourhoods, and pairs whose mouths sit close together along one street.
fn picks(live: &LiveScene, sim: &Sim) -> Vec<(String, Pt)> {
    let points = live.scene.junction_points();
    let runs = sim.road_runs();
    let groups = |p: Pt| {
        let mut g: Vec<Option<usize>> = runs
            .iter()
            .enumerate()
            .filter(|(i, r)| {
                sim.road_visible(*i)
                    && r.points.windows(2).any(|s| {
                        let d = s[1] - s[0];
                        let len2 = d.dot(d).max(1e-9);
                        let t = ((p - s[0]).dot(d) / len2).clamp(0.0, 1.0);
                        p.dist(s[0] + d * t) < 2.0
                    })
            })
            .map(|(_, r)| r.addition)
            .collect();
        g.sort();
        g.dedup();
        g
    };
    let mut out = Vec::new();
    let mut by_hall: Vec<Pt> = points.clone();
    by_hall.sort_by(|a, b| a.dist(sim.hall).total_cmp(&b.dist(sim.hall)));
    for (k, p) in by_hall.iter().take(3).enumerate() {
        out.push((format!("hall-{k}"), *p));
    }
    let mut split = 0;
    for p in &by_hall {
        if split < 3 && groups(*p).len() > 1 {
            out.push((format!("split-{split}"), *p));
            split += 1;
        }
    }
    let mut stagger = Vec::new();
    for (i, a) in points.iter().enumerate() {
        for b in &points[i + 1..] {
            let d = a.dist(*b);
            if (8.0..45.0).contains(&d) {
                stagger.push((d, *a, *b));
            }
        }
    }
    stagger.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (k, (_, a, b)) in stagger.iter().take(4).enumerate() {
        out.push((format!("stagger-{k}"), a.lerp(*b, 0.5)));
    }
    out
}

fn shoot(
    renderer: &mut Renderer,
    live: &mut LiveScene,
    dir: &Path,
    name: &str,
    at: Pt,
    width_m: f32,
    tilt: f32,
) {
    let z = live.ground_at(at.x as f32, at.y as f32);
    let camera = Camera::framing(
        Vec3::new(at.x as f32, at.y as f32, z),
        width_m,
        W as f32 / H as f32,
        20.0,
        tilt,
    );
    let style = Style::default();
    for (weather, snow, month) in [("clear", 0.0, 6), ("snow", 1.0, 1)] {
        live.scene
            .set_calendar(Some(Calendar::from_date(month, 15, 11.0)));
        live.scene.set_weather(WeatherLook {
            active: true,
            snow,
            ..WeatherLook::default()
        });
        let f = renderer.render_to_rgba(&live.scene, &camera, &style, W, H);
        let path = dir.join(format!("{name}-{weather}.png"));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write capture");
    }
}

fn capture_town(renderer: &mut Renderer, dir: &Path, label: &str, mut sim: Sim) {
    let town = Arc::new(sim.town().clone());
    let mut live = LiveScene::new(town, &sim, true);
    live.sync(&mut sim, 1.0e9);
    live.scene.set_traffic_hour(8.0);
    live.scene.time = 30.0;
    let picks = picks(&live, &sim);
    let mut index = String::new();
    for (name, p) in &picks {
        index.push_str(&format!("{label}-{name} {:.1} {:.1}\n", p.x, p.y));
        for (view, width) in [("", 90.0), ("-close", 40.0)] {
            shoot(
                renderer,
                &mut live,
                dir,
                &format!("{label}-{name}{view}"),
                *p,
                width,
                88.0,
            );
        }
    }
    std::fs::write(dir.join(format!("{label}-junctions.txt")), index).unwrap();
    // URBAN_SPRAWL_CAPTURE_SURVEY=n: the n junctions nearest the hall, numbered by distance
    let survey: usize = std::env::var("URBAN_SPRAWL_CAPTURE_SURVEY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut near = live.scene.junction_points();
    near.sort_by(|a, b| a.dist(sim.hall).total_cmp(&b.dist(sim.hall)));
    let mut list = String::new();
    for (k, p) in near.iter().take(survey).enumerate() {
        list.push_str(&format!("{k:03} {:.1} {:.1}\n", p.x, p.y));
        shoot_one(
            renderer,
            &mut live,
            dir,
            &format!("{label}-survey-{k:03}"),
            *p,
        );
    }
    // URBAN_SPRAWL_CAPTURE_ZOOM=3,17: close views of chosen survey junctions
    if let Ok(list) = std::env::var("URBAN_SPRAWL_CAPTURE_ZOOM") {
        for k in list
            .split(',')
            .filter_map(|v| v.trim().parse::<usize>().ok())
        {
            if let Some(p) = near.get(k) {
                shoot(
                    renderer,
                    &mut live,
                    dir,
                    &format!("{label}-zoom-{k:03}"),
                    *p,
                    50.0,
                    88.0,
                );
            }
        }
    }
    if survey > 0 {
        std::fs::write(dir.join(format!("{label}-survey.txt")), list).unwrap();
    }
}

fn shoot_one(renderer: &mut Renderer, live: &mut LiveScene, dir: &Path, name: &str, at: Pt) {
    let z = live.ground_at(at.x as f32, at.y as f32);
    let camera = Camera::framing(Vec3::new(at.x as f32, at.y as f32, z), 70.0, 1.5, 0.0, 89.0);
    live.scene
        .set_calendar(Some(Calendar::from_date(6, 15, 12.0)));
    live.scene.set_weather(WeatherLook {
        active: true,
        ..WeatherLook::default()
    });
    let f = renderer.render_to_rgba(&live.scene, &camera, &Style::default(), 360, 240);
    image::save_buffer(
        dir.join(format!("{name}.png")),
        &f.rgba,
        f.width,
        f.height,
        image::ColorType::Rgba8,
    )
    .expect("write capture");
}

#[test]
#[ignore = "writes pictures; needs a graphics adapter and URBAN_SPRAWL_CAPTURE_DIR"]
fn capture_junctions() {
    let Some(dir) = out_dir() else { return };
    let Ok(mut renderer) = Renderer::new() else {
        eprintln!("no graphics adapter");
        return;
    };
    if let Some(save) = std::env::var_os("URBAN_SPRAWL_CAPTURE_SAVE") {
        let sim = crate::saves::load(Path::new(&save)).expect("save loads");
        capture_town(&mut renderer, &dir, "save", sim);
    }
    if let Some((mut sim, target)) = player_stagger() {
        let town = Arc::new(sim.town().clone());
        let mut live = LiveScene::new(town, &sim, true);
        live.sync(&mut sim, 1.0e9);
        let points = live.scene.junction_points();
        let mut near: Vec<Pt> = points;
        near.sort_by(|a, b| a.dist(target).total_cmp(&b.dist(target)));
        eprintln!(
            "player stagger at {target:?}; nearest junctions {:?}",
            &near[..near.len().min(3)]
        );
        let mid = target;
        shoot(
            &mut renderer,
            &mut live,
            &dir,
            "player-stagger",
            mid,
            60.0,
            88.0,
        );
        shoot(
            &mut renderer,
            &mut live,
            &dir,
            "player-stagger-tilt",
            mid,
            70.0,
            45.0,
        );
    }
    for (preset, seed) in [(Preset::Winding, 7), (Preset::Grid, 5)] {
        let sim = grown(preset, seed, 2400);
        capture_town(&mut renderer, &dir, preset.name(), sim);
    }
}

/// A young town where the player has opened two side streets 18 m apart on opposite sides
/// of a main road, the tightest stagger the road tools allow without a crossroads.
fn player_stagger() -> Option<(Sim, Pt)> {
    let town = Arc::new(generate(Preset::Grid, 3));
    let hall = hall_sites(&town)[0];
    let rules = us_sim::Rules::sandbox_unlimited();
    let mut sim = Sim::new_with_rules(&town, hall, 3, 0, rules);
    for _ in 0..200 {
        sim.step_day();
    }
    let mut spots: Vec<(Pt, Pt, f64)> = Vec::new();
    for r in sim.road_runs() {
        if !matches!(
            r.class,
            us_gen::RoadClass::Arterial | us_gen::RoadClass::Collector
        ) {
            continue;
        }
        for seg in r.points.windows(2) {
            let length = seg[0].dist(seg[1]);
            if length > 160.0 {
                let mid = seg[0].lerp(seg[1], 0.5);
                spots.push((
                    mid,
                    (seg[1] - seg[0]) * (1.0 / length),
                    r.class.half_width(),
                ));
            }
        }
    }
    spots.sort_by(|a, b| a.0.dist(hall).total_cmp(&b.0.dist(hall)));
    for (mid, dir, _) in spots {
        let across = us_gen::pt(-dir.y, dir.x);
        let a = [mid, mid + across * 90.0];
        let b0 = mid + dir * 18.0;
        let b = [b0, b0 - across * 90.0];
        if sim.check_road(us_gen::RoadClass::Collector, &a).is_ok()
            && sim.check_road(us_gen::RoadClass::Collector, &b).is_ok()
            && sim.build_road(us_gen::RoadClass::Collector, &a).is_ok()
            && sim.build_road(us_gen::RoadClass::Collector, &b).is_ok()
        {
            for _ in 0..100 {
                sim.step_day();
            }
            let n = sim.road_runs().len();
            eprintln!(
                "built {:?} and {:?}; open {} {}",
                sim.road_runs()[n - 2].points,
                sim.road_runs()[n - 1].points,
                sim.road_open(n - 2),
                sim.road_open(n - 1)
            );
            return Some((sim, mid + dir * 9.0));
        }
    }
    None
}

/// Low and zoomed-out views across the edge of the playable map, by day and at dusk.
#[test]
#[ignore = "writes pictures; needs a graphics adapter and URBAN_SPRAWL_CAPTURE_DIR"]
fn capture_horizon() {
    let Some(dir) = out_dir() else { return };
    let Ok(mut renderer) = Renderer::new() else {
        eprintln!("no graphics adapter");
        return;
    };
    let style = Style::default();
    for preset in [Preset::Winding, Preset::Coast, Preset::Mesa] {
        let mut sim = grown(preset, 3, 900);
        let town = Arc::new(sim.town().clone());
        let size = town.size as f32;
        let mut live = LiveScene::new(town, &sim, true);
        live.sync(&mut sim, 1.0e9);
        live.scene.set_weather(WeatherLook {
            active: true,
            ..WeatherLook::default()
        });
        let hall = Vec3::new(sim.hall.x as f32, sim.hall.y as f32, 0.0);
        // a low look toward each map edge from a third of the way in, then the whole map
        let mut views: Vec<(String, Vec3, f32, f32, f32)> = Vec::new();
        for (name, yaw, x, y) in [
            ("north", 0.0, 0.5, 0.72),
            ("east", 90.0, 0.72, 0.5),
            ("south", 180.0, 0.5, 0.28),
            ("west", 270.0, 0.28, 0.5),
        ] {
            views.push((
                format!("low-{name}"),
                Vec3::new(size * x, size * y, 0.0),
                900.0,
                yaw,
                9.0,
            ));
        }
        views.push((
            "overview".into(),
            Vec3::new(size * 0.5, size * 0.5, 0.0),
            size * 1.5,
            30.0,
            32.0,
        ));
        views.push((
            "far".into(),
            Vec3::new(size * 0.5, size * 0.5, 0.0),
            size * 3.2,
            30.0,
            22.0,
        ));
        views.push(("street".into(), hall, 260.0, 40.0, 6.0));
        // where the shore crosses the north and south map edges, looking along the coast
        for (name, y, yaw) in [("shore-north", 0.93, 0.0), ("shore-south", 0.07, 180.0)] {
            let y = size * y;
            if let Some(x) = (0..400).map(|k| size * k as f32 / 400.0).find(|&x| {
                live.land()
                    .water_at(us_gen::pt(x as f64, y as f64))
                    .is_some()
            }) {
                views.push((name.into(), Vec3::new(x - 150.0, y, 0.0), 1100.0, yaw, 10.0));
            }
        }
        for (name, mut target, width, yaw, tilt) in views {
            target.z = live.ground_at(target.x, target.y);
            let camera = Camera::framing(target, width, 16.0 / 9.0, yaw, tilt);
            for (light, hour, overcast) in [
                ("day", 12.0, 0.0),
                ("dusk", 19.4, 0.0),
                ("cloud", 12.0, 0.85),
            ] {
                live.scene
                    .set_calendar(Some(Calendar::from_date(7, 15, hour)));
                live.scene.set_weather(WeatherLook {
                    active: true,
                    overcast,
                    ..WeatherLook::default()
                });
                let f = renderer.render_to_rgba(&live.scene, &camera, &style, 960, 540);
                let path = dir.join(format!("{}-{name}-{light}.png", preset.name()));
                image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
                    .expect("write capture");
            }
        }
    }
}

/// Reopen every save listed in `URBAN_SPRAWL_CAPTURE_SAVES` (separated by the platform's path
/// separator) and build its live scene, as the Load screen does.
#[test]
#[ignore = "reads saves named by URBAN_SPRAWL_CAPTURE_SAVES"]
fn reopen_saves() {
    let Some(list) = std::env::var_os("URBAN_SPRAWL_CAPTURE_SAVES") else {
        return;
    };
    for path in std::env::split_paths(&list).filter(|p| !p.as_os_str().is_empty()) {
        let mut sim = crate::saves::load(&path)
            .unwrap_or_else(|e| panic!("{} does not load: {e}", path.display()));
        let town = Arc::new(sim.town().clone());
        let mut live = LiveScene::new(town, &sim, true);
        live.sync(&mut sim, 1.0e9);
        eprintln!(
            "{}: {} day {} pop {} chunks {}",
            path.display(),
            sim.town().preset.name(),
            sim.day,
            sim.population(),
            live.scene.chunks.len()
        );
    }
}

/// CPU cost of a frame at 8x on an existing save, without drawing: the simulation days due
/// at 60 frames a second, the live scene sync, and a health coverage overlay refreshed twice a
/// second as the map view does. Prints percentiles per part.
#[test]
#[ignore = "reads URBAN_SPRAWL_CAPTURE_SAVE; a timing probe, not a check"]
fn frame_cost_at_8x() {
    use std::time::Instant;
    let Some(save) = std::env::var_os("URBAN_SPRAWL_CAPTURE_SAVE") else {
        return;
    };
    let overlay: bool = std::env::var("URBAN_SPRAWL_CAPTURE_OVERLAY").is_ok_and(|v| v == "1");
    let mut sim = crate::saves::load(Path::new(&save)).expect("save loads");
    let town = Arc::new(sim.town().clone());
    let mut live = LiveScene::new(town, &sim, true);
    live.sync(&mut sim, 1.0e9);
    let (dt, days_per_second) = (1.0f64 / 60.0, 8.0 / 1.3);
    let mut acc = 0.0f64;
    let (mut step_ms, mut sync_ms, mut over_ms, mut frame_ms) = (vec![], vec![], vec![], vec![]);
    let mut ov_age = 0.0;
    for _ in 0..(60 * 40) {
        let f0 = Instant::now();
        acc = (acc + dt * days_per_second).min(16.0);
        let t = Instant::now();
        let mut steps = 0;
        while acc >= 1.0 && (steps == 0 || t.elapsed().as_millis() < 5) {
            acc -= 1.0;
            sim.step_day();
            steps += 1;
        }
        step_ms.push(t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now();
        live.sync(&mut sim, 5.0);
        sync_ms.push(t.elapsed().as_secs_f64() * 1e3);
        let t = Instant::now();
        ov_age += dt;
        if overlay && ov_age > 0.5 {
            ov_age = 0.0;
            let mut lots = sim.lot_values(us_sim::MapKind::Coverage(us_sim::Service::Health));
            let structures = sim
                .lots()
                .iter()
                .zip(&mut lots)
                .map(|(l, v)| {
                    if matches!(
                        l.phase,
                        us_sim::Phase::Hidden | us_sim::Phase::Cleared { .. }
                    ) {
                        *v = f32::NAN;
                    }
                    matches!(
                        l.phase,
                        us_sim::Phase::Built { .. } | us_sim::Phase::Empty { .. }
                    )
                })
                .collect();
            live.ensure_geometry(&sim);
            live.scene.set_overlay(Some(us_render::Overlay {
                kind: us_render::OverlayKind::ServiceCoverage,
                lots,
                structures,
                roads: Vec::new(),
            }));
        }
        over_ms.push(t.elapsed().as_secs_f64() * 1e3);
        live.scene.time += dt as f32;
        let _ = live.scene.instances();
        frame_ms.push(f0.elapsed().as_secs_f64() * 1e3);
    }
    let report = |name: &str, v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        let p = |q: f64| v[((v.len() - 1) as f64 * q) as usize];
        eprintln!(
            "{name:>8}: p50 {:7.2} p95 {:7.2} p99 {:7.2} max {:7.2} ms",
            p(0.5),
            p(0.95),
            p(0.99),
            v[v.len() - 1]
        );
    };
    eprintln!("day {} pop {} overlay {overlay}", sim.day, sim.population());
    report("steps", &mut step_ms);
    report("sync", &mut sync_ms);
    report("overlay", &mut over_ms);
    report("frame", &mut frame_ms);
}
