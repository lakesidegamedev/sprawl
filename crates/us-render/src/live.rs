//! Pieces for a town that grows while you watch: ground meshes per addition, and building
//! variants (under construction, empty) from the same models the static scene uses.

use crate::ground::{Ground, layer};
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::scene::{
    ASPHALT, ASPHALT_HWY, BALLAST, CAMPUS, CEMETERY, EDGE, FIELDS, GRASS, PARK_GREEN, PAVING_CORE,
    PAVING_INDUSTRY, PAVING_MALL, SAND, STEEL, WATER, YELLOW,
};
use us_gen::{Pattern, Pt, RoadClass, Town, Zone};

fn hash2(a: u64, b: u64) -> u64 {
    crate::rng::Rng::new(a, b).next_u64()
}

/// Ground colour of an addition's land, if it has one (houses sit on plain grass).
pub fn zone_colour(zone: Zone, pattern: Pattern, seed: u64, index: usize) -> Option<Rgb> {
    match zone {
        Zone::Core => Some(PAVING_CORE),
        Zone::Industry => Some(PAVING_INDUSTRY),
        Zone::Mall => Some(PAVING_MALL),
        Zone::Campus => Some(CAMPUS),
        Zone::Park => Some(if pattern == Pattern::Cemetery {
            CEMETERY
        } else {
            PARK_GREEN
        }),
        Zone::Farm => Some(FIELDS[(hash2(seed, index as u64) % FIELDS.len() as u64) as usize]),
        Zone::Inner | Zone::Outer => None,
    }
}

fn road_rank(c: RoadClass) -> u8 {
    match c {
        RoadClass::Alley => 0,
        RoadClass::Local => 1,
        RoadClass::Collector => 2,
        RoadClass::Arterial => 3,
        RoadClass::Highway => 4,
        RoadClass::Rail => 5,
    }
}

/// Grass, the far-off land, farm fields and the river: the ground that is there from day one.
pub fn base_ground(t: &Town, farm_additions: &[usize]) -> Mesh {
    let mut g = Ground::default();
    let s = t.size as f32;
    g.rect(-1500.0, -1500.0, s + 1500.0, s + 1500.0, layer::BASE, GRASS);
    for &ai in farm_additions {
        let a = &t.additions[ai];
        if let Some(c) = zone_colour(a.zone, a.pattern, t.seed, ai) {
            g.polygon(&a.polygon, layer::ZONE, c);
        }
    }
    for w in &t.water {
        g.polygon(&Ground::grow(w, 14.0), layer::SAND, SAND);
        g.polygon(w, layer::WATER, WATER);
    }
    g.mesh
}

/// The paved land of one addition (its zone colour).
pub fn zone_ground(t: &Town, index: usize) -> Mesh {
    let mut g = Ground::default();
    let a = &t.additions[index];
    if let Some(c) = zone_colour(a.zone, a.pattern, t.seed, index) {
        g.polygon(&a.polygon, layer::ZONE, c);
    }
    g.mesh
}

/// Roads and turning bulbs, drawn with dark edges under the surface the way the static scene does.
pub fn roads_ground(roads: &[(RoadClass, &[Pt])], culs: &[Pt]) -> Mesh {
    let mut g = Ground::default();
    let mut order: Vec<usize> = (0..roads.len())
        .filter(|&i| roads[i].0 != RoadClass::Rail)
        .collect();
    order.sort_by_key(|&i| (road_rank(roads[i].0), i));
    for &i in &order {
        let (c, pts) = roads[i];
        g.ribbon(pts, c.half_width() as f32 + 0.5, 0.0, layer::EDGE, EDGE);
    }
    for &c in culs {
        g.disc(c, 9.5, layer::EDGE, EDGE);
    }
    for &c in culs {
        g.disc(c, 9.0, layer::ROAD, ASPHALT);
    }
    for &i in &order {
        let (c, pts) = roads[i];
        let col = if c == RoadClass::Highway {
            ASPHALT_HWY
        } else {
            ASPHALT
        };
        g.ribbon(pts, c.half_width() as f32, 0.0, layer::ROAD, col);
    }
    for &i in &order {
        let (c, pts) = roads[i];
        if matches!(c, RoadClass::Arterial | RoadClass::Highway) {
            g.ribbon(pts, 0.22, 0.0, layer::MARK, YELLOW);
        }
    }
    for (c, pts) in roads {
        if *c == RoadClass::Rail {
            g.ribbon(pts, 2.6, 0.0, layer::RAIL, BALLAST);
            g.ribbon(pts, 0.2, 0.9, layer::RAIL + 0.06, STEEL);
            g.ribbon(pts, 0.2, -0.9, layer::RAIL + 0.06, STEEL);
        }
    }
    g.mesh
}

fn mix(c: Rgb, to: Rgb, k: f32) -> Rgb {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * k).round().clamp(0.0, 255.0) as u8;
    [m(c[0], to[0]), m(c[1], to[1]), m(c[2], to[2])]
}

/// A copy of the model as a building site: lowered to `progress` (0 to 1) of its height and
/// washed toward dusty tan.
pub fn construction(obj: &Obj, progress: f32) -> Obj {
    let h = 0.18 + 0.82 * progress.clamp(0.0, 1.0);
    let mut o = obj.clone();
    for p in &mut o.polys {
        for v in &mut p.pts {
            v[2] *= h;
        }
        p.color = mix(p.color, [204, 178, 120], 0.55);
    }
    for s in &mut o.solids {
        s.centre[2] *= h;
    }
    o
}

/// A copy of the model gone grey: nobody lives or works here any more.
pub fn abandoned(obj: &Obj) -> Obj {
    let mut o = obj.clone();
    for p in &mut o.polys {
        let g = ((p.color[0] as u32 * 3 + p.color[1] as u32 * 6 + p.color[2] as u32) / 10) as u8;
        let grey = [g.saturating_sub(10), g.saturating_sub(8), g.saturating_sub(4)];
        p.color = mix(p.color, grey, 0.88);
        p.flat = false;
    }
    o
}
