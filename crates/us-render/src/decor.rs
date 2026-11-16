//! Everything around the buildings: rows of street trees along sidewalks, the grounds of house
//! lots (lawn bands, driveway, path, shrubs, hedges), shop and office forecourts with paving
//! bands and street furniture, and loading aprons with trucks at depots. All of it is placed
//! once from the town and its seed, so the same town always has the same trees and yards.
//! Ground marks are flat decals; the few solid pieces are small boxes, so the cost stays low and
//! the far (level of detail) mesh drops them.

use crate::instances::{self, Path};
use crate::kit;
use crate::kit_misc;
use crate::land::Land;
use crate::obj::{Obj, Rgb, V3};
use crate::rng::Rng;
use std::collections::BTreeMap;
use us_gen::{Lot, LotKind, Pt, RoadClass, Town, Zone, pt};

/// A model with where it stands: (model, x, y, turn, floor height).
pub type Placed = (Obj, f32, f32, f32, f32);

const CELL: f64 = 48.0;

fn cell(p: Pt) -> (i32, i32) {
    ((p.x / CELL).floor() as i32, (p.y / CELL).floor() as i32)
}

type SegCells = BTreeMap<(i32, i32), Vec<(u32, Pt, Pt)>>;

/// Road segments in a grid: "how far is this spot from the edge of any road".
struct Roads {
    cells: SegCells,
    hw: Vec<f64>,
    class: Vec<RoadClass>,
}

impl Roads {
    fn new(t: &Town) -> Roads {
        let mut cells: SegCells = BTreeMap::new();
        for (ri, r) in t.roads.iter().enumerate() {
            if r.class == RoadClass::Rail {
                continue;
            }
            for w in r.points.windows(2) {
                let n = ((w[0].dist(w[1]) / 16.0).ceil() as usize).max(1);
                let mut last = (i32::MIN, i32::MIN);
                for k in 0..=n {
                    let c = cell(w[0].lerp(w[1], k as f64 / n as f64));
                    if c != last {
                        cells.entry(c).or_default().push((ri as u32, w[0], w[1]));
                        last = c;
                    }
                }
            }
        }
        Roads {
            cells,
            hw: t.roads.iter().map(|r| r.class.half_width()).collect(),
            class: t.roads.iter().map(|r| r.class).collect(),
        }
    }

    /// Gap between `p` and the nearest road edge (not counting road `skip`), and that road.
    fn edge_dist(&self, p: Pt, skip: Option<usize>) -> (f64, Option<usize>) {
        let (cx, cy) = cell(p);
        let mut best = (f64::INFINITY, None);
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for (ri, a, b) in self.cells.get(&(x, y)).map_or(&[][..], |v| v.as_slice()) {
                    if Some(*ri as usize) == skip {
                        continue;
                    }
                    let (abx, aby) = (b.x - a.x, b.y - a.y);
                    let l2 = abx * abx + aby * aby;
                    let tt = if l2 < 1e-12 {
                        0.0
                    } else {
                        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
                    };
                    let d = p.dist(pt(a.x + abx * tt, a.y + aby * tt)) - self.hw[*ri as usize];
                    if d < best.0 {
                        best = (d, Some(*ri as usize));
                    }
                }
            }
        }
        best
    }
}

/// Lot circles (plus the hall and stations) in a grid.
struct Circles {
    cells: BTreeMap<(i32, i32), Vec<(Pt, f64)>>,
}

impl Circles {
    fn new(t: &Town) -> Circles {
        let mut cells: BTreeMap<(i32, i32), Vec<(Pt, f64)>> = BTreeMap::new();
        let mut add = |c: Pt, r: f64| cells.entry(cell(c)).or_default().push((c, r));
        for l in &t.lots {
            add(l.centre, l.radius);
        }
        add(pt(t.hall.x, t.hall.y), 18.0);
        for s in &t.stations {
            add(s.pos, 16.0);
        }
        Circles { cells }
    }

    fn blocked(&self, p: Pt, margin: f64) -> bool {
        let (cx, cy) = cell(p);
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for (c, r) in self.cells.get(&(x, y)).map_or(&[][..], |v| v.as_slice()) {
                    if c.dist(p) < r + margin {
                        return true;
                    }
                }
            }
        }
        false
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

/// Downtown paving areas with their bounding boxes, for "is this spot downtown".
struct Core {
    areas: Vec<(Pt, Pt, Vec<Pt>)>,
}

