//! Renders and kit checks. GPU tests skip cleanly when the machine has no adapter (hosted CI).

use glam::Vec3;
use us_gen::{Preset, generate};
use us_render::kit::{self, ALL_KINDS};
use us_render::obj::newell;
use us_render::{Camera, Mesh, Renderer, Rng, Scene, Style};

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

const GOLDEN: u64 = 0x0706_a25a_8553_b146;
