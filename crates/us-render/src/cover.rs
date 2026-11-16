//! Ground cover: grass tufts, bushes, boulders, flower mounds, reeds, logs, crop rows, hay bales,
//! hedges, walls, fences and tree lines, placed from the town seed and the ground polygons.
//! Everything is a few flat faces in the cel look, baked into static chunks with a far form that
//! keeps only the big shapes (hedges, walls, tree lines, boulders). Nothing is placed on roads,
//! lots, rail, water, or inside the built-up additions (the buildings track owns lots).

use crate::kit;
use crate::land::Land;
use crate::mesh::{ID_NONE, Mesh};
use crate::obj::{Obj, Rgb, Sit};
use crate::rng::Rng;
use crate::scene::{Chunk, RoadGrid};
use glam::Vec3;
use std::collections::{BTreeMap, BTreeSet};
use us_gen::{Pattern, Pt, Town, Zone, pt};

/// Marker colours the shader recolours by season (blue carries a step 0 to 3).
pub const GRASS_M: Rgb = [100, 168, 58];
pub const FLOWER_M: Rgb = [246, 206, 30];
pub const WHEAT_M: Rgb = [212, 176, 60];
pub const BALE_M: Rgb = [232, 188, 70];

const CELL: f32 = 6.0;
const CHUNK: f32 = 256.0;
const ID_LO: u32 = 0x0090_0000;
const ID_SPAN: u32 = 0x002F_0000;

/// Which part of the town a build covers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Group {
    /// Open meadow, field margins, crop rows, field edges, reeds.
    Base,
    /// The edge band and woods of one park addition.
    Park(usize),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Crop {
    Green,
    Wheat,
    Plough,
    Hay,
}

/// Crop of a field, from its colour index in `scene::FIELDS` so near and far views agree.
fn crop_of(idx: usize) -> Crop {
    [
        Crop::Green,
        Crop::Wheat,
        Crop::Green,
        Crop::Hay,
        Crop::Plough,
        Crop::Green,
        Crop::Wheat,
        Crop::Plough,
    ][idx % 8]
}

fn hash2(a: u64, b: u64) -> u64 {
    Rng::new(a, b).next_u64()
}

fn lat(seed: u64, i: i64, j: i64) -> f32 {
    Rng::new(
        seed ^ 0xA5A5,
        ((i as u64 & 0xF_FFFF) << 24) | (j as u64 & 0xFF_FFFF),
    )
    .f32()
}

/// Smooth value noise in [0, 1].
fn vn(seed: u64, x: f32, y: f32) -> f32 {
    let (fx, fy) = (x.floor(), y.floor());
    let (i, j) = (fx as i64, fy as i64);
    let (tx, ty) = (x - fx, y - fy);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = lat(seed, i, j);
    let b = lat(seed, i + 1, j);
    let c = lat(seed, i, j + 1);
    let d = lat(seed, i + 1, j + 1);
    (a + (b - a) * sx) + ((c + (d - c) * sx) - (a + (b - a) * sx)) * sy
}

fn m(c: Rgb, step: u8) -> Rgb {
    [c[0], c[1], c[2] + step.min(3)]
}

// ---- models ----

fn plain(mut o: Obj, shadow: bool, outline: bool) -> Obj {
    for p in &mut o.polys {
        p.shadow = shadow;
        p.outline = outline;
    }
    o
}

fn tuft(h: f32, step: u8) -> Obj {
    let mut o = Obj::new();
    let c = m(GRASS_M, step);
    for &(x, y, k) in &[(0.0, 0.0, 1.0), (0.3, 0.12, 0.7), (-0.24, 0.2, 0.82)] {
        o.pyramid(x * h, y * h, 0.0, 0.2 * h, h * k, c, 3, x * 90.0);
    }
    plain(o, false, false)
}

fn bush(shape: usize, dark: bool, step: u8) -> Obj {
    let c = m(if dark { kit::TREE_D } else { kit::TREE }, step);
    let mut o = Obj::new();
    match shape {
        0 => {
            o.prism(0.0, 0.0, 0.0, 0.55, 1.0, c, 6, None);
            o.pyramid(0.0, 0.0, 0.55, 1.0, 0.55, c, 6, 0.0);
        }
        1 => {
            o.prism(0.0, 0.0, 0.0, 0.45, 1.5, c, 6, None);
            o.pyramid(0.0, 0.0, 0.45, 1.5, 0.45, c, 6, 20.0);
            o.prism(0.9, 0.4, 0.0, 0.75, 0.8, c, 5, None);
            o.pyramid(0.9, 0.4, 0.75, 0.8, 0.4, c, 5, 0.0);
        }
        _ => {
            o.prism(0.0, 0.0, 0.0, 1.0, 0.65, c, 6, None);
            o.pyramid(0.0, 0.0, 1.0, 0.65, 0.8, c, 6, 30.0);
        }
    }
    plain(o, true, true)
}