impl Core {
    fn new(t: &Town) -> Core {
        let mut areas = Vec::new();
        for a in t.additions.iter().filter(|a| a.zone == Zone::Core) {
            let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
            for p in &a.polygon {
                lo = pt(lo.x.min(p.x), lo.y.min(p.y));
                hi = pt(hi.x.max(p.x), hi.y.max(p.y));
            }
            areas.push((lo, hi, a.polygon.clone()));
        }
        Core { areas }
    }

    fn has(&self, p: Pt) -> bool {
        self.areas.iter().any(|(lo, hi, ring)| {
            p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y && in_ring(p, ring)
        })
    }
}

fn quad(o: &mut Obj, x0: f32, y0: f32, x1: f32, y1: f32, z: f32, col: Rgb) {
    let pts: [V3; 4] = [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]];
    o.poly(&pts, col, false, false, true);
}

fn shade(c: Rgb, k: f32) -> Rgb {
    [
        (c[0] as f32 * k).min(255.0) as u8,
        (c[1] as f32 * k).min(255.0) as u8,
        (c[2] as f32 * k).min(255.0) as u8,
    ]
}

const SHRUB_A: Rgb = [66, 122, 64];
const SHRUB_B: Rgb = [84, 140, 70];
const LAWN_A: Rgb = [104, 160, 84];
const CONC_L: Rgb = [200, 196, 188];
const ASPH: Rgb = [100, 102, 110];
const SOIL: Rgb = [118, 88, 62];

// ---- street trees ----

/// Cumulative lengths of a polyline.
fn lengths(line: &[Pt]) -> Vec<f64> {
    let mut acc = vec![0.0];
    for w in line.windows(2) {
        acc.push(acc.last().unwrap() + w[0].dist(w[1]));
    }
    acc
}

/// Point and unit tangent at arclength `s`.
fn along(line: &[Pt], acc: &[f64], s: f64) -> (Pt, Pt) {
    let i = acc.partition_point(|&a| a <= s).clamp(1, line.len() - 1);
    let (a, b) = (line[i - 1], line[i]);
    let l = (acc[i] - acc[i - 1]).max(1e-9);
    let d = pt((b.x - a.x) / l, (b.y - a.y) / l);
    (a.lerp(b, ((s - acc[i - 1]) / l).clamp(0.0, 1.0)), d)
}

fn street_tree(core: bool, rng: &mut Rng, x: f32, y: f32) -> Obj {
    let green = [kit::TREE, kit::TREE_D];
    let col = kit::tree_variant(rng.pick(&green), x, y);
    if core {
        let mut o = kit::tree(rng.range(1.3, 1.7), rng.range(4.6, 5.8), col);
        // a tree pit: a dark grate square set in the sidewalk
        quad(&mut o, -0.6, -0.6, 0.6, 0.6, 0.27, [96, 94, 92]);
        quad(&mut o, -0.45, -0.45, 0.45, 0.45, 0.28, [120, 98, 76]);
        o
    } else {
        let roll = rng.f32() * 0.84;
        crate::kit_more::tree_species(roll, rng.range(1.6, 2.2), rng.range(5.0, 6.8), col)
    }
}

#[allow(clippy::too_many_arguments)]
fn street_trees(
    t: &Town,
    land: &Land,
    roads: &Roads,
    circles: &Circles,
    core: &Core,
    drives: &[Pt],
    out: &mut Vec<Placed>,
) {
    let paths: Vec<Path> = t
        .roads
        .iter()
        .map(|r| Path::new(r.class, &r.points))
        .collect();
    let lamps: Vec<[f32; 2]> = instances::lamp_instances(&paths)
        .iter()
        .map(|l| [l.pos[0], l.pos[1]])
        .collect();
    for (ri, r) in t.roads.iter().enumerate() {
        if !matches!(r.class, RoadClass::Local | RoadClass::Collector) || r.points.len() < 2 {
            continue;
        }
        let hw = r.class.half_width();
        let ring = crate::roadpaint::ring_arc(&crate::live::RoadDraw {
            class: r.class,
            points: &r.points,
            works: None,
        });
        let acc = lengths(&r.points);
        let total = *acc.last().unwrap();
        for side in [-1.0f64, 1.0] {
            if let Some((_, ccw)) = ring
                && (side > 0.0) == ccw
            {
                continue;
            }
            let mut rng = Rng::new(t.seed ^ 0x57ee_7a11, ri as u64 * 2 + (side > 0.0) as u64);
            let mut s = rng.range(5.0, 11.0) as f64;
            while s < total - 6.0 {
                let (p, d) = along(&r.points, &acc, s);
                let n = pt(-d.y * side, d.x * side);
                let at_core = core.has(p);
                let (off, step) = if at_core { (1.6, 10.5) } else { (3.1, 11.5) };
                s += step + rng.range(-1.0, 1.0) as f64;
                let q = pt(p.x + n.x * (hw + off), p.y + n.y * (hw + off));
                let blocked = roads.edge_dist(q, Some(ri)).0 < 5.0
                    || roads.edge_dist(q, None).0 < 2.6
                    || circles.blocked(q, -0.8)
                    || land.water_at(q).is_some()
                    || drives.iter().any(|m| m.dist(q) < 3.4)
                    || lamps
                        .iter()
                        .any(|l| (l[0] as f64 - q.x).hypot(l[1] as f64 - q.y) < 2.8);
                if blocked {
                    continue;
                }
                let (x, y) = (q.x as f32, q.y as f32);
                out.push((
                    street_tree(at_core, &mut rng, x, y),
                    x,
                    y,
                    0.0,
                    land.height(x, y),
                ));
            }
        }
    }
}

