//! Capture the renderer graphics prototype with fixed towns and cameras.
//! cargo run -p us-render --example graphics_prototype -- <output directory>

use std::path::PathBuf;

use glam::Vec3;
use us_gen::{Preset, Pt, RoadClass, Town, generate};
use us_render::{Calendar, Camera, Renderer, Scene, Style, WeatherLook};

const WIDTH: u32 = 850;
const HEIGHT: u32 = 1060;

fn crossing(a: &[Pt], b: &[Pt]) -> Option<Pt> {
    let p = a[0];
    let q = b[0];
    let r = a[1] - p;
    let s = b[1] - q;
    let d = r.cross(s);
    if d.abs() < 1e-8 {
        return None;
    }
    let t = (q - p).cross(s) / d;
    let u = (q - p).cross(r) / d;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(p + r * t)
}

fn rail_crossing(town: &Town) -> Pt {
    town.roads
        .iter()
        .filter(|road| road.class == RoadClass::Rail)
        .flat_map(|rail| {
            town.roads
                .iter()
                .filter(|road| road.class == RoadClass::Arterial)
                .flat_map(|road| {
                    rail.points
                        .windows(2)
                        .flat_map(|a| road.points.windows(2).filter_map(move |b| crossing(a, b)))
                })
        })
        .min_by(|a, b| a.dist(town.hall).total_cmp(&b.dist(town.hall)))
        .expect("Grid seed has a rail and arterial crossing")
}

fn focus(scene: &Scene, p: Pt, width: f32, tilt: f32) -> Camera {
    let (x, y) = (p.x as f32, p.y as f32);
    Camera::framing(
        Vec3::new(x, y, scene.land.height(x, y)),
        width,
        WIDTH as f32 / HEIGHT as f32,
        30.0,
        tilt,
    )
}

fn main() {
    let out = PathBuf::from(std::env::args_os().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).expect("create output directory");
    let mut renderer = Renderer::new().expect("graphics adapter");
    let style = Style {
        supersample: 1,
        shadow_size: 2048,
        ink_width: 1,
        ..Style::default()
    };
    let mut capture = |name: &str, scene: &Scene, camera: &Camera| {
        let frame = renderer.render_to_rgba(scene, camera, &style, WIDTH, HEIGHT);
        image::save_buffer(
            out.join(format!("{name}.png")),
            &frame.rgba,
            frame.width,
            frame.height,
            image::ColorType::Rgba8,
        )
        .expect("write capture");
        eprintln!("{name}: {} triangles", frame.stats.triangles);
    };

    let town = generate(Preset::Crossroads, 1);
    let mut scene = Scene::from_town(&town);
    let camera = focus(&scene, town.hall, 145.0, 45.0);
    for (name, month, day, wet, overcast, snow) in [
        ("01-summer", 7, 18, 0.0, 0.0, 0.0),
        ("02-early-autumn", 9, 8, 0.0, 0.0, 0.0),
        ("03-autumn-rain", 10, 11, 0.96, 0.68, 0.0),
        ("04-drying", 10, 12, 0.39, 0.12, 0.0),
        ("05-first-snow", 12, 9, 0.05, 0.27, 0.35),
    ] {
        scene.set_calendar(Some(Calendar::from_date(month, day, 14.0)));
        scene.set_weather(WeatherLook {
            active: true,
            wet,
            overcast,
            snow,
            ..WeatherLook::default()
        });
        capture(name, &scene, &camera);
    }

    let town = generate(Preset::Grid, 1);
    let mut scene = Scene::from_town(&town);
    scene.set_calendar(Some(Calendar::from_date(7, 18, 14.0)));
    capture(
        "06-rail-crossing",
        &scene,
        &focus(&scene, rail_crossing(&town), 98.0, 56.0),
    );
}
