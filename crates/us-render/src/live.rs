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

/// White of the surf along the sea's edge.
const SURF: Rgb = [244, 248, 250];

/// Fields and scrub in the dry colours of the desert.
const DRY_FIELDS: [Rgb; 5] = [
    [196, 174, 122],
    [186, 160, 112],
    [170, 164, 112],
    [204, 182, 132],
    [178, 150, 106],
];
const DRY_SCRUB: [Rgb; 3] = [[150, 160, 100], [160, 164, 104], [142, 152, 96]];

/// Ground colour of an addition's land in a town of style `p`: the red rock desert has dry fields
/// and scrub where the other styles have green.
pub fn zone_colour_in(
    p: us_gen::Preset,
    zone: Zone,
    pattern: Pattern,
    seed: u64,
    index: usize,
) -> Option<Rgb> {
    if p == us_gen::Preset::Mesa {
        match zone {
            Zone::Farm => {
                return Some(
                    DRY_FIELDS[(hash2(seed, index as u64) % DRY_FIELDS.len() as u64) as usize],
                );
            }
            Zone::Park if pattern != Pattern::Cemetery => {
                return Some(
                    DRY_SCRUB[(hash2(seed, index as u64) % DRY_SCRUB.len() as u64) as usize],
                );
            }
            _ => {}
        }
    }
    zone_colour(zone, pattern, seed, index)
}

/// The land of a town: plain grass, or the rock look of its style.
pub fn ground_terrain(g: &mut Ground, land: &Land, t: &Town) {
    match crate::rocklook::look_for(t.preset) {
        Some(look) if !land.is_flat() => g.terrain_look(land, 1500.0, look),
        _ => g.terrain(land, 1500.0, GRASS),
    }
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
    ground_terrain(&mut g, land, t);
    for &ai in farm_additions {
        let a = &t.additions[ai];
        if let Some(c) = zone_colour_in(t.preset, a.zone, a.pattern, t.seed, ai) {
            g.polygon(&a.polygon, layer::ZONE, c);
        }
    }
    add_water(&mut g, land, t);
    g.mesh
}

/// Sand around each water body and the water itself, flat at its level.
pub fn add_water(g: &mut Ground, land: &Land, t: &Town) {
    add_water_at(g, land, land, t);
}

/// A thin strip along the part of the sea's shore that lies on the map, for the surf.
fn surf_strip(ring: &[Pt], size: f64) -> Option<Vec<Pt>> {
    let shore: Vec<Pt> = ring
        .iter()
        .copied()
        .filter(|p| p.x < size + 1.0 && p.y < size + 1.0)
        .collect();
    if shore.len() < 3 {
        return None;
    }
    let n = shore.len();
    let mut land_side = Vec::with_capacity(n);
    for i in 0..n {
        let (a, b) = (shore[i.saturating_sub(1)], shore[(i + 1).min(n - 1)]);
        let d = us_gen::pt(b.x - a.x, b.y - a.y);
        let l = d.len().max(1e-6);
        // the sea is to the east: the normal that points west is the land side
        let mut nrm = us_gen::pt(-d.y / l, d.x / l);
        if nrm.x > 0.0 {
            nrm = us_gen::pt(-nrm.x, -nrm.y);
        }
        land_side.push(us_gen::pt(
            shore[i].x + nrm.x * 5.0,
            shore[i].y + nrm.y * 5.0,
        ));
    }
    let mut out = land_side;
    out.extend(shore.iter().rev().copied());
    Some(out)
}

/// Like `add_water`, with the water levels taken from `levels` (a finer copy of `land`).
fn add_water_at(g: &mut Ground, land: &Land, levels: &Land, t: &Town) {
    for w in &t.water {
        // the sea runs off the map: no sand ring, a crisp white surf line along its shore
        let sea = w.iter().any(|p| p.x > t.size || p.y > t.size);
        if sea {
            if !land.is_flat()
                && let Some(strip) = surf_strip(w, t.size as f64)
            {
                g.polygon(&strip, layer::SAND + 0.02, SURF);
            }
        } else {
            g.polygon(&Ground::grow(w, 14.0), layer::SAND, SAND);
        }
        if land.is_flat() {
            g.polygon(w, layer::WATER, WATER);
        } else {
            g.polygon_level(w, levels.water_level(w) + layer::WATER - layer::BASE, WATER);
        }
    }
}