fn flowers(step: u8) -> Obj {
    let mut o = Obj::new();
    o.pyramid(0.0, 0.0, 0.0, 0.6, 0.3, GRASS_M, 5, 0.0);
    let f = m(FLOWER_M, step);
    for k in 0..4 {
        let a = k as f32 * 1.571 + 0.3;
        let r = 0.28 + 0.12 * (k % 2) as f32;
        o.pyramid(
            r * libm::cosf(a),
            r * libm::sinf(a),
            0.1,
            0.17,
            0.36,
            f,
            4,
            a * 57.0,
        );
    }
    plain(o, false, false)
}

fn reeds(cattail: bool) -> Obj {
    let mut o = Obj::new();
    for k in 0..4 {
        let a = k as f32 * 1.6;
        let (x, y) = (
            0.28 * libm::cosf(a) * (k as f32 / 3.0),
            0.28 * libm::sinf(a),
        );
        let h = 1.2 + 0.1 * ((k * 5) % 6) as f32;
        o.pyramid(x, y, 0.0, 0.09, h, m(GRASS_M, (k % 3) as u8), 3, a * 60.0);
        if cattail && k % 2 == 1 {
            o.prism(x, y, h - 0.35, h + 0.05, 0.08, [112, 72, 44], 3, None);
        }
    }
    plain(o, false, false)
}

fn rock(shape: usize, col: Rgb) -> Obj {
    let mut o = Obj::new();
    match shape {
        0 => {
            o.prism(0.0, 0.0, 0.0, 0.5, 0.9, col, 5, None);
            o.pyramid(0.1, 0.0, 0.5, 0.75, 0.4, col, 5, 20.0);
        }
        1 => {
            o.prism(0.0, 0.0, 0.0, 0.32, 1.1, col, 6, None);
            o.pyramid(-0.1, 0.1, 0.32, 0.8, 0.28, col, 6, 10.0);
        }
        _ => {
            o.prism(0.0, 0.0, 0.0, 0.4, 0.55, col, 5, None);
            o.pyramid(0.0, 0.0, 0.4, 0.45, 0.25, col, 5, 0.0);
            o.prism(0.7, 0.3, 0.0, 0.28, 0.35, col, 5, None);
            o.pyramid(0.7, 0.3, 0.28, 0.3, 0.2, col, 5, 30.0);
            o.prism(-0.5, 0.5, 0.0, 0.22, 0.3, col, 5, None);
            o.pyramid(-0.5, 0.5, 0.22, 0.3, 0.15, col, 5, 10.0);
        }
    }
    plain(o, true, true)
}

/// A cylinder lying along x from -len/2 to len/2.
fn cyl_x(len: f32, r: f32, sides: usize, col: Rgb, end: Rgb) -> Obj {
    let mut o = Obj::new();
    let ring: Vec<(f32, f32)> = (0..sides)
        .map(|i| {
            let a = i as f32 * std::f32::consts::TAU / sides as f32;
            (r * libm::cosf(a), r + r * libm::sinf(a))
        })
        .collect();
    let (x0, x1) = (-len / 2.0, len / 2.0);
    for i in 0..sides {
        let (a, b) = (ring[i], ring[(i + 1) % sides]);
        o.poly(
            &[
                [x0, a.0, a.1],
                [x0, b.0, b.1],
                [x1, b.0, b.1],
                [x1, a.0, a.1],
            ],
            col,
            true,
            true,
            false,
        );
    }
    let cap1: Vec<[f32; 3]> = ring.iter().map(|p| [x1, p.0, p.1]).collect();
    let cap0: Vec<[f32; 3]> = ring.iter().rev().map(|p| [x0, p.0, p.1]).collect();
    o.poly(&cap1, end, true, true, false);
    o.poly(&cap0, end, true, true, false);
    o
}

/// One piece of crop row along x: a low ridge with two sloping faces.
fn row_piece(len: f32, col: Rgb) -> Obj {
    let (l, w, h) = (len / 2.0, 1.0, 0.28);
    let mut o = Obj::new();
    o.poly(
        &[[-l, w, 0.0], [-l, 0.0, h], [l, 0.0, h], [l, w, 0.0]],
        col,
        false,
        false,
        false,
    );
    o.poly(
        &[[-l, -w, 0.0], [l, -w, 0.0], [l, 0.0, h], [-l, 0.0, h]],
        col,
        false,
        false,
        false,
    );
    o
}

fn hedge_piece(len: f32, h: f32, step: u8, dark: bool) -> Obj {
    let c = m(if dark { kit::TREE_D } else { kit::TREE }, step);
    let mut o = Obj::new();
    let l = len / 2.0;
    o.bx(-l, -0.5, 0.0, l, 0.5, h, c);
    plain_keep(o)
}

fn plain_keep(mut o: Obj) -> Obj {
    for p in &mut o.polys {
        p.outline = true;
    }
    o
}

fn wall_piece(len: f32) -> Obj {
    let mut o = Obj::new();
    let l = len / 2.0;
    o.bx_top(-l, -0.3, 0.0, l, 0.3, 0.9, [150, 146, 138], [176, 170, 158]);
    o
}

