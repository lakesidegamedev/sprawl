//! Town to meshes: ground layers, roads, and the buildings merged into chunk meshes.

use crate::ground::{Ground, layer};
use crate::kit;
use crate::mesh::Mesh;
use crate::obj::{Obj, Rgb};
use crate::rng::Rng;
use glam::Vec3;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use us_gen::{LotKind, Pattern, Pt, RoadClass, Town, Zone, pt};

pub const GRASS: Rgb = [116, 172, 92];
pub const PAVING_CORE: Rgb = [206, 202, 194];
pub const PAVING_INDUSTRY: Rgb = [194, 192, 202];
pub const PAVING_MALL: Rgb = [208, 200, 198];
pub const PARK_GREEN: Rgb = [96, 176, 84];
pub const CAMPUS: Rgb = [196, 210, 160];
pub const CEMETERY: Rgb = [146, 178, 118];
pub const FIELDS: [Rgb; 6] = [
    [176, 198, 106],
    [198, 186, 124],
    [140, 172, 96],
    [168, 190, 110],
    [206, 194, 140],
    [128, 164, 88],
];
pub const WATER: Rgb = [66, 132, 200];
pub const SAND: Rgb = [214, 200, 150];
pub const ASPHALT: Rgb = [108, 108, 120];
pub const ASPHALT_HWY: Rgb = [96, 96, 108];
pub const EDGE: Rgb = [44, 46, 56];
pub const YELLOW: Rgb = [226, 196, 70];
pub const BALLAST: Rgb = [128, 104, 84];
pub const STEEL: Rgb = [66, 68, 78];

const CHUNK: f32 = 256.0;

static NEXT_UID: AtomicU64 = AtomicU64::new(1);

pub struct Chunk {
    pub mesh: Mesh,
    pub lo: Vec3,
    pub hi: Vec3,
}

pub struct Scene {
    /// Unique per built scene: the renderer caches GPU buffers by it.
    pub uid: u64,
    pub size: f32,
    pub ground: Mesh,
    pub chunks: Vec<Chunk>,
    pub hall: Vec3,
}

impl Scene {
    pub fn triangles(&self) -> usize {
        self.ground.triangles()
            + self
                .chunks
                .iter()
                .map(|c| c.mesh.triangles())
                .sum::<usize>()
    }

    pub fn from_town(t: &Town) -> Scene {
        let ground = build_ground(t);
        let chunks = build_chunks(t);
        Scene {
            uid: NEXT_UID.fetch_add(1, Ordering::Relaxed),
            size: t.size as f32,
            ground: ground.mesh,
            chunks,
            hall: Vec3::new(t.hall.x as f32, t.hall.y as f32, 0.0),
        }
    }
}

fn hash2(a: u64, b: u64) -> u64 {
    let mut r = Rng::new(a, b);
    r.next_u64()
}