/// The base ground again on a coarser copy of the land (`coarse`, built from `fine`), for a
/// chunk that is far from the camera. Water keeps the fine level so shores line up.
pub fn base_ground_far_on(fine: &Land, coarse: &Land, t: &Town, farm_additions: &[usize]) -> Mesh {
    let mut g = Ground::on(coarse);
    ground_terrain(&mut g, coarse, t);
    for &ai in farm_additions {
        let a = &t.additions[ai];
        if let Some(c) = zone_colour_in(t.preset, a.zone, a.pattern, t.seed, ai) {
            g.polygon(&a.polygon, layer::ZONE, c);
        }
    }
    add_water_at(&mut g, coarse, fine, t);
    g.mesh
}

/// The paved land of one addition (its zone colour).
pub fn zone_ground(t: &Town, index: usize) -> Mesh {
    zone_ground_on(&Land::flat(t.size as f32), t, index)
}

/// Like `zone_ground`, draped on the hills.
pub fn zone_ground_on(land: &Land, t: &Town, index: usize) -> Mesh {
    let mut g = Ground::on(land);
    let a = &t.additions[index];
    if let Some(c) = zone_colour_in(t.preset, a.zone, a.pattern, t.seed, index) {
        g.polygon(&a.polygon, layer::ZONE, c);
    }
    if a.zone == Zone::Park && a.pattern == Pattern::Park {
        crate::park::add_park(&mut g, land, &a.polygon, t.seed, index);
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
pub const DIRT_RUT: Rgb = [160, 122, 80];
pub const GRAVEL: Rgb = [176, 170, 158];
pub const FRESH_ASPHALT: Rgb = [58, 60, 68];

/// Pieces of `line` that are `on` metres long with `off` metres between them.
fn dashes(line: &[Pt], on: f64, off: f64) -> Vec<Vec<Pt>> {
    let mut out = Vec::new();
    let mut cur: Vec<Pt> = Vec::new();
    let (mut drawing, mut left) = (true, on);
    if let Some(&p) = line.first() {
        cur.push(p);
    }
    for w in line.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.dist(b);
        let mut t = 0.0;
        while len - t > left {
            t += left;
            let p = a.lerp(b, t / len);
            if drawing {
                cur.push(p);
                out.push(std::mem::take(&mut cur));
            } else {
                cur.push(p);
            }
            drawing = !drawing;
            left = if drawing { on } else { off };
        }
        left -= len - t;
        if drawing {
            cur.push(b);
        }
    }
    if drawing && cur.len() > 1 {
        out.push(cur);
    }
    out
}

/// A road being built, by how far along the work is: graded dirt, gravel, fresh asphalt, then the
/// finished surface with its markings, with a machine on the site and cones along the edges.
fn road_works(g: &mut Ground, r: &RoadDraw, hw: f32, p: f32) {
    let rut = layer::ROAD + 0.01;
    if p < 0.3 {
        // graded earth: two darker wheel tracks and a rough centre
        for side in [-1.0f32, 1.0] {
            g.ribbon(r.points, 0.55, side * hw * 0.45, rut, DIRT_RUT);
        }
        g.ribbon(r.points, hw * 0.25, 0.0, rut, DIRT_RUT);
    } else if p < 0.6 {
        // gravel laid down the middle, dirt shoulders left
        g.ribbon(
            r.points,
            (hw - 0.9).max(0.5),
            0.0,
            layer::ROAD + 0.02,
            GRAVEL,
        );
    } else {
        g.ribbon(
            r.points,
            (hw - 0.9).max(0.5),
            0.0,
            layer::ROAD + 0.02,
            GRAVEL,
        );
        g.ribbon(
            r.points,
            (hw - 0.9).max(0.5),
            0.0,
            layer::ROAD + 0.04,
            FRESH_ASPHALT,
        );
        if p >= 0.82 {
            // lines painted last
            let line = if matches!(r.class, RoadClass::Arterial | RoadClass::Highway) {
                YELLOW
            } else {
                [236, 232, 214]
            };
            for d in dashes(r.points, 3.5, 4.5) {
                g.ribbon(&d, 0.16, 0.0, layer::MARK + 0.01, line);
            }
        }
    }
    for side in [-1.0f32, 1.0] {
        g.ribbon(
            r.points,
            0.35,
            side * (hw - 0.6),
            layer::MARK + 0.02,
            CONE_ORANGE,
        );
    }
    // the machine: a truck while the earth is moved, a roller after
    let total: f64 = r.points.windows(2).map(|w| w[0].dist(w[1])).sum();
    let (at, ahead) = along(r.points, 0.5);
    if let Some(prop) = site_machine(p < 0.3).filter(|_| total >= 40.0) {
        let theta = libm::atan2(ahead.y - at.y, ahead.x - at.x) as f32;
        g.place_prop(&prop, at.x as f32, at.y as f32, theta);
    }
}

/// The point `share` of the way along a line, and a point a little further on.
fn along(line: &[Pt], share: f64) -> (Pt, Pt) {
    let total: f64 = line.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut left = total * share.clamp(0.0, 1.0);
    for w in line.windows(2) {
        let len = w[0].dist(w[1]);
        if left <= len && len > 1e-9 {
            let t = left / len;
            let t2 = ((left + 3.0) / len).min(1.0);
            return (w[0].lerp(w[1], t), w[0].lerp(w[1], t2));
        }
        left -= len;
    }
    let n = line.len();
    (line[n - 1], line[n - 1])
}

/// A dump truck (`true`) or a road roller (`false`), nose along local +x.
fn site_machine(truck: bool) -> Option<Obj> {
    let mut o = Obj::new();
    let yellow = [240, 190, 40];
    let dark = [48, 50, 58];
    if truck {
        o.bx(-2.4, -1.0, 0.5, 1.0, 1.0, 0.7, dark);
        o.bx(-2.4, -1.0, 0.7, 0.8, 1.0, 1.8, yellow);
        o.bx(1.0, -1.0, 0.5, 2.5, 1.0, 2.0, [220, 100, 40]);
        o.bx(1.9, -0.9, 1.3, 2.52, 0.9, 1.8, [120, 180, 220]);
        for x in [-1.6f32, 0.0, 1.8] {
            for y in [-1.05f32, 0.85] {
                o.bx(x - 0.45, y, 0.0, x + 0.45, y + 0.2, 0.9, dark);
            }
        }
        o.bx(-2.3, -0.9, 1.8, 0.7, 0.9, 2.0, [232, 180, 36]);
    } else {
        // two drums and a cab over a yellow body
        o.bx(-0.3, -1.0, 0.8, 2.2, 1.0, 1.7, yellow);
        o.bx(-1.9, -0.9, 0.8, -0.3, 0.9, 2.5, [240, 150, 30]);
        o.bx(-1.9, -0.8, 2.5, -0.2, 0.8, 2.7, dark);
        o.bx(1.5, -1.2, 0.0, 2.7, 1.2, 1.1, [150, 150, 156]);
        o.bx(-2.3, -1.15, 0.0, -1.1, 1.15, 1.05, [150, 150, 156]);
    }
    Some(o)
}

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

/// Like `roads_ground_state_on`, with roads of other chunks that only shape the junctions.
pub fn roads_ground_state_ctx_on(
    land: &Land,
    roads: &[RoadDraw],
    culs: &[Pt],
    context: &[RoadDraw],
) -> Mesh {
    let mut g = Ground::on(land);
    add_roads_with(&mut g, roads, culs, context);
    g.mesh
}

/// Add roads, turning bulbs, markings and rails to a ground builder.
pub fn add_roads(g: &mut Ground, roads: &[RoadDraw], culs: &[Pt]) {
    add_roads_with(g, roads, culs, &[]);
}

/// Like `add_roads`. `context` are roads drawn elsewhere that meet these: they are not drawn
/// here, they only decide where junctions are.
pub fn add_roads_with(g: &mut Ground, roads: &[RoadDraw], culs: &[Pt], context: &[RoadDraw]) {
    // ramps are drawn from the highway's edge, so they peel off it instead of cutting across
    let highways: Vec<&[Pt]> = roads
        .iter()
        .chain(context)
        .filter(|r| r.class == RoadClass::Highway)
        .map(|r| r.points)
        .collect();
    let cut_w: Vec<Option<crate::roadpaint::RampCut>> = roads
        .iter()
        .map(|r| crate::roadpaint::ramp_cut(r, &highways))
        .collect();
    let wedges: Vec<[Pt; 3]> = cut_w.iter().flatten().map(|c| c.wedge).collect();
    let gaps: Vec<[Pt; 4]> = cut_w.iter().flatten().map(|c| c.gap).collect();
    let is_ramp: Vec<bool> = cut_w.iter().map(|c| c.is_some()).collect();
    let cut: Vec<Option<Vec<Pt>>> = cut_w.into_iter().map(|c| c.map(|c| c.line)).collect();
    let trimmed: Vec<RoadDraw> = roads
        .iter()
        .zip(&cut)
        .map(|(r, c)| RoadDraw {
            class: r.class,
            points: c.as_deref().unwrap_or(r.points),
            works: r.works,
        })
        .collect();
    let context_cut: Vec<Option<Vec<Pt>>> = context
        .iter()
        .map(|r| crate::roadpaint::ramp_cut(r, &highways).map(|c| c.line))
        .collect();
    let context_trimmed: Vec<RoadDraw> = context
        .iter()
        .zip(&context_cut)
        .map(|(r, c)| RoadDraw {
            class: r.class,
            points: c.as_deref().unwrap_or(r.points),
            works: r.works,
        })
        .collect();
    let roads = &trimmed[..];
    let context = &context_trimmed[..];
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
    let hw_of = |i: usize| {
        if is_ramp[i] {
            crate::roadpaint::RAMP_HW as f32
        } else {
            roads[i].class.half_width() as f32
        }
    };
    for &i in &order {
        let r = &roads[i];
        g.ribbon(r.points, hw_of(i) + 0.5, 0.0, layer::EDGE, EDGE);
    }
    for w in &wedges {
        g.polygon(&Ground::grow(&w[..], 0.5), layer::EDGE, EDGE);
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
        g.ribbon(r.points, hw_of(i), 0.0, layer::ROAD, col);
    }
    for w in &wedges {
        g.polygon(&w[..], layer::ROAD, ASPHALT);
    }
    for &i in &order {
        let r = &roads[i];
        let hw = hw_of(i);
        if let Some(p) = r.works {
            road_works(g, r, hw, p);
        } else {
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
    crate::roadpaint::paint(g, roads, context);
    // the highway's edge line stops where a slip lane joins it
    for gap in &gaps {
        g.polygon(&gap[..], crate::roadpaint::GAP_Z, ASPHALT_HWY);
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

/// A copy of the model scaled about its own origin: a young tree or a prop that is still
/// growing in. `k` is 0 to 1.
pub fn grown(obj: &Obj, k: f32) -> Obj {
    let k = k.clamp(0.05, 1.0);
    let mut o = obj.clone();
    for p in &mut o.polys {
        for v in &mut p.pts {
            *v = [v[0] * k, v[1] * k, v[2] * k];
        }
    }
    for s in &mut o.solids {
        s.centre = [s.centre[0] * k, s.centre[1] * k, s.centre[2] * k];
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
