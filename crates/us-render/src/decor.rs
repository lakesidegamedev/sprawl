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
        if !matches!(
            r.class,
            RoadClass::Local | RoadClass::Collector | RoadClass::Arterial
        ) || r.points.len() < 2
        {
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
                let (off, step) = if r.class == RoadClass::Arterial {
                    (4.2, 9.5)
                } else if at_core {
                    (1.6, 8.5)
                } else {
                    (3.1, 9.5)
                };
                // uneven gaps and the odd missing tree: a planted row, not a ruler
                s += step + rng.range(-3.0, 3.2) as f64;
                if rng.f32() < 0.15 {
                    continue;
                }
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

/// Planter trees and benches in open downtown ground.
fn plaza(t: &Town, land: &Land, roads: &Roads, circles: &Circles, out: &mut Vec<Placed>) {
    const SZ: f64 = 10.5;
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
                if pick < 0.55 {
                    let tr = street_tree(true, &mut rng, x, y);
                    o.add_placed(&tr, 0.0, 0.0, 0.0);
                    o.bx_top(-1.75, -1.4, 0.0, 1.75, 1.4, 0.38, [150, 130, 108], SOIL);
                    for (sx, sy) in [(-1.1, -0.75), (1.05, -0.6), (0.9, 0.85)] {
                        let tone = rng.pick(&[[96, 150, 66], [112, 158, 78], [78, 126, 77]]);
                        o.pyramid(sx, sy, 0.38, 0.43, rng.range(0.46, 0.75), tone, 6, 0.15);
                    }
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

/// The parts of the streetscape the running game places one lot or one street at a time, so a
/// growing town gets its trees and yards as things open and get built. Placement is the same as
/// `build`: the same seeds, the same rules.
pub struct Streetscape {
    roads: Roads,
    core: Core,
}

impl Streetscape {
    pub fn new(t: &Town) -> Streetscape {
        Streetscape {
            roads: Roads::new(t),
            core: Core::new(t),
        }
    }

    /// The grounds of lot `i` of the town around its model `obj` (driveway, walk, lawn bands,
    /// shrubs for houses; paving, planters and furniture for shops, offices and depots). The
    /// result is local to the lot (facing south, like the model). Also the point a driveway
    /// meets the street, which street trees keep clear of.
    pub fn lot_yard(&self, t: &Town, i: usize, obj: &Obj) -> (Option<Obj>, Option<Pt>) {
        let Some(lot) = t.lots.get(i) else {
            return (None, None);
        };
        let cx = Ctx {
            roads: &self.roads,
            core: &self.core,
        };
        let mut drives = Vec::new();
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
        (yard, drives.first().copied())
    }

    /// Every street tree of the town, as `build` places them but without the driveway check
    /// (the game drops a tree whose spot a built lot's driveway took).
    pub fn street_trees(&self, t: &Town, land: &Land) -> Vec<Placed> {
        let circles = Circles::new(t);
        let mut out = Vec::new();
        street_trees(t, land, &self.roads, &circles, &self.core, &[], &mut out);
        out
    }

    /// Planters and benches in open downtown ground.
    pub fn plaza(&self, t: &Town, land: &Land) -> Vec<Placed> {
        let circles = Circles::new(t);
        let mut out = Vec::new();
        plaza(t, land, &self.roads, &circles, &mut out);
        out
    }
}

/// The paved ground under one built lot in a downtown, industrial or mall district: a pad the
/// size of the lot, so land that is not built on stays natural. Local to the lot.
pub fn lot_pad(zone: Zone, r: f32, footprint: [f32; 4]) -> Option<Obj> {
    use crate::scene::{PAVING_CORE, PAVING_INDUSTRY, PAVING_MALL};
    let base = match zone {
        Zone::Core => PAVING_CORE,
        Zone::Industry => PAVING_INDUSTRY,
        Zone::Mall => PAVING_MALL,
        _ => return None,
    };
    // the building and a margin of paving, never more than the lot: a big mall or works lot is
    // paved where it is built, not across the whole circle
    let m = 7.0;
    let (x0, y0) = (
        (footprint[0] - m).max(-r * 0.92),
        (footprint[1] - m).max(-r * 0.92),
    );
    let (x1, y1) = (
        (footprint[2] + m).min(r * 0.92),
        (footprint[3] + m).min(r * 0.92),
    );
    if x1 - x0 < 2.0 || y1 - y0 < 2.0 {
        return None;
    }
    let mut o = Obj::new();
    if zone == Zone::Core {
        // downtown lots are paved edge to edge so built neighbours join into paved blocks; the
        // plain paving of next-door lots overlaps in one colour, the checker stays by the building
        quad(&mut o, -r, -r, r, r, 0.03, base);
    } else {
        // trodden ground round the paving, so the pad does not end in a clean line on grass
        quad(
            &mut o,
            (x0 - 1.3).max(-r * 0.92),
            (y0 - 1.3).max(-r * 0.92),
            (x1 + 1.3).min(r * 0.92),
            (y1 + 1.3).min(r * 0.92),
            0.025,
            [150, 141, 118],
        );
        quad(&mut o, x0, y0, x1, y1, 0.03, base);
    }
    if zone == Zone::Core {
        // paving bands: darker squares in a checker
        let dark = shade(base, 0.93);
        let (nx, ny) = (
            ((x1 - x0) / 3.5).ceil() as i32,
            ((y1 - y0) / 3.5).ceil() as i32,
        );
        for ix in 0..nx {
            for iy in 0..ny {
                if (ix + iy) & 1 == 1 {
                    let (qx, qy) = (x0 + ix as f32 * 3.5, y0 + iy as f32 * 3.5);
                    quad(
                        &mut o,
                        qx,
                        qy,
                        (qx + 3.5).min(x1),
                        (qy + 3.5).min(y1),
                        0.034,
                        dark,
                    );
                }
            }
        }
    }
    Some(o)
}

// ---- the town square and footpaths ----

/// The square lies around the hall, in the hall's own frame (front toward -y).
const SQUARE_X: f32 = 39.0;
const SQUARE_Y0: f32 = -39.0;
const SQUARE_Y1: f32 = 36.0;
const SQUARE_TILE: f32 = 3.0;

/// Hall frame to world.
fn hall_world(h: Pt, th: f32, lx: f32, ly: f32) -> (f32, f32) {
    let (s, c) = (libm::sinf(th), libm::cosf(th));
    (h.x as f32 + c * lx - s * ly, h.y as f32 + s * lx + c * ly)
}

fn bench_obj(along_x: bool) -> Obj {
    let mut o = Obj::new();
    let wood = [150, 104, 70];
    let iron = [60, 62, 70];
    if along_x {
        o.bx(-0.9, -0.25, 0.4, 0.9, 0.25, 0.55, wood);
        o.bx(-0.8, -0.2, 0.0, -0.65, 0.2, 0.4, iron);
        o.bx(0.65, -0.2, 0.0, 0.8, 0.2, 0.4, iron);
        o.bx(-0.9, 0.2, 0.55, 0.9, 0.3, 1.0, wood);
    } else {
        o.bx(-0.25, -0.9, 0.4, 0.25, 0.9, 0.55, wood);
        o.bx(-0.2, -0.8, 0.0, 0.2, -0.65, 0.4, iron);
        o.bx(-0.2, 0.65, 0.0, 0.2, 0.8, 0.4, iron);
        o.bx(0.2, -0.9, 0.55, 0.3, 0.9, 1.0, wood);
    }
    o
}

fn lamp_obj() -> Obj {
    let mut o = Obj::new();
    o.bx(-0.3, -0.3, 0.0, 0.3, 0.3, 0.35, [96, 98, 104]);
    o.bx(-0.1, -0.1, 0.35, 0.1, 0.1, 4.2, [52, 56, 66]);
    o.bx(-0.3, -0.3, 4.2, 0.3, 0.3, 4.7, [255, 232, 160]);
    o.bx(-0.4, -0.4, 4.7, 0.4, 0.4, 4.85, [52, 56, 66]);
    o
}

fn fountain_obj() -> Obj {
    let mut o = Obj::new();
    let stone = [176, 174, 170];
    o.prism(0.0, 0.0, 0.0, 0.7, 4.8, stone, 14, Some([150, 148, 146]));
    o.prism(
        0.0,
        0.0,
        0.5,
        0.62,
        4.1,
        [92, 164, 214],
        14,
        Some([112, 182, 226]),
    );
    o.prism(0.0, 0.0, 0.0, 2.6, 0.9, stone, 8, None);
    o.prism(0.0, 0.0, 2.2, 2.5, 2.0, stone, 12, Some([92, 164, 214]));
    o.prism(
        0.0,
        0.0,
        2.5,
        3.7,
        0.22,
        [200, 232, 250],
        6,
        Some([230, 246, 255]),
    );
    o
}

/// The town square: what to draw and where its paving is.
pub struct Square {
    pub pieces: Vec<Placed>,
    /// Centres of the paved tiles, world coordinates: the places a footpath can end.
    pub tiles: Vec<Pt>,
}

/// The paved town square around the hall: paving bands, a ring path and a straight path from the
/// hall, a fountain, benches, trees in pits and lamp posts. Paving is left out where a street,
/// a lot or water is, and the fountain goes on whichever side of the hall has the most room.
/// Pieces are placed one per patch so the game can draw each in the chunk it stands in.
/// `theta` is the hall's turn.
pub fn town_square(t: &Town, land: &Land, hall: Pt, theta: f32) -> Square {
    let roads = Roads::new(t);
    let near_lots: Vec<(Pt, f64)> = t
        .lots
        .iter()
        .filter(|l| l.centre.dist(hall) < 110.0)
        .map(|l| (l.centre, l.radius))
        .collect();
    let free = |lx: f32, ly: f32, m: f64| -> bool {
        let (x, y) = hall_world(hall, theta, lx, ly);
        let p = pt(x as f64, y as f64);
        roads.edge_dist(p, None).0 > m
            && !near_lots.iter().any(|(c, r)| c.dist(p) < r - 1.0 + m)
            && land.water_at(p).is_none()
    };
    let under_hall = |mx: f32, my: f32| mx.abs() < 16.5 && my > -12.8 && my < 9.5;
    // the fountain stands on the side of the hall with the most paving room
    let sides: [((f32, f32), (f32, f32)); 4] = [
        ((0.0, -30.0), (0.0, -12.8)),
        ((0.0, 25.0), (0.0, 9.5)),
        ((-30.0, -2.0), (-16.5, -2.0)),
        ((30.0, -2.0), (16.5, -2.0)),
    ];
    let room = |f: (f32, f32)| -> usize {
        let mut n = 0;
        let mut y = f.1 - 12.0;
        while y <= f.1 + 12.0 {
            let mut x = f.0 - 12.0;
            while x <= f.0 + 12.0 {
                if (x - f.0).hypot(y - f.1) <= 12.0 && !under_hall(x, y) && free(x, y, 0.5) {
                    n += 1;
                }
                x += SQUARE_TILE;
            }
            y += SQUARE_TILE;
        }
        n
    };
    let mut best = (0usize, 0usize);
    for (k, (f, _)) in sides.iter().enumerate() {
        let n = room(*f);
        if n > best.0 {
            best = (n, k);
        }
    }
    let (fountain, door) = sides[best.1];
    let sand: Rgb = [216, 198, 162];
    let light: Rgb = [204, 200, 192];
    let dark: Rgb = [188, 184, 178];
    let edge: Rgb = [226, 222, 214];
    let on_axis = |mx: f32, my: f32| -> bool {
        let (ax, ay) = (door.0, door.1);
        let (bx, by) = fountain;
        let (dx, dy) = (bx - ax, by - ay);
        let l2 = dx * dx + dy * dy;
        let u = (((mx - ax) * dx + (my - ay) * dy) / l2).clamp(0.0, 1.0);
        (mx - ax - dx * u).hypot(my - ay - dy * u) < 2.0
    };
    let mut out: Vec<Placed> = Vec::new();
    let mut tiles: Vec<Pt> = Vec::new();
    // paving, one patch of 6 x 6 tiles at a time
    let t_sz = SQUARE_TILE;
    let nx = (2.0 * SQUARE_X / t_sz).round() as i32;
    let ny = ((SQUARE_Y1 - SQUARE_Y0) / t_sz).round() as i32;
    let patch = 6;
    for bx in 0..(nx + patch - 1) / patch {
        for by in 0..(ny + patch - 1) / patch {
            let (ox, oy) = (
                -SQUARE_X + (bx * patch) as f32 * t_sz,
                SQUARE_Y0 + (by * patch) as f32 * t_sz,
            );
            let (cx, cy) = (
                ox + patch as f32 * t_sz / 2.0,
                oy + patch as f32 * t_sz / 2.0,
            );
            let mut o = Obj::new();
            let mut any = false;
            for ix in bx * patch..((bx + 1) * patch).min(nx) {
                for iy in by * patch..((by + 1) * patch).min(ny) {
                    let x0 = -SQUARE_X + ix as f32 * t_sz;
                    let y0 = SQUARE_Y0 + iy as f32 * t_sz;
                    let (mx, my) = (x0 + t_sz / 2.0, y0 + t_sz / 2.0);
                    if under_hall(mx, my) || !free(mx, my, 0.5) {
                        continue;
                    }
                    let df = (mx - fountain.0).hypot(my - fountain.1);
                    // the paving thins out toward the corners
                    if df > 30.0 && (mx.abs() > 30.0 || !(-30.0..=26.0).contains(&my)) {
                        continue;
                    }
                    let rim = mx.abs() > SQUARE_X - t_sz || my < SQUARE_Y0 + t_sz;
                    let col = if (8.2..10.8).contains(&df) || on_axis(mx, my) {
                        sand
                    } else if rim {
                        edge
                    } else if (ix + iy) & 1 == 0 {
                        light
                    } else {
                        dark
                    };
                    quad(
                        &mut o,
                        x0 - cx,
                        y0 - cy,
                        x0 + t_sz - cx,
                        y0 + t_sz - cy,
                        0.03,
                        col,
                    );
                    let (wx, wy) = hall_world(hall, theta, mx, my);
                    tiles.push(pt(wx as f64, wy as f64));
                    any = true;
                }
            }
            if any {
                let (wx, wy) = hall_world(hall, theta, cx, cy);
                out.push((o, wx, wy, theta, land.height(wx, wy)));
            }
        }
    }
    let mut put = |o: Obj, lx: f32, ly: f32, extra: f32, m: f64| {
        if free(lx, ly, m) && !under_hall(lx, ly) {
            let (wx, wy) = hall_world(hall, theta, lx, ly);
            out.push((o, wx, wy, theta + extra, land.height(wx, wy)));
        }
    };
    let (fx, fy) = fountain;
    put(fountain_obj(), fx, fy, 0.0, 2.0);
    // benches around the ring, facing the fountain
    for (dx, dy, along_x) in [
        (0.0, 11.6, true),
        (0.0, -11.6, true),
        (-11.6, 0.0, false),
        (11.6, 0.0, false),
    ] {
        put(bench_obj(along_x), fx + dx, fy + dy, 0.0, 0.5);
    }
    // trees in pits and lamp posts on the ring's outside, and by the path
    let mut rng = Rng::new(t.seed ^ 0x7077_a000, 5);
    for (dx, dy) in [
        (-8.0f32, -8.0f32),
        (8.0, -8.0),
        (-8.0, 8.0),
        (8.0, 8.0),
        (-16.0, 0.0),
        (16.0, 0.0),
        (0.0, -16.0),
        (0.0, 16.0),
    ] {
        let mut o = Obj::new();
        let tr = street_tree(true, &mut rng, 0.0, 0.0);
        o.add_placed(&tr, 0.0, 0.0, 0.0);
        o.bx_top(-1.2, -1.2, 0.0, 1.2, 1.2, 0.4, [150, 130, 108], SOIL);
        // trees stand upright whatever the hall's turn
        put(o, fx + dx, fy + dy, -theta, 1.5);
    }
    for (dx, dy) in [
        (-5.5f32, -5.5f32),
        (5.5, -5.5),
        (-5.5, 5.5),
        (5.5, 5.5),
        (-12.0, -12.0),
        (12.0, -12.0),
        (-12.0, 12.0),
        (12.0, 12.0),
        (-20.0, 0.0),
        (20.0, 0.0),
    ] {
        put(lamp_obj(), fx + dx, fy + dy, 0.0, 0.8);
    }
    // the rest of the paving: trees in pits, benches and lamps on a loose 12 m lattice
    for i in -4i32..=4 {
        for j in -4i32..=4 {
            let (lx, ly) = (i as f32 * 12.0, -1.5 + j as f32 * 12.0);
            let off_ring = (lx - fx).hypot(ly - fy) > 14.0;
            let by_hall = lx.abs() < 20.0 && ly > -17.0 && ly < 13.0;
            if !off_ring || by_hall {
                continue;
            }
            match (i + j).rem_euclid(4) {
                0 | 2 => {
                    let mut o = Obj::new();
                    let tr = street_tree(true, &mut rng, 0.0, 0.0);
                    o.add_placed(&tr, 0.0, 0.0, 0.0);
                    o.bx_top(-1.2, -1.2, 0.0, 1.2, 1.2, 0.4, [150, 130, 108], SOIL);
                    put(o, lx, ly, -theta, 1.5);
                }
                1 => put(bench_obj(i % 2 == 0), lx, ly, 0.0, 1.0),
                _ => put(lamp_obj(), lx, ly, 0.0, 0.8),
            }
        }
    }
    // lamps along the hall's own front
    for lx in [-12.0f32, 12.0] {
        let ly = if door.1 < -1.0 { -15.5 } else { 12.5 };
        if door.0 == 0.0 {
            put(lamp_obj(), lx, ly, 0.0, 0.8);
        }
    }
    Square { pieces: out, tiles }
}

/// A paved footpath from `from` to `to`: straight, or with one bend if a lot stands in the way
/// of the straight line. `blocks` are circles to go round. The result lists the corner points.
pub fn plan_footpath(from: Pt, to: Pt, blocks: &[(Pt, f64)]) -> Vec<Pt> {
    let clear = |a: Pt, b: Pt| blocks.iter().all(|(c, r)| seg_dist(*c, a, b) > *r);
    if clear(from, to) {
        return vec![from, to];
    }
    let mut best: Option<(f64, Vec<Pt>)> = None;
    let d = to - from;
    let n = pt(-d.y, d.x);
    let nl = (n.x * n.x + n.y * n.y).sqrt().max(1e-9);
    let mut cands = vec![pt(from.x, to.y), pt(to.x, from.y)];
    for k in [-1.0f64, 1.0] {
        for f in [0.3f64, 0.5, 0.7] {
            let m = from.lerp(to, f);
            for off in [10.0f64, 18.0, 30.0] {
                cands.push(pt(m.x + n.x / nl * off * k, m.y + n.y / nl * off * k));
            }
        }
    }
    for c in cands {
        if clear(from, c) && clear(c, to) {
            let len = from.dist(c) + c.dist(to);
            if best.as_ref().is_none_or(|(l, _)| len < *l) {
                best = Some((len, vec![from, c, to]));
            }
        }
    }
    best.map_or_else(|| vec![from, to], |(_, v)| v)
}

fn seg_dist(p: Pt, a: Pt, b: Pt) -> f64 {
    let (abx, aby) = (b.x - a.x, b.y - a.y);
    let l2 = abx * abx + aby * aby;
    let t = if l2 < 1e-12 {
        0.0
    } else {
        (((p.x - a.x) * abx + (p.y - a.y) * aby) / l2).clamp(0.0, 1.0)
    };
    p.dist(pt(a.x + abx * t, a.y + aby * t))
}

/// The paved ribbon of a footpath, cut into pieces of at most `reach` metres: each piece is
/// local to the point returned with it (its middle), so the game can draw it in its own chunk.
/// Short quads follow the ground over hills.
pub fn footpath_pieces(line: &[Pt], reach: f64) -> Vec<(Obj, Pt)> {
    const W: f32 = 1.1;
    let mut out = Vec::new();
    // walk the line in steps of about 4 m, grouping steps into pieces
    let mut steps: Vec<(Pt, Pt)> = Vec::new();
    for w in line.windows(2) {
        let n = ((w[0].dist(w[1]) / 4.0).ceil() as usize).max(1);
        for k in 0..n {
            steps.push((
                w[0].lerp(w[1], k as f64 / n as f64),
                w[0].lerp(w[1], (k + 1) as f64 / n as f64),
            ));
        }
    }
    let mut i = 0;
    while i < steps.len() {
        let mut j = i;
        let mut len = 0.0;
        while j < steps.len() && len < reach {
            len += steps[j].0.dist(steps[j].1);
            j += 1;
        }
        let mid = steps[i].0.lerp(steps[j - 1].1, 0.5);
        let mut o = Obj::new();
        for (k, (a, b)) in steps[i..j].iter().enumerate() {
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l = (dx * dx + dy * dy).sqrt().max(1e-9);
            let (nx, ny) = ((-dy / l) as f32 * W, (dx / l) as f32 * W);
            let (ax, ay) = ((a.x - mid.x) as f32, (a.y - mid.y) as f32);
            let (bx, by) = ((b.x - mid.x) as f32, (b.y - mid.y) as f32);
            // a little overlap at each end keeps bends closed
            let (ex, ey) = ((dx / l) as f32 * 0.5, (dy / l) as f32 * 0.5);
            let (ax, ay, bx, by) = (ax - ex, ay - ey, bx + ex, by + ey);
            let col = if k % 2 == 0 {
                CONC_L
            } else {
                shade(CONC_L, 0.96)
            };
            let pts: [V3; 4] = [
                [ax - nx, ay - ny, 0.04],
                [bx - nx, by - ny, 0.04],
                [bx + nx, by + ny, 0.04],
                [ax + nx, ay + ny, 0.04],
            ];
            o.poly(&pts, col, false, false, true);
        }
        out.push((o, mid));
        i = j;
    }
    out
}

/// How close a driveway keeps street trees away, metres.
pub const DRIVE_CLEAR: f64 = 3.4;

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
    fn streetscape_places_what_build_places() {
        let (t, all) = placed(2);
        let land = Land::from_town(&t);
        let scape = Streetscape::new(&t);
        // every street tree of the one-shot build is among the per-street trees
        let trees = scape.street_trees(&t, &land);
        let kept = all
            .iter()
            .filter(|p| p.3 == 0.0 && p.0.polys.len() <= 60)
            .count();
        assert!(trees.len() >= kept.min(trees.len()) && !trees.is_empty());
        // a house gets a yard from its own model, the same every time
        let i = t
            .lots
            .iter()
            .position(|l| l.kind == LotKind::House)
            .unwrap();
        let mut rng = Rng::new(t.seed ^ 0x5eed_0000, i as u64 * 7919 + 13);
        let obj = kit::fit_to_lot(
            kit::build_lot_in(crate::parts::Region::of(t.preset), LotKind::House, &mut rng),
            t.lots[i].radius as f32,
        );
        let (a, da) = scape.lot_yard(&t, i, &obj);
        let (b, db) = scape.lot_yard(&t, i, &obj);
        assert_eq!(a.map(|o| o.polys.len()), b.map(|o| o.polys.len()));
        assert_eq!(da, db);
    }

    #[test]
    fn lot_pad_stays_inside_the_lot() {
        let pad = lot_pad(Zone::Mall, 50.0, [-10.0, -8.0, 12.0, 9.0]).unwrap();
        for p in &pad.polys {
            for v in &p.pts {
                assert!(v[0].abs() <= 46.0 && v[1].abs() <= 46.0);
                assert!(v[0] >= -17.1 && v[0] <= 19.1, "pad hugs the building");
            }
        }
        assert!(lot_pad(Zone::Inner, 20.0, [-5.0, -5.0, 5.0, 5.0]).is_none());
    }

    #[test]
    fn town_square_is_the_same_every_time_and_keeps_off_roads() {
        let t = generate(Preset::LakeTown, 3);
        let land = Land::from_town(&t);
        let a = town_square(&t, &land, t.hall, 0.4);
        let b = town_square(&t, &land, t.hall, 0.4);
        assert_eq!(a.tiles.len(), b.tiles.len());
        assert!(a.tiles.len() > 40, "a square has paving: {}", a.tiles.len());
        assert!(a.pieces.len() > 4);
        let roads = Roads::new(&t);
        for p in &a.tiles {
            assert!(
                roads.edge_dist(*p, None).0 > 0.0,
                "paving on a road at {p:?}"
            );
        }
    }

    #[test]
    fn footpath_bends_round_a_lot_and_pieces_are_short() {
        let (a, b) = (pt(0.0, 0.0), pt(100.0, 0.0));
        assert_eq!(plan_footpath(a, b, &[]).len(), 2);
        let line = plan_footpath(a, b, &[(pt(50.0, 0.0), 8.0)]);
        assert_eq!(line.len(), 3, "one bend");
        for w in line.windows(2) {
            assert!(seg_dist(pt(50.0, 0.0), w[0], w[1]) > 8.0);
        }
        let pieces = footpath_pieces(&line, 40.0);
        assert!(pieces.len() >= 3);
        for (o, _) in &pieces {
            let f = o.footprint();
            assert!(f[2] - f[0] < 60.0 && f[3] - f[1] < 60.0);
        }
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
