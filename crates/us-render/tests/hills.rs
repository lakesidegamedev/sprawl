//! Hills: ground draped on the land, bridges, picking and the lot fit.

use glam::Vec3;
use us_gen::{LotKind, Preset, pt};
use us_render::ground::{Ground, layer};
use us_render::{Camera, Land, Rng, kit, pick_terrain, ray_ground};

/// A 4 km style grid: a rolling surface with a valley and a river band across it.
fn rolling() -> Land {
    let n = 129;
    let size = 2048.0f32;
    let step = size / (n - 1) as f32;
    let mut d = Vec::new();
    for j in 0..n {
        for i in 0..n {
            let (x, y) = (i as f32 * step, j as f32 * step);
            let valley = 18.0 * (-((y - 1000.0) / 260.0).powi(2)).exp();
            d.push(30.0 - valley + 5.0 * (x / 200.0).sin() + 4.0 * (y / 170.0).cos());
        }
    }
    let river = vec![
        pt(-50.0, 960.0),
        pt(2100.0, 960.0),
        pt(2100.0, 1040.0),
        pt(-50.0, 1040.0),
    ];
    Land::from_grid(n, size, d, vec![river])
}

#[test]
fn polygons_lie_on_the_surface() {
    let land = rolling();
    let mut g = Ground::on(&land);
    let ring = [
        pt(100.0, 100.0),
        pt(700.0, 130.0),
        pt(640.0, 500.0),
        pt(90.0, 430.0),
    ];
    g.polygon(&ring, layer::ZONE, [10, 200, 10]);
    let m = &g.mesh;
    assert!(m.triangles() > 50, "the polygon is cut along the grid");
    for v in &m.vertices {
        let want = land.height(v.pos[0], v.pos[1]) + layer::ZONE;
        assert!((v.pos[2] - want).abs() < 1e-3, "vertex off the surface");
    }
    // every face points up: the cut keeps the winding
    for t in m.plain.chunks(3) {
        let (a, b, c) = (
            Vec3::from(m.vertices[t[0] as usize].pos),
            Vec3::from(m.vertices[t[1] as usize].pos),
            Vec3::from(m.vertices[t[2] as usize].pos),
        );
        assert!((b - a).cross(c - a).z > 0.0, "face turned over");
    }
}

#[test]
fn a_road_over_the_river_gets_a_deck_and_walls() {
    let land = rolling();
    let line = [pt(300.0, 1000.0 - 400.0), pt(300.0, 1000.0 + 400.0)];
    let bed = land.bed(&line);
    let spans = bed.spans();
    assert_eq!(spans.len(), 1, "one bridge");
    let (a, b) = spans[0];
    let deck = bed.deck[a];
    assert!(
        deck > land.height(300.0, 1000.0) + 2.0,
        "deck stands over the water"
    );
    assert!(bed.pts[a].y < 960.0 && bed.pts[b].y >= 1035.0);
    let mut g = Ground::on(&land);
    g.bridge(&line, 6.0);
    assert!(!g.mesh.cast.is_empty(), "walls and parapets cast shadows");
    assert!(g.mesh.vertices.iter().any(|v| v.pos[2] > deck));
}

#[test]
fn picking_finds_the_hill_under_the_cursor() {
    let land = rolling();
    let target = Vec3::new(600.0, 600.0, land.height(600.0, 600.0));
    let cam = Camera::framing(target, 400.0, 1.6, 20.0, 35.0);
    let view = cam.view(1.6);
    let p = pick_terrain(&view, (0.0, 0.0), &land).expect("hit");
    assert!((p.x - target.x).abs() < 3.0 && (p.y - target.y).abs() < 3.0);
    assert!((p.z - land.height(p.x, p.y)).abs() < 0.05);
    // a ray going up never lands
    assert!(ray_ground(Vec3::new(0.0, 0.0, 100.0), Vec3::Z, 1000.0, &land).is_none());
}

#[test]
fn level_land_is_the_old_plane() {
    let flat = Land::flat(100.0);
    let cam = Camera::framing(Vec3::new(50.0, 50.0, 0.0), 100.0, 1.6, 0.0, 40.0);
    let view = cam.view(1.6);
    let a = pick_terrain(&view, (0.3, -0.2), &flat).unwrap();
    let b = us_render::pick_ground(&view, (0.3, -0.2)).unwrap();
    assert!((a - b).length() < 1e-4);
}

#[test]
fn every_model_fits_inside_its_lot() {
    for preset in Preset::ALL {
        for kind in kit::ALL_KINDS {
            for seed in 0..6u64 {
                let radius = match kind {
                    LotKind::House => 6.5,
                    LotKind::Row | LotKind::Shop | LotKind::FastFood | LotKind::Gas => 8.0,
                    _ => 14.0,
                };
                let mut rng = Rng::new(7, seed);
                let o = kit::build_lot_for(preset, kind, radius, &mut rng);
                let far = o
                    .polys
                    .iter()
                    .flat_map(|p| p.pts.iter())
                    .map(|v| v[0].hypot(v[1]))
                    .fold(0.0f32, f32::max);
                assert!(
                    far <= radius * kit::LOT_FIT + 1e-3,
                    "{kind:?} reaches {far} m in a {radius} m lot"
                );
            }
        }
    }
}
