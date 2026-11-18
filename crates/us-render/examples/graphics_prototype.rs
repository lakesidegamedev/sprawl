//! Capture the renderer graphics prototype with fixed towns and cameras.
//! cargo run -p us-render --example graphics_prototype -- <output directory> [--reel]

use std::path::PathBuf;

use glam::Vec3;
use us_gen::{Preset, Pt, RoadClass, Town, generate};
use us_render::{
    Calendar, Camera, Overlay, OverlayKind, Renderer, RoadInfo, Scene, Style, WeatherLook,
};
use us_traffic::{AssignParams, Demand, DemandParams, Network, assign};

const WIDTH: u32 = 850;
const HEIGHT: u32 = 1060;
const REEL_SIZE: (u32, u32) = (640, 798);

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

fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn capture_traffic(
    town: &Town,
    scene: &mut Scene,
    capture: &mut impl FnMut(&str, &Scene, &Camera),
) {
    let network = Network::from_town(town);
    let peak = Demand::from_town(
        town,
        &network,
        &DemandParams {
            trips_per_unit: 2.5,
            ..DemandParams::default()
        },
    );
    let mut quiet = peak.clone();
    quiet.total *= 0.4;
    quiet.od.iter_mut().for_each(|flow| *flow *= 0.4);
    let roads = network
        .streets
        .edges
        .iter()
        .map(|edge| RoadInfo {
            class: edge.class,
            points: edge.pts.clone(),
            works: None,
        })
        .collect();
    scene.set_geometry(Vec::new(), roads);
    scene.set_weather(WeatherLook::default());
    let camera = focus(scene, town.hall, 2100.0, 62.0);
    for (name, demand) in [("traffic-quiet", quiet), ("traffic-peak", peak)] {
        let result = assign(&network, &demand, &AssignParams::default());
        let over_capacity = result.edge_vc.iter().filter(|&&vc| vc >= 1.0).count();
        eprintln!(
            "{name}: {:.0} trips/hour, {over_capacity} streets at or over capacity, {:.1}% longer than free flow",
            demand.total,
            100.0 * (result.vht / result.vht_free - 1.0)
        );
        scene.set_overlay(Some(Overlay {
            kind: OverlayKind::Traffic,
            roads: result
                .edge_vc
                .iter()
                .map(|&vc| (vc / 1.5).clamp(0.0, 1.0) as f32)
                .collect(),
            lots: Vec::new(),
        }));
        capture(name, scene, &camera);
    }
}

fn capture_reel(mut capture: impl FnMut(&str, &Scene, &Camera)) {
    let town = generate(Preset::Crossroads, 1);
    let mut scene = Scene::from_town(&town);
    let camera = focus(&scene, town.hall, 145.0, 45.0);
    for i in 0..14 {
        let t = i as f32 / 13.0;
        scene.set_time(t * 3.0);
        scene.set_calendar(Some(Calendar::from_date(9, 1 + (28.0 * t) as u32, 14.0)));
        capture(&format!("season-{i:03}"), &scene, &camera);
    }
    for i in 0..38 {
        let t = i as f32 / 37.0;
        let front = ease((t - 0.07) / 0.26);
        let clearing = ease((t - 0.53) / 0.28);
        let rain = front * (1.0 - clearing);
        let wet = ease((t - 0.16) / 0.34) * (1.0 - 0.76 * ease((t - 0.7) / 0.3));
        scene.set_time(3.0 + i as f32 * 0.095);
        scene.set_calendar(Some(Calendar::from_date(10, 4, 14.0)));
        scene.set_weather(WeatherLook {
            active: true,
            wet,
            rain,
            overcast: front * (0.79 - clearing * 0.67),
            wind: [5.0 + 7.0 * rain, 4.0 * rain],
            ..WeatherLook::default()
        });
        capture(&format!("storm-{i:03}"), &scene, &camera);
    }

    capture_traffic(&town, &mut scene, &mut capture);

    let town = generate(Preset::Grid, 1);
    let mut scene = Scene::from_town(&town);
    scene.set_calendar(Some(Calendar::from_date(9, 17, 15.0)));
    let crossing = rail_crossing(&town);
    for i in 0..17 {
        let t = ease(i as f32 / 16.0);
        scene.set_time(i as f32 * 0.12);
        let mut camera = focus(&scene, crossing, 98.0 - 21.0 * t, 56.0 - 5.0 * t);
        camera.yaw_deg += 8.0 * t;
        capture(&format!("rail-{i:03}"), &scene, &camera);
    }
}

fn main() {
    let mut args = std::env::args_os().skip(1);
    let out = PathBuf::from(args.next().expect("output directory"));
    let reel = args.next().is_some_and(|flag| flag == "--reel");
    let (width, height) = if reel { REEL_SIZE } else { (WIDTH, HEIGHT) };
    std::fs::create_dir_all(&out).expect("create output directory");
    let mut renderer = Renderer::new().expect("graphics adapter");
    let style = Style {
        supersample: 1,
        shadow_size: 2048,
        ink_width: 1,
        ..Style::default()
    };
    let mut capture = |name: &str, scene: &Scene, camera: &Camera| {
        let frame = renderer.render_to_rgba(scene, camera, &style, width, height);
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

    if reel {
        capture_reel(capture);
        return;
    }

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
