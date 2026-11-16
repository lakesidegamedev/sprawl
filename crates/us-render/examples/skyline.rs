//! `cargo run --release -p us-render --example skyline -- <out_dir> [preset] [seed]`
//!
//! A grown-up town: every lot drawn at the level its density lets it reach, to show where the
//! towers gather (downtown, the station blocks) and the railway depot.

use glam::Vec3;
use std::path::PathBuf;
use us_gen::{LotKind, Preset, StationKind, generate};
use us_render::{Calendar, Camera, LotState, Renderer, Scene, Style};

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
    // optional hour of day (0 to 24) for a night look, and a view filter
    let hour: Option<f32> = args.next().and_then(|s| s.parse().ok());
    let only: Option<String> = args.next();
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
    if let Some(h) = hour {
        scene.set_calendar(Some(Calendar::from_date(6, 21, h)));
    }
    let mut r = Renderer::new().expect("adapter");
    let (w, h) = (1600u32, 1000u32);
    let aspect = w as f32 / h as f32;
    let hall = Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0);
    let mut views = vec![
        ("downtown", hall, 420.0, 30.0, 32.0),
        ("downtown-close", hall, 200.0, 210.0, 28.0),
    ];
    // a close look at the first lot of some kinds: a street of houses, a shop strip, industry
    for (name, kinds, ext) in [
        ("houses", &[LotKind::House][..], 110.0),
        ("shops", &[LotKind::Strip, LotKind::Shop][..], 110.0),
        (
            "industry",
            &[LotKind::Workshop, LotKind::Warehouse][..],
            130.0,
        ),
    ] {
        let mut it = town.lots.iter().filter(|l| kinds.contains(&l.kind));
        if let Some(l) = it.nth(25) {
            views.push((
                name,
                Vec3::new(l.centre.x as f32, l.centre.y as f32, 0.0),
                ext,
                (l.facing.to_degrees() as f32) + 200.0,
                34.0,
            ));
        }
    }
    views.push(("whole", Vec3::new(2048.0, 1900.0, 0.0), 4200.0, 30.0, 48.0));
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
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let target = Vec3::new(target.x, target.y, scene.height_at(target.x, target.y));
        let cam = Camera::framing(target, ext, aspect, yaw, pitch);
        let style = Style {
            supersample: 2,
            shadow_size: 4096,
            ink_width: 1,
            ..Style::default()
        };
        let f = r.render_to_rgba(&scene, &cam, &style, w, h);
        let tag = hour.map_or(String::new(), |h| format!("-h{h}"));
        let path = out.join(format!("{}-{seed}-{name}{tag}.png", preset.name()));
        image::save_buffer(&path, &f.rgba, f.width, f.height, image::ColorType::Rgba8)
            .expect("write png");
        eprintln!("{}", path.display());
    }
}
