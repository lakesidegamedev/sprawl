//! The building kit's variety: every region, kind and level builds sound, repeatable geometry
//! and offers many distinct looks.

use std::collections::HashSet;
use us_render::kit::{self, ALL_KINDS};
use us_render::obj::newell;
use us_render::{MAX_LEVEL, Obj, Region, Rng, build_lot_level_in};

fn closed_and_outward(obj: &Obj, what: &str) {
    let mut sums = vec![[0.0f64; 3]; obj.solids.len()];
    for p in &obj.polys {
        assert!(
            p.pts.len() >= 3,
            "{what}: face with fewer than three points"
        );
        assert!(
            p.pts.iter().all(|v| v.iter().all(|c| c.is_finite())),
            "{what}: not finite"
        );
        let n = newell(&p.pts);
        assert!(n != [0.0; 3], "{what}: degenerate face");
        if p.roof {
            assert!(n[2] > 0.05, "{what}: roof face points down {n:?}");
        }
        let Some(s) = p.solid else { continue };
        let k = p.pts.len() as f32;
        let c: Vec<f32> = (0..3)
            .map(|i| p.pts.iter().map(|v| v[i]).sum::<f32>() / k)
            .collect();
        let centre = obj.solids[s as usize].centre;
        let d = n[0] * (c[0] - centre[0]) + n[1] * (c[1] - centre[1]) + n[2] * (c[2] - centre[2]);
        assert!(d > -1e-4, "{what}: face points inward");
        for i in 1..p.pts.len() - 1 {
            let (a, b, cc) = (p.pts[0], p.pts[i], p.pts[i + 1]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [cc[0] - a[0], cc[1] - a[1], cc[2] - a[2]];
            sums[s as usize][0] += (u[1] * v[2] - u[2] * v[1]) as f64 / 2.0;
            sums[s as usize][1] += (u[2] * v[0] - u[0] * v[2]) as f64 / 2.0;
            sums[s as usize][2] += (u[0] * v[1] - u[1] * v[0]) as f64 / 2.0;
        }
    }
    for (i, s) in sums.iter().enumerate() {
        assert!(
            s[0].abs() < 1e-2 && s[1].abs() < 1e-2 && s[2] > -1e-2,
            "{what}: solid {i} is not closed: {s:?}"
        );
    }
}

#[test]
fn every_region_kind_and_level_is_sound() {
    for region in Region::ALL {
        for (k, kind) in ALL_KINDS.iter().enumerate() {
            for level in 1..=MAX_LEVEL {
                for variant in 0..12u64 {
                    let mut rng = Rng::new(variant * 31 + level as u64, k as u64);
                    let obj = build_lot_level_in(region, *kind, level, &mut rng);
                    closed_and_outward(&obj, &format!("{region:?} {kind:?} L{level} #{variant}"));
                }
            }
        }
    }
}

#[test]
fn models_are_repeatable() {
    for region in Region::ALL {
        for (k, kind) in ALL_KINDS.iter().enumerate() {
            let a = kit::build_lot_in(region, *kind, &mut Rng::new(9, k as u64));
            let b = kit::build_lot_in(region, *kind, &mut Rng::new(9, k as u64));
            assert_eq!(a.polys.len(), b.polys.len());
            for (p, q) in a.polys.iter().zip(&b.polys) {
                assert_eq!(p.pts, q.pts);
                assert_eq!(p.color, q.color);
            }
        }
    }
}

fn look(o: &Obj) -> u64 {
    let mut cols: Vec<[u8; 3]> = o.polys.iter().map(|p| p.color).collect();
    cols.sort_unstable();
    cols.dedup();
    let fp = o.footprint();
    let top = o
        .polys
        .iter()
        .flat_map(|p| p.pts.iter().map(|v| v[2]))
        .fold(0.0f32, f32::max);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut put = |v: i64| {
        h ^= v as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for c in cols {
        for k in c {
            put(k as i64);
        }
    }
    put(o.polys.len() as i64);
    for k in fp {
        put((k / 2.0).round() as i64);
    }
    put(top.round() as i64);
    h
}

/// The main building kinds each show a lot of distinct looks, and regions differ.
#[test]
fn homes_shops_and_blocks_have_many_looks() {
    use us_gen::LotKind::*;
    for (kind, level, want) in [
        (House, 2, 600),
        (House, 3, 600),
        (Shop, 2, 300),
        (Apartments, 4, 300),
        (Office, 3, 150),
    ] {
        let mut seen = HashSet::new();
        for i in 0..1500u64 {
            let o = build_lot_level_in(Region::Mixed, kind, level, &mut Rng::new(5, i));
            seen.insert(look(&o));
        }
        assert!(
            seen.len() >= want,
            "{kind:?} L{level}: only {} looks",
            seen.len()
        );
    }
    let mut prairie = HashSet::new();
    let mut sunbelt = HashSet::new();
    for i in 0..300u64 {
        prairie.insert(look(&kit::build_lot_in(
            Region::Prairie,
            House,
            &mut Rng::new(5, i),
        )));
        sunbelt.insert(look(&kit::build_lot_in(
            Region::Sunbelt,
            House,
            &mut Rng::new(5, i),
        )));
    }
    assert!(
        prairie.intersection(&sunbelt).count() < prairie.len() / 2,
        "regions look alike"
    );
}