// ---- lots ----

/// How far the sidewalk's outer edge is along a lot's front axis, and whether the road has one.
fn front_reach(roads: &Roads, lot: &Lot) -> Option<(f32, bool)> {
    let (st, ct) = (libm::sin(lot.facing), libm::cos(lot.facing));
    for k in 0..64 {
        let t = k as f64 * 0.5;
        if t < lot.radius * 0.6 {
            continue;
        }
        let p = pt(lot.centre.x + st * t, lot.centre.y - ct * t);
        if let (d, Some(ri)) = roads.edge_dist(p, None) {
            let walk = matches!(roads.class[ri], RoadClass::Local | RoadClass::Collector);
            if d <= if walk { 2.5 } else { 0.3 } {
                return Some((t as f32, walk));
            }
        }
    }
    None
}

/// Box around everything tall in a model: where a yard must not put things.
fn occupied(o: &Obj) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for p in &o.polys {
        if p.solid.is_none() || p.pts.iter().fold(0.0f32, |a, v| a.max(v[2])) < 1.5 {
            continue;
        }
        for v in &p.pts {
            b[0] = b[0].min(v[0]);
            b[1] = b[1].min(v[1]);
            b[2] = b[2].max(v[0]);
            b[3] = b[3].max(v[1]);
        }
    }
    if b[0] > b[2] {
        [-4.0, -4.0, 4.0, 4.0]
    } else {
        b
    }
}

/// The x of the house's own front walk (a 1 m wide slab) and where it ends, if it has one.
fn own_walk(o: &Obj) -> Option<(f32, f32)> {
    for p in &o.polys {
        if p.pts.len() != 4 || p.pts.iter().any(|v| (v[2] - 0.05).abs() > 0.011) {
            continue;
        }
        let (x0, x1) = p
            .pts
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v[0]), b.max(v[0])));
        let y0 = p.pts.iter().fold(f32::MAX, |a, v| a.min(v[1]));
        if (x1 - x0 - 1.0).abs() < 0.02 && y0 < -2.0 {
            return Some(((x0 + x1) / 2.0, y0));
        }
    }
    None
}

fn shrub(o: &mut Obj, x: f32, y: f32, s: f32, rng: &mut Rng) {
    let c = if rng.chance(0.5) { SHRUB_A } else { SHRUB_B };
    o.bx_top(x - s, y - s, 0.0, x + s, y + s, s * 1.5, c, shade(c, 1.12));
}

fn car_at(o: &mut Obj, x: f32, y: f32, turn: f32, rng: &mut Rng) {
    o.add_placed(&kit_misc::car(rng), x, y, turn);
}

struct Ctx<'a> {
    roads: &'a Roads,
    core: &'a Core,
}