fn zone_colour(zone: Zone, pattern: Pattern, seed: u64, ai: usize) -> Option<Rgb> {
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
        Zone::Farm => Some(FIELDS[(hash2(seed, ai as u64) % FIELDS.len() as u64) as usize]),
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

fn build_ground(t: &Town) -> Ground {
    let mut g = Ground::default();
    let s = t.size as f32;
    g.rect(-1500.0, -1500.0, s + 1500.0, s + 1500.0, layer::BASE, GRASS);
    for (ai, a) in t.additions.iter().enumerate() {
        if let Some(c) = zone_colour(a.zone, a.pattern, t.seed, ai) {
            g.polygon(&a.polygon, layer::ZONE, c);
        }
    }
    for w in &t.water {
        g.polygon(&Ground::grow(w, 14.0), layer::SAND, SAND);
        g.polygon(w, layer::WATER, WATER);
    }
    let mut order: Vec<usize> = (0..t.roads.len())
        .filter(|&i| t.roads[i].class != RoadClass::Rail)
        .collect();
    order.sort_by_key(|&i| (road_rank(t.roads[i].class), i));
    // dark edges first, then every surface over them
    for &i in &order {
        let r = &t.roads[i];
        g.ribbon(
            &r.points,
            r.class.half_width() as f32 + 0.5,
            0.0,
            layer::EDGE,
            EDGE,
        );
    }
    for c in &t.cul_de_sacs {
        g.disc(c.pos, 9.5, layer::EDGE, EDGE);
    }
    let mut culs_drawn = false;
    for &i in &order {
        let r = &t.roads[i];
        if !culs_drawn && road_rank(r.class) > road_rank(RoadClass::Local) {
            for c in &t.cul_de_sacs {
                g.disc(c.pos, 9.0, layer::ROAD, ASPHALT);
            }
            culs_drawn = true;
        }
        let col = if r.class == RoadClass::Highway {
            ASPHALT_HWY
        } else {
            ASPHALT
        };
        g.ribbon(
            &r.points,
            r.class.half_width() as f32,
            0.0,
            layer::ROAD,
            col,
        );
    }
    if !culs_drawn {
        for c in &t.cul_de_sacs {
            g.disc(c.pos, 9.0, layer::ROAD, ASPHALT);
        }
    }
    for &i in &order {
        let r = &t.roads[i];
        if matches!(r.class, RoadClass::Arterial | RoadClass::Highway) {
            g.ribbon(&r.points, 0.22, 0.0, layer::MARK, YELLOW);
        }
    }
    for r in t.roads.iter().filter(|r| r.class == RoadClass::Rail) {
        g.ribbon(&r.points, 2.6, 0.0, layer::RAIL, BALLAST);
        g.ribbon(&r.points, 0.2, 0.9, layer::RAIL + 0.06, STEEL);
        g.ribbon(&r.points, 0.2, -0.9, layer::RAIL + 0.06, STEEL);
    }
    g
}

/// Uniform grid over road segments, for "how far is this spot from any road".
type SegCells = BTreeMap<(i32, i32), Vec<(Pt, Pt, f64)>>;

struct RoadGrid {
    cells: SegCells,
}

impl RoadGrid {
    fn new(t: &Town) -> RoadGrid {
        let mut cells: SegCells = BTreeMap::new();
        for r in &t.roads {
            let hw = r.class.half_width();
            for w in r.points.windows(2) {
                let n = ((w[0].dist(w[1]) / 16.0).ceil() as usize).max(1);
                let mut last = (i32::MIN, i32::MIN);
                for k in 0..=n {
                    let p = w[0].lerp(w[1], k as f64 / n as f64);
                    let c = ((p.x / 48.0).floor() as i32, (p.y / 48.0).floor() as i32);
                    if c != last {
                        cells.entry(c).or_default().push((w[0], w[1], hw));
                        last = c;
                    }
                }
            }
        }
        RoadGrid { cells }
    }

    /// Smallest gap between `p` and the edge of any road, within 2 cells.
    fn clearance(&self, p: Pt) -> f64 {
        let (cx, cy) = ((p.x / 48.0).floor() as i32, (p.y / 48.0).floor() as i32);
        let mut best = f64::INFINITY;
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for (a, b, hw) in self.cells.get(&(x, y)).map_or(&[][..], |v| v.as_slice()) {
                    let (abx, aby) = (b.x - a.x, b.y - a.y);
                    let l2 = abx * abx + aby * aby;
                    let tt = if l2 < 1e-12 {
                        0.0
                    } else {
                        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
                    };
                    let d = p.dist(pt(a.x + abx * tt, a.y + aby * tt)) - hw;
                    best = best.min(d);
                }
            }
        }
        best
    }
}

