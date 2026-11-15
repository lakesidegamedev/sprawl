//! `cargo run --release -p us-render --example skyline -- <out_dir> [preset] [seed]`
//!
//! A grown-up town: every lot drawn at the level its density lets it reach, to show where the
//! towers gather (downtown, the station blocks) and the railway depot.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{LotKind, Preset, StationKind, generate};
use us_render::{Camera, LotState, Renderer, Scene, Style};

fn mature_level(kind: LotKind, density: f64) -> u8 {
    let top = match kind {
        LotKind::House => 3,
        LotKind::Row | LotKind::Workshop => 4,
        LotKind::Apartments | LotKind::Office => 5,
        LotKind::Shop | LotKind::Strip | LotKind::BigBox | LotKind::Warehouse => 3,
        LotKind::Gas | LotKind::FastFood => 2,
        _ => 1,
    };
    (1 + (density * 4.4) as u8).min(top)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args
        .next()
        .unwrap_or_else(|| "target/skyline".into())
        .into();
    let preset = args
        .next()
        .and_then(|n| Preset::from_name(&n))
        .unwrap_or(Preset::Grid);
    let seed: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1);
    std::fs::create_dir_all(&out).expect("create output dir");
    let town = generate(preset, seed);
    let mut scene = Scene::from_town(&town);
    let changes: Vec<(usize, Option<u8>, LotState)> = town
        .lots
        .iter()
        .enumerate()
        .map(|(i, l)| (i, Some(mature_level(l.kind, l.density)), LotState::Normal))
        .collect();
    scene.set_lots(&changes);
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
    let mut views = vec![
        ("downtown", hall, 420.0, 30.0, 32.0),
        ("downtown-close", hall, 200.0, 210.0, 28.0),
    ];
    if let Some(st) = town.stations.iter().find(|s| s.kind == StationKind::Depot) {
        views.push((
            "station",
            Vec3::new(st.pos.x as f32, st.pos.y as f32, 0.0),
            70.0,
            (st.facing.to_degrees() as f32) + 30.0,
            40.0,
        ));
    }
    if let Some(st) = town
        .stations
        .iter()
        .find(|s| s.kind == StationKind::Elevator)
    {
        views.push((
            "elevator",
            Vec3::new(st.pos.x as f32, st.pos.y as f32, 0.0),
            90.0,
            (st.facing.to_degrees() as f32) + 20.0,
            30.0,
        ));
    }
    for (name, target, ext, yaw, pitch) in views {
        let target = Vec3::new(target.x, target.y, scene.height_at(target.x, target.y));
        let cam = Camera::framing(target, ext, aspect, yaw, pitch);
        let style = Style {
            supersample: 2,
            shadow_size: 4096,
            ink_width: 1,
            ..Style::default()
        };
        let f = r.render_to_rgba(&scene, &cam, &style, w, h);
        let path = out.join(format!("{}-{seed}-{name}.png", preset.name()));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!("{}", path.display());
    }
}