fn house_yard(lot: &Lot, i: usize, obj: &Obj, cx: &Ctx, drives: &mut Vec<Pt>) -> Option<Obj> {
    let mut rng = Rng::new(0x9a2d_0000 ^ (lot.variant * 1e9) as u64, i as u64 * 31 + 7);
    let r = lot.radius as f32;
    let occ = occupied(obj);
    let walk = own_walk(obj);
    let reach = front_reach(cx.roads, lot);
    let mut o = Obj::new();
    let zl = 0.04 + 0.002 * (i % 5) as f32;
    // front lawn: a plot in two greens, in bands across the lot
    if let Some((l, _)) = reach {
        let (xa, xb) = (-r * 0.95, r * 0.95);
        let (ya, yb) = (-(l - 0.15), occ[1] - 0.2);
        if yb - ya > 1.0 {
            let n = ((xb - xa) / 2.0).ceil() as usize;
            for k in (0..n).step_by(2) {
                let x0 = xa + k as f32 * 2.0;
                quad(&mut o, x0, ya, (x0 + 2.0).min(xb), yb, zl, LAWN_A);
            }
        }
    }
    // driveway to the street, beside the house, with a dropped kerb over the sidewalk
    let wide = (occ[2] - occ[0]).max(1.0);
    let side = if occ[2].abs() > occ[0].abs() + 1.0 {
        1.0
    } else if occ[0].abs() > occ[2].abs() + 1.0 {
        -1.0
    } else if lot.variant < 0.5 {
        1.0
    } else {
        -1.0
    };
    let ext = if side > 0.0 { occ[2] } else { -occ[0] };
    let annex = ext > wide / 2.0 + 1.0;
    let cxd = if annex {
        side * (ext - 1.6)
    } else {
        side * (ext + 1.6)
    };
    let mut drive_x = None;
    if let Some((l, walkable)) = reach
        && cxd.abs() + 1.4 <= r * 0.98
        && l > 4.0
        && (annex || rng.chance(0.75))
    {
        let col = if rng.chance(0.5) { CONC_L } else { ASPH };
        let (xa, xb) = (cxd - 1.4, cxd + 1.4);
        quad(&mut o, xa, -l, xb, occ[1] + 0.2, 0.09, col);
        if walkable {
            quad(&mut o, xa, -l - 2.0, xb, -l + 0.2, 0.285, col);
        }
        drive_x = Some(cxd);
        let (st, ct) = (libm::sin(lot.facing) as f64, libm::cos(lot.facing) as f64);
        let (lx, ly) = (cxd as f64, -(l as f64) - 1.0);
        drives.push(pt(
            lot.centre.x + ct * lx - st * ly,
            lot.centre.y + st * lx + ct * ly,
        ));
        if (l - occ[1].abs()) > 5.0 && rng.chance(0.5) {
            let yc = occ[1] - 2.8;
            car_at(&mut o, cxd, yc, std::f32::consts::FRAC_PI_2, &mut rng);
        }
    }
    // the front walk reaches the sidewalk
    if let (Some((wx, wy)), Some((l, _))) = (walk, reach)
        && (drive_x.is_none_or(|d| (d - wx).abs() > 2.6))
    {
        quad(&mut o, wx - 0.5, -l, wx + 0.5, wy + 0.1, 0.09, CONC_L);
    }
    // shrubs at the foot of the walls and the house corners
    if rng.chance(0.8) {
        let n = 2 + rng.below(2);
        for k in 0..n {
            let y = occ[1] + 0.8 + k as f32 * 2.1;
            if y > occ[3] - 0.8 {
                break;
            }
            let x = if k % 2 == 0 {
                occ[0] - 0.6
            } else {
                occ[2] + 0.6
            };
            if drive_x.is_some_and(|d| (d - x).abs() < 2.0) {
                continue;
            }
            shrub(&mut o, x, y, rng.range(0.38, 0.55), &mut rng);
        }
        for x in [occ[0] + 0.6, occ[2] - 0.6] {
            if walk.is_some_and(|w| (w.0 - x).abs() < 1.8)
                || drive_x.is_some_and(|d| (d - x).abs() < 2.0)
            {
                continue;
            }
            shrub(&mut o, x, occ[1] - 0.45, 0.4, &mut rng);
        }
    }
    // a hedge on one side line
    if rng.chance(0.4) {
        let sx = if drive_x.is_some_and(|d| d > 0.0) || rng.chance(0.5) {
            -r * 0.93
        } else {
            r * 0.93
        };
        if drive_x.is_none_or(|d| (d - sx).abs() > 2.5) {
            o.bx_top(
                sx - 0.3,
                -r * 0.25,
                0.0,
                sx + 0.3,
                r * 0.8,
                0.9,
                crate::parts::GREEN,
                [104, 160, 84],
            );
        }
    }
    // a garden bed along the back when there is room
    if r * 0.96 - occ[3] > 1.5 && rng.chance(0.55) {
        let x0 = rng.range(-3.0, 0.0);
        let y0 = occ[3] + 0.5;
        quad(&mut o, x0, y0, x0 + 3.2, y0 + 1.0, 0.1, SOIL);
        let c = rng.pick(&[
            [226, 120, 150],
            [236, 190, 70],
            [150, 110, 200],
            [240, 240, 232],
        ]);
        for k in 0..4 {
            let px = x0 + 0.4 + k as f32 * 0.8;
            o.bx(px - 0.18, y0 + 0.3, 0.1, px + 0.18, y0 + 0.66, 0.5, c);
        }
    }
    // a lantern on a post by the walk: lit at night
    if let (Some((wx, wy)), true) = (walk, rng.chance(0.4)) {
        let lx = wx + 0.9;
        let ly = wy - 0.3;
        o.bx(
            lx - 0.05,
            ly - 0.05,
            0.0,
            lx + 0.05,
            ly + 0.05,
            1.0,
            [60, 62, 70],
        );
        o.bx(
            lx - 0.2,
            ly - 0.2,
            1.0,
            lx + 0.2,
            ly + 0.2,
            1.4,
            [60, 62, 70],
        );
        let lens: [V3; 4] = [
            [lx - 0.2, ly - 0.21, 1.1],
            [lx + 0.2, ly - 0.21, 1.1],
            [lx + 0.2, ly - 0.21, 1.35],
            [lx - 0.2, ly - 0.21, 1.35],
        ];
        o.poly(&lens, instances::LAMP_LENS, false, false, true);
        let pool: Vec<V3> = (0..10)
            .map(|k| {
                let a = (k as f32 + 0.5) * std::f32::consts::TAU / 10.0;
                [lx + 1.3 * libm::cosf(a), ly + 1.3 * libm::sinf(a), 0.11]
            })
            .collect();
        o.poly(&pool, instances::LAMP_POOL, false, false, true);
    }
    let _ = cx.core;
    (!o.polys.is_empty()).then_some(o)
}

