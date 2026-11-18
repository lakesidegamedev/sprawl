//! Renders and kit checks. GPU tests skip cleanly when the machine has no adapter (hosted CI).

use glam::Vec3;
use us_gen::{Preset, generate};
use us_render::kit::{self, ALL_KINDS};
use us_render::obj::newell;
use us_render::{Calendar, Camera, Mesh, Renderer, Rng, Scene, Style, WeatherLook};

fn renderer() -> Option<Renderer> {
    match Renderer::new() {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

#[test]
fn tiny_town_renders_without_panicking() {
    let Some(mut r) = renderer() else { return };
    let mut town = generate(Preset::Grid, 1);
    town.lots.truncate(40);
    let scene = Scene::from_town(&town);
    let cam = Camera::framing(
        Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0),
        300.0,
        1.6,
        45.0,
        40.0,
    );
    let style = Style {
        shadow_size: 1024,
        ..Style::default()
    };
    let f = r.render_to_rgba(&scene, &cam, &style, 160, 100);
    assert_eq!((f.width, f.height), (160, 100));
    assert_eq!(f.rgba.len(), 160 * 100 * 4);
    assert!(f.rgba.chunks_exact(4).all(|p| p[3] == 255));
    // not a blank frame: more than one colour, and some dark ink
    let first = &f.rgba[..3];
    assert!(f.rgba.chunks_exact(4).any(|p| &p[..3] != first));
    assert!(f.rgba.chunks_exact(4).any(|p| p[0] < 60 && p[1] < 60));
}

#[test]
fn rendering_is_repeatable() {
    let Some(mut r) = renderer() else { return };
    let mut town = generate(Preset::Winding, 2);
    town.lots.truncate(60);
    let scene = Scene::from_town(&town);
    let cam = Camera::framing(
        Vec3::new(town.hall.x as f32, town.hall.y as f32, 0.0),
        300.0,
        1.6,
        45.0,
        40.0,
    );
    let style = Style {
        shadow_size: 1024,
        ..Style::default()
    };
    let a = r.render_to_rgba(&scene, &cam, &style, 96, 60);
    let b = r.render_to_rgba(&scene, &cam, &style, 96, 60);
    assert_eq!(a.rgba, b.rgba);
}

#[test]
fn fog_moves_over_ground_and_stays_below_high_roofs() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut mesh = Mesh::default();
    mesh.push_poly(
        &[
            [0., 0., 0.],
            [200., 0., 0.],
            [200., 200., 0.],
            [0., 200., 0.],
        ],
        [70, 66, 54],
        true,
        false,
        0,
    );
    let roof = [
        [80., 80., 32.],
        [120., 80., 32.],
        [120., 120., 32.],
        [80., 120., 32.],
    ];
    mesh.push_poly(&roof, [180, 70, 55], true, false, 6);
    let target = Vec3::new(100.0, 100.0, 0.0);
    let mut scene = Scene::dynamic(200.0, mesh, target);
    let camera = Camera::framing(target, 160.0, 1.0, 0.0, 90.0);
    let style = Style {
        shadow_size: 128,
        supersample: 1,
        ..Style::default()
    };
    let dry = renderer.render_to_rgba(&scene, &camera, &style, 128, 128);
    scene.set_weather(WeatherLook {
        ground_fog: 1.0,
        wind: [2.0, 0.5],
        ..WeatherLook::default()
    });
    let still = renderer.render_to_rgba(&scene, &camera, &style, 128, 128);
    let offset = (64 * 128 + 64) * 4;
    assert_eq!(
        &dry.rgba[offset..offset + 4],
        &still.rgba[offset..offset + 4]
    );
    let changed_pixels = dry
        .rgba
        .chunks_exact(4)
        .zip(still.rgba.chunks_exact(4))
        .filter(|(a, b)| a[0].abs_diff(b[0]) > 15)
        .count();
    assert!(changed_pixels > 128 * 128 / 10);
    scene.set_time(32.0);
    let moving = renderer.render_to_rgba(&scene, &camera, &style, 128, 128);
    assert_ne!(still.rgba, moving.rgba);
}

#[test]
fn world_precipitation_is_occluded_by_a_high_roof() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let plane = |z| [[0., 0., z], [200., 0., z], [200., 200., z], [0., 200., z]];
    let target = Vec3::new(100.0, 100.0, 0.0);
    let mut covered = Mesh::default();
    covered.push_poly(&plane(65.0), [90, 87, 81], true, false, 5);
    let mut open = Mesh::default();
    open.push_poly(&plane(0.0), [90, 87, 81], true, false, 0);
    let camera = Camera::framing(target, 130.0, 1.0, 14.0, 82.0);
    let style = Style {
        shadow_size: 128,
        supersample: 1,
        ..Style::default()
    };
    let mut outside = Scene::dynamic(200.0, open, target);
    let mut indoors = Scene::dynamic(200.0, covered, target);
    for (scene, covered) in [(&mut outside, false), (&mut indoors, true)] {
        let dry = renderer.render_to_rgba(scene, &camera, &style, 172, 172);
        for (rain, snowfall) in [(1.0, 0.0), (0.0, 1.0)] {
            scene.set_weather(WeatherLook {
                rain,
                snowfall,
                ..WeatherLook::default()
            });
            let wet = renderer.render_to_rgba(scene, &camera, &style, 172, 172);
            assert_eq!(dry.rgba == wet.rgba, covered, "rain {rain} snow {snowfall}");
        }
    }
}

