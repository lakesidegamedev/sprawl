//! Pieces for a town that grows while you watch: ground meshes per addition, and building
//! variants (under construction, empty) from the same models the static scene uses.

use crate::ground::{Ground, layer};
use crate::land::Land;
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
    base_ground_on(&Land::flat(t.size as f32), t, farm_additions)
}

/// Like `base_ground`, with the hills: the land is a grid of faces, fields and sand lie on it
/// and each river band or lake is a flat sheet at its water level.
pub fn base_ground_on(land: &Land, t: &Town, farm_additions: &[usize]) -> Mesh {
    let mut g = Ground::on(land);
    g.terrain(land, 1500.0, GRASS);
    for &ai in farm_additions {
        let a = &t.additions[ai];
        if let Some(c) = zone_colour(a.zone, a.pattern, t.seed, ai) {
            g.polygon(&a.polygon, layer::ZONE, c);
        }
    }
    add_water(&mut g, land, t);
    g.mesh
}

/// Sand around each water body and the water itself, flat at its level.
pub fn add_water(g: &mut Ground, land: &Land, t: &Town) {
    for w in &t.water {
        g.polygon(&Ground::grow(w, 14.0), layer::SAND, SAND);
        if land.is_flat() {
            g.polygon(w, layer::WATER, WATER);
        } else {
            g.polygon_level(w, land.water_level(w) + layer::WATER - layer::BASE, WATER);
        }
    }
}

/// The paved land of one addition (its zone colour).
pub fn zone_ground(t: &Town, index: usize) -> Mesh {
    zone_ground_on(&Land::flat(t.size as f32), t, index)
}

/// Like `zone_ground`, draped on the hills.
pub fn zone_ground_on(land: &Land, t: &Town, index: usize) -> Mesh {
    let mut g = Ground::on(land);
    let a = &t.additions[index];
    if let Some(c) = zone_colour(a.zone, a.pattern, t.seed, index) {
        g.polygon(&a.polygon, layer::ZONE, c);
    }
    g.mesh
}

/// One road to draw, with an optional roadworks state (progress 0 to 1).
#[derive(Clone, Copy, Debug)]
pub struct RoadDraw<'a> {
    pub class: RoadClass,
    pub points: &'a [Pt],
    /// `Some(progress)` while the road is being rebuilt: bare earth with orange edge strips.
    pub works: Option<f32>,
}

/// Marker colour of snow banked along a road edge: drawn in winter only.
pub const SNOW_EDGE: Rgb = [247, 248, 253];
pub const DIRT: Rgb = [190, 152, 104];
pub const CONE_ORANGE: Rgb = [238, 132, 40];

/// Roads and turning bulbs, drawn with dark edges under the surface the way the static scene does.
pub fn roads_ground(roads: &[(RoadClass, &[Pt])], culs: &[Pt]) -> Mesh {
    roads_ground_on(&Land::flat(1.0), roads, culs)
}

/// Like `roads_ground`, draped on the hills with bridges over water.
pub fn roads_ground_on(land: &Land, roads: &[(RoadClass, &[Pt])], culs: &[Pt]) -> Mesh {
    let draws: Vec<RoadDraw> = roads
        .iter()
        .map(|&(class, points)| RoadDraw {
            class,
            points,
            works: None,
        })
        .collect();
    roads_ground_state_on(land, &draws, culs)
}

/// Like `roads_ground`, with roadworks.
pub fn roads_ground_state(roads: &[RoadDraw], culs: &[Pt]) -> Mesh {
    roads_ground_state_on(&Land::flat(1.0), roads, culs)
}

/// Like `roads_ground_state`, draped on the hills.
pub fn roads_ground_state_on(land: &Land, roads: &[RoadDraw], culs: &[Pt]) -> Mesh {
    let mut g = Ground::on(land);
    add_roads(&mut g, roads, culs);
    g.mesh
}

/// Add roads, turning bulbs, markings and rails to a ground builder.
pub fn add_roads(g: &mut Ground, roads: &[RoadDraw], culs: &[Pt]) {
    let mut order: Vec<usize> = (0..roads.len())
        .filter(|&i| roads[i].class != RoadClass::Rail)
        .collect();
    order.sort_by_key(|&i| (road_rank(roads[i].class), i));
    for r in roads {
        let hw = if r.class == RoadClass::Rail {
            2.6
        } else {
            r.class.half_width() as f32
        };
        g.bridge(r.points, hw);
    }
    for &i in &order {
        let r = &roads[i];
        g.ribbon(
            r.points,
            r.class.half_width() as f32 + 0.5,
            0.0,
            layer::EDGE,
            EDGE,
        );
    }
    for &c in culs {
        g.disc(c, 9.5, layer::EDGE, EDGE);
    }
    for &c in culs {
        g.disc(c, 9.0, layer::ROAD, ASPHALT);
    }
    for &i in &order {
        let r = &roads[i];
        let col = if r.works.is_some() {
            DIRT
        } else if r.class == RoadClass::Highway {
            ASPHALT_HWY
        } else {
            ASPHALT
        };
        g.ribbon(r.points, r.class.half_width() as f32, 0.0, layer::ROAD, col);
    }
    for &i in &order {
        let r = &roads[i];
        let hw = r.class.half_width() as f32;
        if let Some(p) = r.works {
            for side in [-1.0f32, 1.0] {
                g.ribbon(r.points, 0.35, side * (hw - 0.6), layer::MARK, CONE_ORANGE);
            }
            // fresh surface laid so far, as a dark stripe down the middle
            if p > 0.5 {
                g.ribbon(
                    r.points,
                    (hw - 1.4).max(0.4),
                    0.0,
                    layer::MARK - 0.02,
                    ASPHALT,
                );
            }
        } else {
            if matches!(r.class, RoadClass::Arterial | RoadClass::Highway) {
                g.ribbon(r.points, 0.22, 0.0, layer::MARK, YELLOW);
            }
            // snow banked along both edges, under the road surface so side streets cut through
            // it: the shader shows it in winter only
            if r.class != RoadClass::Highway {
                for side in [-1.0f32, 1.0] {
                    g.ribbon(
                        r.points,
                        0.8,
                        side * (hw + 0.3),
                        layer::ROAD - 0.02,
                        SNOW_EDGE,
                    );
                }
            }
        }
    }
    for r in roads {
        if r.class == RoadClass::Rail {
            g.ribbon(r.points, 2.6, 0.0, layer::RAIL, BALLAST);
            g.ribbon(r.points, 0.2, 0.9, layer::RAIL + 0.06, STEEL);
            g.ribbon(r.points, 0.2, -0.9, layer::RAIL + 0.06, STEEL);
        }
    }
}

fn mix(c: Rgb, to: Rgb, k: f32) -> Rgb {
    let m = |a: u8, b: u8| {
        (a as f32 + (b as f32 - a as f32) * k)
            .round()
            .clamp(0.0, 255.0) as u8
    };
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
        if p.flat {
            // windows and doors go dark
            p.color = [52, 54, 62];
            continue;
        }
        let g = ((p.color[0] as u32 * 3 + p.color[1] as u32 * 6 + p.color[2] as u32) / 10) as f32;
        let d = (g * 0.62).round().clamp(40.0, 150.0) as u8;
        p.color = mix(p.color, [d.saturating_add(6), d, d.saturating_sub(6)], 0.92);
    }
    o
}