fn in_ring(p: Pt, ring: &[Pt]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn trees_for(kind: LotKind, rng: &mut Rng) -> usize {
    match kind {
        LotKind::House => rng.below(4),
        LotKind::Row | LotKind::Apartments | LotKind::Shop | LotKind::FastFood => rng.below(2),
        LotKind::Church => 1 + rng.below(3),
        LotKind::School => 2 + rng.below(3),
        LotKind::Sports => 3,
        LotKind::Farm => 5 + rng.below(5),
        LotKind::Workshop | LotKind::Warehouse => rng.below(2),
        _ => 0,
    }
}

struct ChunkBuild {
    mesh: Mesh,
    lo: Vec3,
    hi: Vec3,
}

fn build_chunks(t: &Town) -> Vec<Chunk> {
    let mut cells: BTreeMap<(i32, i32), ChunkBuild> = BTreeMap::new();
    let mut next_id: u32 = 1;
    let place = |obj: &Obj,
                 x: f32,
                 y: f32,
                 theta: f32,
                 cells: &mut BTreeMap<(i32, i32), ChunkBuild>,
                 next_id: &mut u32| {
        let key = ((x / CHUNK).floor() as i32, (y / CHUNK).floor() as i32);
        let cb = cells.entry(key).or_insert_with(|| ChunkBuild {
            mesh: Mesh::default(),
            lo: Vec3::splat(f32::MAX),
            hi: Vec3::splat(f32::MIN),
        });
        obj.emit(&mut cb.mesh, next_id, x, y, theta, 1.0);
        let b = obj.footprint();
        let reach = b[0].abs().max(b[1].abs()).max(b[2].abs()).max(b[3].abs()) * 1.5 + 1.0;
        cb.lo = cb.lo.min(Vec3::new(x - reach, y - reach, 0.0));
        cb.hi = cb.hi.max(Vec3::new(x + reach, y + reach, 60.0));
    };

    // town hall faces the nearest road
    let hall = pt(t.hall.x, t.hall.y);
    let mut best: Option<(f64, Pt)> = None;
    for r in &t.roads {
        for w in r.points.windows(2) {
            let (abx, aby) = (w[1].x - w[0].x, w[1].y - w[0].y);
            let l2 = abx * abx + aby * aby;
            if l2 < 1e-12 {
                continue;
            }
            let tt = (((hall.x - w[0].x) * abx + (hall.y - w[0].y) * aby) / l2).clamp(0.0, 1.0);
            let q = pt(w[0].x + abx * tt, w[0].y + aby * tt);
            let d = hall.dist(q);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, q));
            }
        }
    }
    let theta = best.map_or(0.0, |(_, q)| {
        libm::atan2f((q.x - hall.x) as f32, -(q.y - hall.y) as f32)
    });
    let mut hrng = Rng::new(t.seed, 0x4a11);
    place(
        &kit::town_hall(&mut hrng),
        hall.x as f32,
        hall.y as f32,
        theta,
        &mut cells,
        &mut next_id,
    );

    let green = [kit::TREE, kit::TREE_D];
    for (i, lot) in t.lots.iter().enumerate() {
        let mut rng = Rng::new(t.seed ^ 0x5eed_0000, i as u64 * 7919 + 13);
        let obj = kit::build_lot(lot.kind, &mut rng);
        let (x, y) = (lot.centre.x as f32, lot.centre.y as f32);
        let theta = lot.facing as f32;
        place(&obj, x, y, theta, &mut cells, &mut next_id);
        let n = trees_for(lot.kind, &mut rng);
        if n == 0 {
            continue;
        }
        let fp = obj.footprint();
        let (st, ct) = (libm::sinf(theta), libm::cosf(theta));
        let mut placed: Vec<[f32; 2]> = Vec::new();
        for _ in 0..n {
            for _try in 0..14 {
                let ang = rng.range(0.0, std::f32::consts::TAU);
                let r = rng.range(0.35, 1.0) * lot.radius as f32 + 0.3;
                let (lx, ly) = (r * libm::cosf(ang), r * libm::sinf(ang));
                let m = 0.4;
                let inside = lx > fp[0] - m && lx < fp[2] + m && ly > fp[1] - m && ly < fp[3] + m;
                if inside || placed.iter().any(|p| (p[0] - lx).hypot(p[1] - ly) < 3.2) {
                    continue;
                }
                placed.push([lx, ly]);
                let (rad, h) = (rng.range(1.8, 2.8), rng.range(5.0, 8.0));
                let tree = kit::tree(rad, h, rng.pick(&green));
                place(
                    &tree,
                    x + ct * lx - st * ly,
                    y + st * lx + ct * ly,
                    0.0,
                    &mut cells,
                    &mut next_id,
                );
                break;
            }
        }
    }

    // trees in open park land
    let grid = RoadGrid::new(t);
    for (ai, a) in t.additions.iter().enumerate() {
        let per = match (a.zone, a.pattern) {
            (Zone::Park, Pattern::Golf) => 1800.0,
            (Zone::Park, Pattern::Cemetery) => 900.0,
            (Zone::Park, _) => 520.0,
            (Zone::Campus, _) => 1600.0,
            _ => continue,
        };
        let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
        for p in &a.polygon {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
        let area = (hi.x - lo.x) * (hi.y - lo.y);
        let mut rng = Rng::new(t.seed ^ 0x7ee5, ai as u64);
        let tries = (area / per) as usize;
        for _ in 0..tries {
            let p = pt(
                lo.x + rng.f32() as f64 * (hi.x - lo.x),
                lo.y + rng.f32() as f64 * (hi.y - lo.y),
            );
            if !in_ring(p, &a.polygon) || grid.clearance(p) < 5.0 {
                continue;
            }
            if t.lots.iter().any(|l| l.centre.dist(p) < l.radius + 3.0) {
                continue;
            }
            let (rad, h) = (rng.range(1.8, 2.8), rng.range(5.0, 8.0));
            let tree = kit::tree(rad, h, rng.pick(&green));
            place(&tree, p.x as f32, p.y as f32, 0.0, &mut cells, &mut next_id);
        }
    }

    cells
        .into_values()
        .map(|c| Chunk {
            mesh: c.mesh,
            lo: c.lo,
            hi: c.hi,
        })
        .collect()
}