#[test]
fn night_windows_are_stable_when_outline_ids_change() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let target = Vec3::new(100.0, 100.0, 0.0);
    let camera = Camera::framing(target, 130.0, 1.0, 0.0, 90.0);
    let style = Style {
        shadow_size: 128,
        supersample: 1,
        ..Style::default()
    };
    let mut draw = |start_id: u32, hour| {
        let mut mesh = Mesh::default();
        for i in 0..8 {
            let x = i as f32 * 10.0 + 62.0;
            let y = 100.0;
            mesh.push_poly(
                &[
                    [x, y, 0.0],
                    [x + 6.0, y, 0.0],
                    [x + 6.0, y + 7.0, 0.0],
                    [x, y + 7.0, 0.0],
                ],
                [98, 156, 204],
                true,
                false,
                start_id + i,
            );
        }
        let mut scene = Scene::dynamic(200.0, mesh, target);
        scene.set_calendar(Some(Calendar::from_date(9, 12, hour)));
        renderer
            .render_to_rgba(&scene, &camera, &style, 160, 160)
            .rgba
    };
    let evening = draw(20, 20.5);
    assert_eq!(evening, draw(5199, 20.5));
    assert!(
        evening
            .chunks_exact(4)
            .any(|px| px[0] > 170 && px[0] > px[2])
    );
    let next = draw(20, 20.51);
    assert!(evening.iter().zip(next).all(|(a, b)| a.abs_diff(b) < 12));
}

/// Every solid is watertight apart from its floor: the area-weighted normals sum to straight up.
/// Walls face away from their solid's centre, roofs face up.
#[test]
fn kit_solids_are_closed_and_face_outward() {
    for (k, kind) in ALL_KINDS.iter().enumerate() {
        for variant in 0..8u64 {
            let mut rng = Rng::new(variant, k as u64);
            let obj = kit::build_lot(*kind, &mut rng);
            let mut sums = vec![[0.0f64; 3]; obj.solids.len()];
            for p in &obj.polys {
                let n = newell(&p.pts);
                assert!(n != [0.0; 3], "{kind:?}: degenerate face");
                assert!(n.iter().all(|v| v.is_finite()));
                let c = {
                    let mut c = [0.0f32; 3];
                    for v in &p.pts {
                        for i in 0..3 {
                            c[i] += v[i] / p.pts.len() as f32;
                        }
                    }
                    c
                };
                if p.roof {
                    assert!(n[2] > 0.1, "{kind:?}: roof face points down {n:?}");
                }
                let Some(s) = p.solid else { continue };
                let centre = obj.solids[s as usize].centre;
                let out = [c[0] - centre[0], c[1] - centre[1], c[2] - centre[2]];
                let d = n[0] * out[0] + n[1] * out[1] + n[2] * out[2];
                assert!(d > -1e-4, "{kind:?} variant {variant}: face points inward");
                // area by fan
                let mut area = [0.0f64; 3];
                for i in 1..p.pts.len() - 1 {
                    let (a, b, cc) = (p.pts[0], p.pts[i], p.pts[i + 1]);
                    let (u, v) = (
                        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                        [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]],
                    );
                    area[0] += (u[1] * v[2] - u[2] * v[1]) as f64 / 2.0;
                    area[1] += (u[2] * v[0] - u[0] * v[2]) as f64 / 2.0;
                    area[2] += (u[0] * v[1] - u[1] * v[0]) as f64 / 2.0;
                }
                for i in 0..3 {
                    sums[s as usize][i] += area[i];
                }
            }
            for (i, s) in sums.iter().enumerate() {
                assert!(
                    s[0].abs() < 1e-2 && s[1].abs() < 1e-2 && s[2] > -1e-2,
                    "{kind:?} variant {variant}: solid {i} is not closed: {s:?}"
                );
            }
        }
    }
}

#[test]
fn every_lot_kind_builds_geometry() {
    for kind in ALL_KINDS {
        let obj = kit::build_lot(kind, &mut Rng::new(1, 1));
        assert!(obj.polys.len() > 3, "{kind:?} is empty");
    }
}

/// A fixed set of lots always builds the same bytes. Float maths goes through libm, so the hash
/// holds on every platform.
#[test]
fn mesh_hash_for_a_fixed_lot_set_is_stable() {
    let mut mesh = Mesh::default();
    let mut id = 1u32;
    for (i, kind) in ALL_KINDS.iter().enumerate() {
        let obj = kit::build_lot(*kind, &mut Rng::new(42, i as u64));
        obj.emit(&mut mesh, &mut id, i as f32 * 50.0, 10.0, 0.5, 1.0);
    }
    let h = mesh.fingerprint();
    assert_eq!(h, GOLDEN, "mesh hash changed: {h:#018x}");
}

const GOLDEN: u64 = 0xb24b_afe6_d5f9_eb5b;