fn fence_piece(len: f32) -> Obj {
    let wood = [146, 108, 74];
    let mut o = Obj::new();
    let l = len / 2.0;
    let posts = ((len / 4.0).ceil() as usize).max(1) + 1;
    for k in 0..posts {
        let x = -l + len * k as f32 / (posts - 1) as f32;
        o.bx(x - 0.07, -0.07, 0.0, x + 0.07, 0.07, 1.15, wood);
    }
    o.bx(-l, -0.04, 0.4, l, 0.04, 0.54, wood);
    o.bx(-l, -0.04, 0.82, l, 0.04, 0.96, wood);
    plain_keep(o)
}

// ---- model registry ----

struct Tpl {
    obj: Obj,
    far: bool,
}

struct Kit {
    tpls: Vec<Tpl>,
    tuft: Vec<usize>,   // [size 2][step 4]
    bush: Vec<usize>,   // [shape 3][dark 2][step 4]
    flower: Vec<usize>, // [step 4]
    reed: [usize; 2],
    rock: Vec<usize>, // [shape 3][colour 3]
    log: [usize; 2],
    bale: usize,
    tree: Vec<usize>, // [size 3][dark 2][step 4]
    rows: BTreeMap<(u16, u8, u8), usize>,
}

impl Kit {
    fn add(&mut self, obj: Obj, far: bool) -> usize {
        self.tpls.push(Tpl { obj, far });
        self.tpls.len() - 1
    }

    fn new() -> Kit {
        let mut k = Kit {
            tpls: Vec::new(),
            tuft: Vec::new(),
            bush: Vec::new(),
            flower: Vec::new(),
            reed: [0; 2],
            rock: Vec::new(),
            log: [0; 2],
            bale: 0,
            tree: Vec::new(),
            rows: BTreeMap::new(),
        };
        for h in [0.7, 1.2] {
            for s in 0..4 {
                let i = k.add(tuft(h, s), false);
                k.tuft.push(i);
            }
        }
        for shape in 0..3 {
            for dark in [false, true] {
                for s in 0..4 {
                    let i = k.add(bush(shape, dark, s), false);
                    k.bush.push(i);
                }
            }
        }
        for s in 0..4 {
            let i = k.add(flowers(s), false);
            k.flower.push(i);
        }
        k.reed = [k.add(reeds(false), false), k.add(reeds(true), false)];
        for shape in 0..3 {
            for col in [[152, 150, 146], [122, 120, 122], [132, 148, 118]] {
                let i = k.add(rock(shape, col), shape != 2);
                k.rock.push(i);
            }
        }
        let bark = [112, 82, 56];
        k.log = [
            k.add(cyl_x(3.2, 0.28, 6, bark, [176, 140, 100]), false),
            k.add(cyl_x(2.2, 0.22, 6, bark, [176, 140, 100]), false),
        ];
        k.bale = k.add(cyl_x(1.3, 0.62, 6, BALE_M, [206, 168, 70]), false);
        for r in [(1.9f32, 5.9f32), (2.4, 7.4), (2.9, 8.9)] {
            for dark in [false, true] {
                for s in 0..4u8 {
                    let c = m(if dark { kit::TREE_D } else { kit::TREE }, s);
                    let i = k.add(kit::tree(r.0, r.1, c), true);
                    k.tree.push(i);
                }
            }
        }
        k
    }
}

// ---- context ----

struct Add {
    lo: Pt,
    hi: Pt,
    idx: usize,
}

struct Ctx<'a> {
    t: &'a Town,
    land: &'a Land,
    roads: RoadGrid,
    lots: BTreeMap<(i32, i32), Vec<(Pt, f64)>>,
    mask: BTreeSet<(i32, i32)>,
    near: BTreeSet<(i32, i32)>,
    adds: Vec<Add>,
    seed: u64,
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

fn ring_dist(p: Pt, ring: &[Pt]) -> f64 {
    let mut d = f64::MAX;
    for i in 0..ring.len() {
        d = d.min(seg_dist(p, ring[i], ring[(i + 1) % ring.len()]));
    }
    d
}

#[derive(Clone, Copy, PartialEq)]
enum Cls {
    Open,
    Farm(usize),
    Park(usize),
    Urban,
}