/// A delivery truck about 8 m long facing +x.
fn truck(rng: &mut Rng) -> Obj {
    let mut o = Obj::new();
    let col = rng.pick(&[
        [226u8, 226, 222],
        [196, 58, 52],
        [48, 84, 150],
        [230, 190, 60],
        [60, 130, 96],
    ]);
    o.bx(-3.8, -1.2, 0.5, 1.9, 1.2, 3.1, col);
    o.bx_top(2.0, -1.15, 0.5, 3.8, 1.15, 2.3, shade(col, 0.85), col);
    o.bx(3.2, -1.0, 1.5, 3.82, 1.0, 2.2, [70, 96, 120]);
    o.bx(-3.7, -1.0, 0.2, 3.7, 1.0, 0.5, [50, 52, 58]);
    o
}

fn forecourt(lot: &Lot, i: usize, obj: &Obj, cx: &Ctx) -> Option<Obj> {
    let mut rng = Rng::new(0xf0c0_0000 ^ (lot.variant * 1e9) as u64, i as u64 * 17 + 3);
    let (l, walkable) = front_reach(cx.roads, lot)?;
    let fp = obj.footprint();
    let front = fp[1].min(0.0);
    if l + front < 1.2 {
        return None;
    }
    let r = lot.radius as f32;
    let w2 = ((fp[2] - fp[0]) / 2.0 + 1.5).min(r * 0.95);
    let xc = ((fp[0] + fp[2]) / 2.0).clamp(-r * 0.3, r * 0.3);
    let (xa, xb) = ((xc - w2).max(-r * 0.95), (xc + w2).min(r * 0.95));
    let (ya, yb) = (-(l - 0.1), front + 0.1);
    let downtown = cx.core.has(lot.centre);
    let industrial = matches!(
        lot.kind,
        LotKind::Workshop | LotKind::Warehouse | LotKind::Gas
    );
    let (base, tile) = if industrial {
        rng.pick(&[
            ([170u8, 170, 176], [156u8, 156, 164]),
            ([178, 176, 170], [164, 162, 156]),
        ])
    } else if downtown {
        rng.pick(&[
            ([212u8, 206, 194], [196u8, 190, 178]),
            ([206, 200, 200], [188, 184, 186]),
            ([214, 196, 170], [200, 178, 150]),
        ])
    } else {
        rng.pick(&[
            ([200u8, 196, 188], [186u8, 182, 174]),
            ([208, 190, 168], [194, 172, 148]),
        ])
    };
    let mut o = Obj::new();
    quad(&mut o, xa, ya, xb, yb, 0.1, base);
    let cell_sz = if industrial { 3.6 } else { 3.0 };
    let (nx, ny) = (
        ((xb - xa) / cell_sz).ceil() as usize,
        ((yb - ya) / cell_sz).ceil() as usize,
    );
    for ix in 0..nx.min(12) {
        for iy in 0..ny.min(8) {
            if (ix + iy) % 2 == 0 {
                continue;
            }
            let (x0, y0) = (xa + ix as f32 * cell_sz, ya + iy as f32 * cell_sz);
            quad(
                &mut o,
                x0,
                y0,
                (x0 + cell_sz).min(xb),
                (y0 + cell_sz).min(yb),
                0.11,
                tile,
            );
        }
    }
    match lot.kind {
        LotKind::Warehouse | LotKind::Workshop => {
            // yellow bay lines and a truck backed up to the dock
            let bays = ((xb - xa) / 5.0).floor().max(1.0) as usize;
            for k in 0..=bays {
                let x = xa + k as f32 * (xb - xa) / bays as f32;
                quad(
                    &mut o,
                    x - 0.1,
                    front - 9.0,
                    x + 0.1,
                    front - 0.2,
                    0.12,
                    [236, 200, 60],
                );
            }
            if yb - ya > 6.0 && rng.chance(0.7) {
                let n = 1 + rng.below(2);
                for k in 0..n {
                    let x = xa + (k as f32 + 0.5 + rng.range(-0.1, 0.1)) * (xb - xa) / n as f32;
                    let tr = truck(&mut rng);
                    o.add_placed(&tr, x, front - 4.6, -std::f32::consts::FRAC_PI_2);
                }
            }
        }
        LotKind::Shop | LotKind::FastFood | LotKind::Office | LotKind::Apartments => {
            let street_y = ya + 0.7;
            // planters along the street edge
            for x in [xa + 0.9, xb - 0.9] {
                if (xb - xa) > 5.0 {
                    o.bx_top(
                        x - 0.6,
                        street_y - 0.35,
                        0.0,
                        x + 0.6,
                        street_y + 0.35,
                        0.5,
                        [150, 130, 108],
                        SHRUB_A,
                    );
                    o.bx(
                        x - 0.3,
                        street_y - 0.2,
                        0.5,
                        x + 0.3,
                        street_y + 0.2,
                        0.95,
                        SHRUB_B,
                    );
                }
            }
            if rng.chance(0.6) && (xb - xa) > 6.0 {
                // a bench facing the building and a bin
                let bx = xc + rng.range(-1.5, 1.5);
                let by = street_y + 1.2;
                o.bx(
                    bx - 0.9,
                    by - 0.25,
                    0.4,
                    bx + 0.9,
                    by + 0.25,
                    0.55,
                    [150, 104, 70],
                );
                o.bx(
                    bx - 0.9,
                    by + 0.2,
                    0.55,
                    bx + 0.9,
                    by + 0.28,
                    1.0,
                    [150, 104, 70],
                );
                o.bx(
                    bx - 0.8,
                    by - 0.2,
                    0.0,
                    bx - 0.65,
                    by + 0.2,
                    0.4,
                    [60, 62, 70],
                );
                o.bx(
                    bx + 0.65,
                    by - 0.2,
                    0.0,
                    bx + 0.8,
                    by + 0.2,
                    0.4,
                    [60, 62, 70],
                );
                let bin_x = xb - 0.9;
                o.bx(
                    bin_x - 0.25,
                    by - 0.25,
                    0.0,
                    bin_x + 0.25,
                    by + 0.25,
                    0.9,
                    [70, 120, 96],
                );
            }
            if matches!(lot.kind, LotKind::Shop | LotKind::FastFood) && rng.chance(0.55) {
                // cafe tables with umbrellas against the shop front
                let n = 1 + rng.below(2);
                let umb = rng.pick(&[
                    [196u8, 58, 52],
                    [226, 170, 50],
                    [48, 120, 170],
                    [60, 140, 100],
                ]);
                for k in 0..n {
                    let tx = xc + (k as f32 * 2.0 - (n as f32 - 1.0)) * 1.3 + rng.range(-0.4, 0.4);
                    let ty = front - 1.6;
                    o.prism(
                        tx,
                        ty,
                        0.0,
                        0.75,
                        0.5,
                        [60, 62, 70],
                        6,
                        Some([230, 226, 214]),
                    );
                    for (dx, dy) in [(-0.8f32, 0.0f32), (0.8, 0.0)] {
                        o.bx(
                            tx + dx - 0.22,
                            ty + dy - 0.22,
                            0.0,
                            tx + dx + 0.22,
                            ty + dy + 0.22,
                            0.5,
                            [60, 62, 70],
                        );
                    }
                    o.bx(
                        tx - 0.04,
                        ty - 0.04,
                        0.0,
                        tx + 0.04,
                        ty + 0.04,
                        2.2,
                        [150, 150, 150],
                    );
                    o.pyramid(tx, ty, 2.1, 1.4, 0.5, umb, 6, 0.0);
                }
            } else if rng.chance(0.4) && (xb - xa) > 6.0 {
                // a bike rack
                let rx = xc + rng.range(-2.0, 2.0);
                for k in 0..3 {
                    let x = rx + k as f32 * 0.5;
                    o.bx(
                        x - 0.03,
                        front - 2.6,
                        0.0,
                        x + 0.03,
                        front - 2.1,
                        0.8,
                        [120, 124, 132],
                    );
                }
            }
        }
        _ => {}
    }
    let _ = walkable;
    Some(o)
}