impl<'a> Ctx<'a> {
    fn new(t: &'a Town, land: &'a Land) -> Ctx<'a> {
        let mut lots: BTreeMap<(i32, i32), Vec<(Pt, f64)>> = BTreeMap::new();
        let mut mask = BTreeSet::new();
        let mut near = BTreeSet::new();
        for l in &t.lots {
            let c = (
                (l.centre.x / 48.0).floor() as i32,
                (l.centre.y / 48.0).floor() as i32,
            );
            lots.entry(c).or_default().push((l.centre, l.radius));
            let m = (
                (l.centre.x / 64.0).floor() as i32,
                (l.centre.y / 64.0).floor() as i32,
            );
            for dx in -2..=2 {
                for dy in -2..=2 {
                    near.insert((m.0 + dx, m.1 + dy));
                }
            }
            for dx in -4..=4 {
                for dy in -4..=4 {
                    mask.insert((m.0 + dx, m.1 + dy));
                }
            }
        }
        let adds = t
            .additions
            .iter()
            .enumerate()
            .map(|(idx, a)| {
                let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
                for p in &a.polygon {
                    lo = pt(lo.x.min(p.x), lo.y.min(p.y));
                    hi = pt(hi.x.max(p.x), hi.y.max(p.y));
                }
                Add { lo, hi, idx }
            })
            .collect();
        Ctx {
            t,
            land,
            roads: RoadGrid::new(t),
            lots,
            mask,
            near,
            adds,
            seed: t.seed ^ 0xC0_7E55,
        }
    }

    fn near_lot(&self, p: Pt, extra: f64) -> bool {
        let (cx, cy) = ((p.x / 48.0).floor() as i32, (p.y / 48.0).floor() as i32);
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for (c, r) in self.lots.get(&(x, y)).into_iter().flatten() {
                    if c.dist(p) < r + extra {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn close(&self, p: Pt) -> bool {
        self.near
            .contains(&((p.x / 64.0).floor() as i32, (p.y / 64.0).floor() as i32))
    }

    fn interesting(&self, p: Pt) -> bool {
        self.mask
            .contains(&((p.x / 64.0).floor() as i32, (p.y / 64.0).floor() as i32))
    }

    fn classify(&self, p: Pt) -> Cls {
        let (mut farm, mut park, mut urban) = (None, None, false);
        for a in &self.adds {
            if p.x < a.lo.x || p.x > a.hi.x || p.y < a.lo.y || p.y > a.hi.y {
                continue;
            }
            let ad = &self.t.additions[a.idx];
            if !in_ring(p, &ad.polygon) {
                continue;
            }
            match ad.zone {
                Zone::Farm => farm = Some(a.idx),
                Zone::Park => park = Some(a.idx),
                _ => urban = true,
            }
        }
        if let Some(i) = park {
            Cls::Park(i)
        } else if urban {
            Cls::Urban
        } else if let Some(i) = farm {
            Cls::Farm(i)
        } else {
            Cls::Open
        }
    }

    fn water(&self, p: Pt) -> bool {
        self.land.water_at(p).is_some()
    }

    fn shore(&self, p: Pt) -> bool {
        const D: [(f64, f64); 8] = [
            (4.0, 0.0),
            (-4.0, 0.0),
            (0.0, 4.0),
            (0.0, -4.0),
            (2.8, 2.8),
            (-2.8, 2.8),
            (2.8, -2.8),
            (-2.8, -2.8),
        ];
        D.iter().any(|d| self.water(pt(p.x + d.0, p.y + d.1)))
    }

    /// A spot where a plant may stand: not on a road, lot or water.
    fn free(&self, p: Pt, road: f64) -> bool {
        p.x > 4.0
            && p.y > self.t.highway_y
            && p.x < self.t.size - 4.0
            && p.y < self.t.size - 4.0
            && !self.water(p)
            && !self.land.wet_shelf(p.x as f32, p.y as f32)
            && self.roads.clearance(p) > road
            && !self.near_lot(p, 2.5)
    }
}

// ---- placement ----

struct Item {
    tpl: usize,
    x: f32,
    y: f32,
    theta: f32,
    scale: f32,
}

#[allow(clippy::too_many_arguments)]
fn scatter_cell(
    cx: &Ctx,
    kit: &Kit,
    cls: Cls,
    p: Pt,
    rng: &mut Rng,
    park_edge: f64,
    woods: bool,
    out: &mut Vec<Item>,
) {
    let (x, y) = (p.x as f32, p.y as f32);
    let patch = vn(cx.seed, x / 50.0, y / 50.0);
    let patch2 = vn(cx.seed ^ 7, x / 17.0 + 40.0, y / 17.0);
    let roll = rng.f32();
    let clear = cx.roads.clearance(p);
    let slope = cx.land.slope(x, y);
    let step = rng.below(4);
    let mut put = |tpl: usize, scale: f32, rng: &mut Rng| {
        out.push(Item {
            tpl,
            x: p.x as f32,
            y: p.y as f32,
            theta: rng.range(0.0, std::f32::consts::TAU),
            scale,
        });
    };
    let tuft = |tall: bool, step: usize| kit.tuft[(tall as usize) * 4 + step];
    let bushi = |rng: &mut Rng| kit.bush[rng.below(3) * 8 + rng.below(2) * 4 + rng.below(4)];
    if cx.shore(p) {
        if roll < 0.85 {
            let r = kit.reed[rng.below(2)];
            put(r, rng.range(0.8, 1.3), rng);
        } else {
            put(tuft(true, step), rng.range(0.9, 1.4), rng);
        }
        return;
    }
    match cls {
        Cls::Farm(_) => {
            // the tall grass margin between a field and its boundary
            if roll < 0.5 {
                put(tuft(roll < 0.2, step), rng.range(0.9, 1.5), rng);
            } else if roll < 0.58 && patch2 > 0.4 {
                put(kit.flower[rng.below(4)], rng.range(0.8, 1.3), rng);
            }
        }
        Cls::Park(_) => {
            if woods {
                if roll < 0.2 {
                    put(bushi(rng), rng.range(0.8, 1.5), rng);
                } else if roll < 0.24 {
                    let l = kit.log[rng.below(2)];
                    put(l, rng.range(0.9, 1.3), rng);
                } else if roll < 0.28 {
                    put(kit.rock[rng.below(9)], rng.range(0.6, 1.4), rng);
                } else if roll < 0.7 {
                    put(tuft(roll < 0.4, step), rng.range(0.9, 1.5), rng);
                }
            } else if park_edge < 9.0 {
                if roll < 0.14 {
                    put(bushi(rng), rng.range(0.7, 1.2), rng);
                } else if roll < 0.4 {
                    put(tuft(false, step), rng.range(0.9, 1.4), rng);
                } else if roll < 0.46 {
                    put(kit.flower[rng.below(4)], rng.range(0.8, 1.2), rng);
                }
            }
        }
        _ => {
            if clear < 5.0 {
                // roadside verge
                if roll < 0.6 {
                    put(tuft(roll < 0.25, step), rng.range(0.9, 1.5), rng);
                }
                return;
            }
            let dens = 0.2 + 0.6 * patch;
            let rocky = (0.008 + 0.3 * (slope - 0.05).max(0.0)).min(0.2);
            if roll < rocky {
                put(kit.rock[rng.below(9)], rng.range(0.6, 1.8), rng);
            } else if roll < rocky + 0.035 {
                put(bushi(rng), rng.range(0.7, 1.4), rng);
            } else if patch2 > 0.7 && roll < rocky + 0.035 + 0.3 {
                put(kit.flower[rng.below(4)], rng.range(0.8, 1.4), rng);
            } else if roll < rocky + 0.035 + dens {
                put(tuft(patch2 > 0.55, step), rng.range(0.8, 1.5), rng);
            }
        }
    }
}

fn scatter(cx: &Ctx, kit: &Kit, group: Group, out: &mut Vec<Item>) {
    let t = cx.t;
    let (lo, hi, park, woods, layout) = match group {
        Group::Base => (pt(0.0, t.highway_y), pt(t.size, t.size), None, false, None),
        Group::Park(ai) => {
            let a = &t.additions[ai];
            let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
            for p in &a.polygon {
                lo = pt(lo.x.min(p.x), lo.y.min(p.y));
                hi = pt(hi.x.max(p.x), hi.y.max(p.y));
            }
            let dressed = a.pattern == Pattern::Park;
            let layout = if dressed {
                crate::park::layout(&a.polygon, t.seed, ai)
            } else {
                None
            };
            (
                lo,
                hi,
                Some(ai),
                crate::scene::wooded_ring(cx.land, &a.polygon),
                layout,
            )
        }
    };
    let (nx, ny) = (
        ((hi.x - lo.x) / CELL as f64).ceil() as i64,
        ((hi.y - lo.y) / CELL as f64).ceil() as i64,
    );
    for gy in 0..ny {
        for gx in 0..nx {
            let mut rng = Rng::new(cx.seed ^ 0x51, ((gy as u64) << 24) | gx as u64);
            let p = pt(
                lo.x + (gx as f64 + rng.f32() as f64) * CELL as f64,
                lo.y + (gy as f64 + rng.f32() as f64) * CELL as f64,
            );
            if group == Group::Base && !cx.interesting(p) {
                continue;
            }
            let cls = cx.classify(p);
            let mut edge = 100.0;
            match (group, cls) {
                (Group::Base, Cls::Open) => {}
                (Group::Base, Cls::Farm(ai)) => {
                    let d = ring_dist(p, &t.additions[ai].polygon);
                    if !(1.8..4.6).contains(&d) {
                        continue;
                    }
                }
                (Group::Park(a), Cls::Park(b)) if a == b => {
                    let ring = &t.additions[a].polygon;
                    edge = ring_dist(p, ring);
                    if edge < 1.5 {
                        continue;
                    }
                    if let Some(l) = &layout {
                        // dressed parks keep their paths, pond and play area clear
                        if edge > 8.0 || l.blocks(p) {
                            continue;
                        }
                    }
                }
                _ => continue,
            }
            if !cx.free(p, 1.2) {
                continue;
            }
            scatter_cell(
                cx,
                kit,
                cls,
                p,
                &mut rng,
                edge,
                woods && park.is_some(),
                out,
            );
        }
    }
}

/// Smallest bounding rectangle direction of a ring: the angle of its long side.
fn long_axis(ring: &[Pt]) -> f64 {
    let mut best = (f64::MAX, 0.0);
    for k in 0..60 {
        let a = k as f64 * std::f64::consts::PI / 60.0;
        let (c, s) = (libm::cos(a), libm::sin(a));
        let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in ring {
            let (u, v) = (p.x * c + p.y * s, -p.x * s + p.y * c);
            u0 = u0.min(u);
            u1 = u1.max(u);
            v0 = v0.min(v);
            v1 = v1.max(v);
        }
        let area = (u1 - u0) * (v1 - v0);
        // prefer the direction where the u extent is the longer side
        let score = area * if u1 - u0 >= v1 - v0 { 1.0 } else { 1.02 };
        if score < best.0 {
            best = (score, a);
        }
    }
    let (c, s) = (libm::cos(best.1), libm::sin(best.1));
    let (mut eu, mut ev) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
    for p in ring {
        let (u, v) = (p.x * c + p.y * s, -p.x * s + p.y * c);
        eu = (eu.0.min(u), eu.1.max(u));
        ev = (ev.0.min(v), ev.1.max(v));
    }
    if eu.1 - eu.0 >= ev.1 - ev.0 {
        best.1
    } else {
        best.1 + std::f64::consts::FRAC_PI_2
    }
}

fn rows_and_edges(cx: &Ctx, kit: &mut Kit, ai: usize, out: &mut Vec<Item>) {
    let t = cx.t;
    let ring = &t.additions[ai].polygon;
    if ring.len() < 3 {
        return;
    }
    let idx = field_index(t.seed, ai);
    let crop = crop_of(idx);
    let mut rng = Rng::new(cx.seed ^ 0xF1E1D, ai as u64);
    let step = (hash2(t.seed, ai as u64 + 99) % 4) as u8;

    // crop rows along the long axis
    let ang = long_axis(ring);
    let (c, s) = (libm::cos(ang), libm::sin(ang));
    let uv: Vec<(f64, f64)> = ring
        .iter()
        .map(|p| (p.x * c + p.y * s, -p.x * s + p.y * c))
        .collect();
    let (mut v0, mut v1) = (f64::MAX, f64::MIN);
    for q in &uv {
        v0 = v0.min(q.1);
        v1 = v1.max(q.1);
    }
    let spacing = if crop == Crop::Wheat { 4.0 } else { 5.0 };
    let mut v = v0 + 2.5;
    while v < v1 - 2.5 && crop != Crop::Hay {
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..uv.len() {
            let (a, b) = (uv[i], uv[(i + 1) % uv.len()]);
            if (a.1 - v) * (b.1 - v) < 0.0 {
                xs.push(a.0 + (b.0 - a.0) * (v - a.1) / (b.1 - a.1));
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for pair in xs.chunks_exact(2) {
            let (s0, s1) = (pair[0] + 2.8, pair[1] - 2.8);
            if s1 - s0 < 6.0 {
                continue;
            }
            let n = ((s1 - s0) / 24.0).ceil() as usize;
            let len = (s1 - s0) / n as f64;
            for k in 0..n {
                let u = s0 + len * (k as f64 + 0.5);
                let w = pt(u * c - v * s, u * s + v * c);
                let e0 = pt((u - len / 2.0) * c - v * s, (u - len / 2.0) * s + v * c);
                let e1 = pt((u + len / 2.0) * c - v * s, (u + len / 2.0) * s + v * c);
                if !(cx.close(w) && cx.free(w, 2.5) && cx.free(e0, 2.0) && cx.free(e1, 2.0)) {
                    continue;
                }
                if ring_dist(w, ring) < 2.4 {
                    continue;
                }
                let l = (len.round() as u16).max(4);
                let key = (l, crop as u8, step);
                let tpl = match kit.rows.get(&key) {
                    Some(&i) => i,
                    None => {
                        let col = match crop {
                            Crop::Green => m(GRASS_M, step),
                            Crop::Wheat => m(WHEAT_M, step % 2),
                            _ => [134 - step * 4, 98 - step * 3, 70],
                        };
                        let i = kit.add(row_piece(l as f32 - 0.4, col), false);
                        kit.rows.insert(key, i);
                        i
                    }
                };
                out.push(Item {
                    tpl,
                    x: w.x as f32,
                    y: w.y as f32,
                    theta: ang as f32,
                    scale: 1.0,
                });
            }
        }
        v += spacing;
    }

    // hay bales on a loose grid
    if crop == Crop::Hay {
        let mut v = v0 + 8.0;
        while v < v1 - 8.0 {
            let mut u = uv.iter().fold(f64::MAX, |a, q| a.min(q.0)) + 8.0;
            let umax = uv.iter().fold(f64::MIN, |a, q| a.max(q.0)) - 8.0;
            while u < umax {
                let (ju, jv) = (rng.f32() as f64 * 6.0, rng.f32() as f64 * 6.0);
                let (uu, vv) = (u + ju, v + jv);
                let w = pt(uu * c - vv * s, uu * s + vv * c);
                if in_ring(w, ring)
                    && cx.close(w)
                    && ring_dist(w, ring) > 5.0
                    && cx.free(w, 3.0)
                    && rng.f32() < 0.5
                {
                    out.push(Item {
                        tpl: kit.bale,
                        x: w.x as f32,
                        y: w.y as f32,
                        theta: rng.range(0.0, std::f32::consts::TAU),
                        scale: rng.range(0.9, 1.15),
                    });
                }
                u += 26.0;
            }
            v += 24.0;
        }
    }

    // the boundary: hedge, dry stone wall, fence or a tree line, or open
    let style = hash2(t.seed ^ 0xED6E, ai as u64) % 6;
    if style >= 5 {
        return;
    }
    let mut gap = false;
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[(i + 1) % ring.len()]);
        let l = a.dist(b);
        if l < 3.0 {
            continue;
        }
        let n = (l / 10.0).ceil() as usize;
        let theta = libm::atan2(b.y - a.y, b.x - a.x) as f32;
        for k in 0..n {
            let (p0, p1) = (
                a.lerp(b, k as f64 / n as f64),
                a.lerp(b, (k + 1) as f64 / n as f64),
            );
            let mid = p0.lerp(p1, 0.5);
            let plen = (l / n as f64) as f32;
            if !(cx.free(mid, 3.0) && cx.free(p0, 2.5) && cx.free(p1, 2.5)) {
                continue;
            }
            // a gate gap now and then
            if !gap && rng.f32() < 0.07 {
                gap = true;
                continue;
            }
            gap = false;
            let obj = match style {
                0 | 3 | 4 => hedge_piece(
                    plen - 0.3,
                    rng.range(1.0, 1.5),
                    rng.below(4) as u8,
                    rng.chance(0.4),
                ),
                1 => wall_piece(plen - 0.2),
                _ => fence_piece(plen - 0.2),
            };
            let tpl = kit.add(obj, true);
            out.push(Item {
                tpl,
                x: mid.x as f32,
                y: mid.y as f32,
                theta,
                scale: 1.0,
            });
            if (style == 3 || style == 4) && (k + i) % 2 == 0 {
                let ti = kit.tree[rng.below(3) * 8 + rng.below(2) * 4 + rng.below(4)];
                out.push(Item {
                    tpl: ti,
                    x: mid.x as f32 + rng.range(-1.0, 1.0),
                    y: mid.y as f32 + rng.range(-1.0, 1.0),
                    theta: 0.0,
                    scale: 1.0,
                });
            }
        }
    }
}

/// Index into `scene::FIELDS` of a farm addition's colour.
pub fn field_index(seed: u64, ai: usize) -> usize {
    (hash2(seed, ai as u64) % crate::scene::FIELDS.len() as u64) as usize
}

/// Soft tall grass strip along the inside of a field boundary.
pub const MARGIN: Rgb = [136, 170, 84];

pub fn field_margin(g: &mut crate::ground::Ground, ring: &[Pt]) {
    if ring.len() < 3 {
        return;
    }
    let mut line: Vec<Pt> = ring.to_vec();
    line.push(ring[0]);
    let side = if crate::ground::signed_area(ring) > 0.0 {
        2.3
    } else {
        -2.3
    };
    g.ribbon(&line, 1.9, side, crate::ground::layer::ZONE + 0.02, MARGIN);
}

// ---- assembly ----

fn finish(items: Vec<Item>, kit: &Kit, land: &Land) -> Vec<Chunk> {
    let mut cells: BTreeMap<(i32, i32), Vec<usize>> = BTreeMap::new();
    for (i, it) in items.iter().enumerate() {
        cells
            .entry(((it.x / CHUNK).floor() as i32, (it.y / CHUNK).floor() as i32))
            .or_default()
            .push(i);
    }
    let mut chunks = Vec::new();
    for (ci, (_, members)) in cells.iter().enumerate() {
        let mut mesh = Mesh::default();
        let mut far = Mesh::default();
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        let base = ID_LO + ((ci as u32 * 4096) % ID_SPAN);
        for (k, &ii) in members.iter().enumerate() {
            let it = &items[ii];
            let tpl = &kit.tpls[it.tpl];
            let id = base + (k as u32 % 4096) + 1;
            let z = land.height(it.x, it.y);
            let sit = (!land.is_flat()).then_some(Sit { land, z });
            let (s, c) = (libm::sinf(it.theta), libm::cosf(it.theta));
            let mut pts: Vec<[f32; 3]> = Vec::with_capacity(8);
            for p in &tpl.obj.polys {
                pts.clear();
                pts.extend(p.pts.iter().map(|v| {
                    let wx = it.x + it.scale * (c * v[0] - s * v[1]);
                    let wy = it.y + it.scale * (s * v[0] + c * v[1]);
                    let lz = it.scale * v[2];
                    [wx, wy, sit.map_or(lz, |st| st.place(wx, wy, lz, v[2]))]
                }));
                mesh.push_poly(
                    &pts,
                    p.color,
                    p.flat,
                    p.shadow,
                    if p.outline { id } else { ID_NONE },
                );
            }
            if tpl.far {
                tpl.obj.emit_far_on(&mut far, it.x, it.y, it.theta, sit);
            }
            let reach = 14.0;
            lo = lo.min(Vec3::new(
                it.x - reach,
                it.y - reach,
                land.min_height().min(0.0),
            ));
            hi = hi.max(Vec3::new(
                it.x + reach,
                it.y + reach,
                land.max_height() + 14.0,
            ));
        }
        chunks.push(Chunk {
            rev: 0,
            mesh,
            far: Some(far),
            lo,
            hi,
        });
    }
    chunks
}

/// Ground cover chunks for one group of the town.
pub fn build(t: &Town, land: &Land, group: Group) -> Vec<Chunk> {
    let cx = Ctx::new(t, land);
    let mut kit = Kit::new();
    let mut items = Vec::new();
    scatter(&cx, &kit, group, &mut items);
    if group == Group::Base {
        for (ai, a) in t.additions.iter().enumerate() {
            if a.zone == Zone::Farm {
                rows_and_edges(&cx, &mut kit, ai, &mut items);
            }
        }
    }
    if std::env::var("US_COVER_STATS").is_ok() {
        let mut by: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
        for it in &items {
            let e = by.entry(it.tpl).or_default();
            e.0 += 1;
            e.1 = kit.tpls[it.tpl]
                .obj
                .polys
                .iter()
                .map(|p| p.pts.len().saturating_sub(2))
                .sum();
        }
        let mut cat = [(0usize, 0usize); 8];
        for (t, (n, tri)) in by {
            let c = if t < 8 {
                0
            } else if t < 32 {
                1
            } else if t < 36 {
                2
            } else if t < 38 {
                3
            } else if t < 47 {
                4
            } else if t < 50 {
                5
            } else {
                6
            };
            cat[c].0 += n;
            cat[c].1 += n * tri;
        }
        for (name, f) in [
            ("tuft", 0usize..8),
            ("bush", 8..32),
            ("flower", 32..36),
            ("reed", 36..38),
            ("rock", 38..47),
            ("bale", 49..50),
        ] {
            if let Some(it) = items.iter().find(|i| f.contains(&i.tpl)) {
                eprintln!("spot {name} {:.0} {:.0}", it.x, it.y);
            }
        }
        if let Some(it) = items
            .iter()
            .find(|i| kit.tpls[i.tpl].far && i.tpl >= 50 && !kit.tree.contains(&i.tpl))
        {
            eprintln!("spot hedge {:.0} {:.0}", it.x, it.y);
        }
        if let Some(it) = items
            .iter()
            .find(|i| kit.rows.values().any(|v| *v == i.tpl))
        {
            eprintln!("spot row {:.0} {:.0}", it.x, it.y);
        }
        let (mut nr, mut nh, mut nt, mut th, mut tr) = (0, 0, 0, 0, 0);
        for it in &items {
            let tr_ = kit.tpls[it.tpl]
                .obj
                .polys
                .iter()
                .map(|p| p.pts.len().saturating_sub(2))
                .sum::<usize>();
            if kit.rows.values().any(|v| *v == it.tpl) {
                nr += 1;
                tr += tr_;
            } else if kit.tree.contains(&it.tpl) {
                nt += 1;
            } else if it.tpl >= 50 {
                nh += 1;
                th += tr_;
            }
        }
        eprintln!("rows {nr} ({tr} tris) hedges {nh} ({th} tris) trees {nt}");
        eprintln!(
            "cover items/tris: tuft {:?} bush {:?} flower {:?} reed {:?} rock {:?} log+bale {:?} other {:?}",
            cat[0], cat[1], cat[2], cat[3], cat[4], cat[5], cat[6]
        );
    }
    finish(items, &kit, land)
}

/// Every chunk of cover a static scene draws: the base and every park.
pub fn build_all(t: &Town, land: &Land) -> Vec<Chunk> {
    let mut v = build(t, land, Group::Base);
    for (ai, a) in t.additions.iter().enumerate() {
        if a.zone == Zone::Park {
            v.extend(build(t, land, Group::Park(ai)));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::{Preset, generate};

    #[test]
    fn cover_is_deterministic_and_keeps_off_roads() {
        let t = generate(Preset::LakeTown, 2);
        let land = Land::from_town(&t);
        let a = build_all(&t, &land);
        let b = build_all(&t, &land);
        assert!(!a.is_empty());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.mesh.vertices.len(), y.mesh.vertices.len());
            assert_eq!(x.mesh.cast, y.mesh.cast);
        }
        // every item stands clear of every road and lot
        let roads = RoadGrid::new(&t);
        let cx = Ctx::new(&t, &land);
        let mut items = Vec::new();
        let kit = Kit::new();
        scatter(&cx, &kit, Group::Base, &mut items);
        assert!(!items.is_empty());
        for it in &items {
            let p = pt(it.x as f64, it.y as f64);
            assert!(roads.clearance(p) > 0.5, "item on a road at {p:?}");
            assert!(!cx.near_lot(p, 1.0), "item inside a lot at {p:?}");
            assert!(!cx.water(p), "item in water at {p:?}");
        }
    }
}