/// Paving bands on downtown ground: a checker of two close tones in 7 m cells wherever a whole
/// cell lies inside a downtown area. Crisp flat shapes, no texture.
pub(crate) fn paving(g: &mut crate::ground::Ground, t: &Town) {
    const SZ: f64 = 7.0;
    let dark: Rgb = [194, 190, 182];
    for a in t.additions.iter().filter(|a| a.zone == Zone::Core) {
        let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
        for p in &a.polygon {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
        let (ix0, ix1) = ((lo.x / SZ).floor() as i64, (hi.x / SZ).ceil() as i64);
        let (iy0, iy1) = ((lo.y / SZ).floor() as i64, (hi.y / SZ).ceil() as i64);
        for ix in ix0..ix1 {
            for iy in iy0..iy1 {
                if (ix + iy) & 1 == 0 {
                    continue;
                }
                let (x0, y0) = (ix as f64 * SZ, iy as f64 * SZ);
                let c = [
                    pt(x0, y0),
                    pt(x0 + SZ, y0),
                    pt(x0 + SZ, y0 + SZ),
                    pt(x0, y0 + SZ),
                ];
                if c.iter().all(|p| in_ring(*p, &a.polygon)) {
                    g.polygon(&c, crate::ground::layer::ZONE + 0.01, dark);
                }
            }
        }
    }
}

/// Planter trees and benches in open downtown ground, on a loose 15 m grid.
fn plaza(t: &Town, land: &Land, roads: &Roads, circles: &Circles, out: &mut Vec<Placed>) {
    const SZ: f64 = 15.0;
    for (ai, a) in t
        .additions
        .iter()
        .enumerate()
        .filter(|(_, a)| a.zone == Zone::Core)
    {
        let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
        for p in &a.polygon {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
        let mut rng = Rng::new(t.seed ^ 0x91a2_a000, ai as u64);
        let (ix0, ix1) = ((lo.x / SZ).floor() as i64, (hi.x / SZ).ceil() as i64);
        let (iy0, iy1) = ((lo.y / SZ).floor() as i64, (hi.y / SZ).ceil() as i64);
        for ix in ix0..ix1 {
            for iy in iy0..iy1 {
                let q = pt(
                    (ix as f64 + 0.5) * SZ + rng.range(-3.0, 3.0) as f64,
                    (iy as f64 + 0.5) * SZ + rng.range(-3.0, 3.0) as f64,
                );
                let pick = rng.f32();
                if pick > 0.7
                    || !in_ring(q, &a.polygon)
                    || roads.edge_dist(q, None).0 < 4.5
                    || circles.blocked(q, 2.0)
                    || land.water_at(q).is_some()
                {
                    continue;
                }
                let (x, y) = (q.x as f32, q.y as f32);
                let mut o = Obj::new();
                if pick < 0.45 {
                    let tr = street_tree(true, &mut rng, x, y);
                    o.add_placed(&tr, 0.0, 0.0, 0.0);
                    o.bx_top(-1.1, -1.1, 0.0, 1.1, 1.1, 0.45, [150, 130, 108], SOIL);
                } else {
                    // a bench pair with a bin
                    for dy in [-1.4f32, 1.4] {
                        o.bx(-0.9, dy - 0.25, 0.4, 0.9, dy + 0.25, 0.55, [150, 104, 70]);
                        o.bx(-0.8, dy - 0.2, 0.0, -0.65, dy + 0.2, 0.4, [60, 62, 70]);
                        o.bx(0.65, dy - 0.2, 0.0, 0.8, dy + 0.2, 0.4, [60, 62, 70]);
                    }
                    o.bx(1.5, -0.25, 0.0, 2.0, 0.25, 0.9, [70, 120, 96]);
                }
                let turn = rng.pick(&[0.0f32, std::f32::consts::FRAC_PI_2]);
                out.push((o, x, y, turn, land.height(x, y)));
            }
        }
    }
}

/// Trees along the streets and the grounds of the lots. `lots` are the lot models as built.
pub(crate) fn build(t: &Town, land: &Land, lots: &[&Obj]) -> Vec<Placed> {
    let roads = Roads::new(t);
    let circles = Circles::new(t);
    let core = Core::new(t);
    let cx = Ctx {
        roads: &roads,
        core: &core,
    };
    let mut out: Vec<Placed> = Vec::new();
    let mut drives: Vec<Pt> = Vec::new();
    for (i, lot) in t.lots.iter().enumerate() {
        let Some(obj) = lots.get(i) else { continue };
        let yard = match lot.kind {
            LotKind::House | LotKind::Row => house_yard(lot, i, obj, &cx, &mut drives),
            LotKind::Shop
            | LotKind::FastFood
            | LotKind::Office
            | LotKind::Apartments
            | LotKind::Warehouse
            | LotKind::Workshop
            | LotKind::Gas => forecourt(lot, i, obj, &cx),
            _ => None,
        };
        if let Some(y) = yard {
            let (x, yy) = (lot.centre.x as f32, lot.centre.y as f32);
            out.push((y, x, yy, lot.facing as f32, land.sit_height(x, yy)));
        }
    }
    plaza(t, land, &roads, &circles, &mut out);
    street_trees(t, land, &roads, &circles, &core, &drives, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    fn placed(seed: u64) -> (Town, Vec<Placed>) {
        let t = generate(Preset::Grid, seed);
        let land = Land::from_town(&t);
        let objs: Vec<Obj> = t
            .lots
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let mut rng = Rng::new(t.seed ^ 0x5eed_0000, i as u64 * 7919 + 13);
                kit::fit_to_lot(
                    kit::build_lot_in(crate::parts::Region::of(t.preset), l.kind, &mut rng),
                    l.radius as f32,
                )
            })
            .collect();
        let refs: Vec<&Obj> = objs.iter().collect();
        let out = build(&t, &land, &refs);
        (t, out)
    }

    #[test]
    fn same_town_same_streetscape() {
        let (_, a) = placed(2);
        let (_, b) = placed(2);
        assert_eq!(a.len(), b.len());
        for (p, q) in a.iter().zip(&b) {
            assert_eq!((p.1, p.2, p.3), (q.1, q.2, q.3));
            assert_eq!(p.0.polys.len(), q.0.polys.len());
        }
        assert!(a.len() > 1000, "a town gets trees and yards");
    }

    #[test]
    fn trees_keep_off_roads_and_buildings() {
        let (t, out) = placed(2);
        let roads = Roads::new(&t);
        let circles = Circles::new(&t);
        let mut checked = 0;
        // tree placements stand at turn 0 with no yard decals; yards are placed at a lot centre
        for (o, x, y, turn, _) in &out {
            let at_lot = t.lots.iter().any(|l| {
                (l.centre.x as f32 - x).abs() < 0.01 && (l.centre.y as f32 - y).abs() < 0.01
            });
            if at_lot || *turn != 0.0 || o.polys.len() > 60 {
                continue;
            }
            let p = pt(*x as f64, *y as f64);
            let (d, ri) = roads.edge_dist(p, None);
            assert!(
                d > 1.5,
                "tree on a road at {x} {y}: {d} from {:?} {:?} polys {}",
                ri.map(|r| roads.class[r]),
                ri.map(|r| (t.roads[r].points.len(), t.roads[r].points[0])),
                o.polys.len()
            );
            assert!(
                !circles.blocked(p, -1.6),
                "tree inside a building lot at {x} {y}"
            );
            checked += 1;
        }
        assert!(checked > 500);
    }
}
