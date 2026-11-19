//! Woodland: forests, copses and lone trees over the open country around the town.
//!
//! The trees are planted from the town seed alone, so a saved town reopens with the same woods.
//! They stand on land nobody has built on; the game clears them where a street, a lot or the
//! town hall appears (`clear_disc`, `clear_along`) and redraws the chunks that changed.
//!
//! Crowns are rounded: each is a few rings with normals that point away from the crown's
//! centre, so the light falls off smoothly from the sunlit top to the shaded underside.

use crate::bough;
use crate::kit::{TREE, TREE_D};
use crate::kit_more::PINE;
use crate::land::Land;
use crate::mesh::{ID_NONE, Mesh, Vertex};
use crate::obj::Rgb;
use crate::rng::Rng;
use crate::scene::Chunk;
use glam::Vec3;
use std::collections::{BTreeMap, BTreeSet};
use us_gen::{Preset, Pt, Town, Zone, pt};

/// Side of a woodland chunk, metres.
pub const CHUNK: f32 = 256.0;
/// Side of a cell of the lookup grid used when clearing, metres.
const BUCKET: f32 = 32.0;
/// Spacing of the planting grid, metres.
const STEP: f32 = 8.0;
const TRUNK: Rgb = [104, 78, 56];

pub type Cell = (i32, i32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Broadleaf,
    Tall,
    Conifer,
}

#[derive(Clone, Copy, Debug)]
struct Tree {
    x: f32,
    y: f32,
    z: f32,
    /// Crown radius and full height, metres.
    r: f32,
    h: f32,
    turn: f32,
    kind: Kind,
    colour: Rgb,
    standing: bool,
}

/// How much of the open land a town style keeps wooded, and how much of that is conifer.
fn climate(p: Preset) -> (f32, f32) {
    // the second number is the share of the forest that is conifer: different for every style,
    // so forests do not all carry the same mix, and every forest keeps some green all winter
    match p {
        Preset::Grid => (0.26, 0.24),
        Preset::Winding => (0.36, 0.31),
        Preset::Sunbelt => (0.07, 0.22),
        Preset::Crossroads => (0.16, 0.27),
        Preset::LakeTown => (0.34, 0.40),
        Preset::Gorge => (0.34, 0.52),
        Preset::Mesa => (0.02, 0.21),
        Preset::Valley => (0.40, 0.66),
        Preset::Coast => (0.24, 0.34),
    }
}

fn lattice(seed: u64, i: i64, j: i64) -> f32 {
    Rng::new(
        seed ^ 0x0077_00D5,
        ((i as u64 & 0xF_FFFF) << 24) | (j as u64 & 0xFF_FFFF),
    )
    .f32()
}

/// Smooth value noise in 0 to 1.
fn noise(seed: u64, x: f32, y: f32) -> f32 {
    let (fx, fy) = (libm::floorf(x), libm::floorf(y));
    let (i, j) = (fx as i64, fy as i64);
    let (tx, ty) = (x - fx, y - fy);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let a = lattice(seed, i, j);
    let b = lattice(seed, i + 1, j);
    let c = lattice(seed, i, j + 1);
    let d = lattice(seed, i + 1, j + 1);
    (a + (b - a) * sx) + ((c + (d - c) * sx) - (a + (b - a) * sx)) * sy
}

/// Where forest wants to stand, 0 to 1: broad masses with ragged edges.
fn forest_field(seed: u64, x: f32, y: f32) -> f32 {
    0.50 * noise(seed, x / 340.0, y / 340.0)
        + 0.32 * noise(seed ^ 11, x / 130.0 + 31.0, y / 130.0)
        + 0.18 * noise(seed ^ 23, x / 48.0, y / 48.0 + 17.0)
}

fn in_ring(p: Pt, ring: &[Pt]) -> bool {
    let mut inside = false;
    let mut j = ring.len().wrapping_sub(1);
    for i in 0..ring.len() {
        let (a, b) = (ring[i], ring[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// The woods of one town.
pub struct Woods {
    trees: Vec<Tree>,
    buckets: BTreeMap<Cell, Vec<u32>>,
    chunks: BTreeMap<Cell, Vec<u32>>,
    z_lo: f32,
    z_hi: f32,
}

impl Woods {
    /// Plant the open land of `t`. Fields, parks, water, steep rock and bare summits stay clear.
    pub fn plant(t: &Town, land: &Land) -> Woods {
        let seed = t.seed ^ 0x57_00D5;
        let (cover, conifer) = climate(t.preset);
        let size = t.size as f32;
        let rock = land.rock();
        // forest stands where the field passes this level; the level is set so that about
        // `cover` of the land does
        let level = 0.5 + (0.5 - cover) * 0.40;
        struct Keep {
            lo: Pt,
            hi: Pt,
            idx: usize,
        }
        let keep: Vec<Keep> = t
            .additions
            .iter()
            .enumerate()
            .filter(|(_, a)| matches!(a.zone, Zone::Farm | Zone::Park))
            .map(|(idx, a)| {
                let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
                for p in &a.polygon {
                    lo = pt(lo.x.min(p.x), lo.y.min(p.y));
                    hi = pt(hi.x.max(p.x), hi.y.max(p.y));
                }
                Keep { lo, hi, idx }
            })
            .collect();
        let n = (size / STEP) as i64;
        let mut trees = Vec::new();
        for gy in 0..n {
            for gx in 0..n {
                let mut rng = Rng::new(seed, ((gy as u64) << 24) | gx as u64);
                let x = (gx as f32 + rng.f32()) * STEP;
                let y = (gy as f32 + rng.f32()) * STEP;
                let roll = rng.f32();
                let field = forest_field(seed, x, y);
                let copse = noise(seed ^ 41, x / 46.0, y / 46.0);
                // dense inside a forest, thinning to nothing across its ragged edge; a few
                // copses and lone trees stand in the open
                let inside = ((field - level) / 0.045).clamp(0.0, 1.0);
                let chance = 0.86 * inside
                    + if copse > 0.78 {
                        0.42 * cover.min(0.3) / 0.3
                    } else {
                        0.0
                    }
                    + 0.016 * (cover / 0.3).min(1.0);
                if roll > chance {
                    continue;
                }
                if x < 6.0 || y < 6.0 || x > size - 6.0 || y > size - 6.0 {
                    continue;
                }
                let p = pt(x as f64, y as f64);
                if land.water_at(p).is_some() || land.wet_shelf(x, y) {
                    continue;
                }
                let near_water = [(5.0, 0.0), (-5.0, 0.0), (0.0, 5.0), (0.0, -5.0)]
                    .iter()
                    .any(|d| land.water_at(pt(p.x + d.0, p.y + d.1)).is_some());
                if near_water {
                    continue;
                }
                let slope = land.slope(x, y);
                let z = land.height(x, y);
                if slope > 0.62 {
                    continue;
                }
                let mut high = 0.0;
                if let Some(rock) = rock {
                    if z > rock.tree_line - 25.0 {
                        continue;
                    }
                    // conifers take over toward the tree line
                    high = ((z - (rock.tree_line - 190.0)) / 150.0).clamp(0.0, 1.0);
                }
                if keep.iter().any(|k| {
                    p.x >= k.lo.x
                        && p.x <= k.hi.x
                        && p.y >= k.lo.y
                        && p.y <= k.hi.y
                        && in_ring(p, &t.additions[k.idx].polygon)
                }) {
                    continue;
                }
                // stands of one kind: conifers gather on their own ground
                let stand = noise(seed ^ 67, x / 150.0, y / 150.0);
                let pine_share = (conifer + 0.9 * high + 0.25 * (slope - 0.2).max(0.0)).min(0.97);
                // `mixed` is spread roughly evenly over 0 to 1, so the share is about the share
                let mixed = (0.5 + (stand * 0.6 + rng.f32() * 0.4 - 0.5) / 0.56).clamp(0.0, 1.0);
                let kind = if mixed < pine_share {
                    Kind::Conifer
                } else if rng.f32() < 0.2 {
                    Kind::Tall
                } else {
                    Kind::Broadleaf
                };
                // taller in the heart of a forest, smaller at the edge and in the open
                let grown = 0.72 + 0.34 * inside + rng.range(-0.14, 0.2);
                let (r, h) = match kind {
                    Kind::Broadleaf => (rng.range(3.1, 4.5) * grown, rng.range(8.5, 12.5) * grown),
                    Kind::Tall => (rng.range(2.3, 3.1) * grown, rng.range(12.0, 16.5) * grown),
                    Kind::Conifer => (rng.range(2.2, 3.2) * grown, rng.range(11.0, 17.5) * grown),
                };
                let colour = match kind {
                    Kind::Conifer => PINE,
                    _ => {
                        let base = if rng.f32() < 0.42 { TREE_D } else { TREE };
                        [base[0], base[1], base[2] + rng.below(4) as u8]
                    }
                };
                trees.push(Tree {
                    x,
                    y,
                    z,
                    r,
                    h,
                    turn: rng.range(0.0, std::f32::consts::TAU),
                    kind,
                    colour,
                    standing: true,
                });
            }
        }
        let mut buckets: BTreeMap<Cell, Vec<u32>> = BTreeMap::new();
        let mut chunks: BTreeMap<Cell, Vec<u32>> = BTreeMap::new();
        for (i, tr) in trees.iter().enumerate() {
            buckets
                .entry((
                    libm::floorf(tr.x / BUCKET) as i32,
                    libm::floorf(tr.y / BUCKET) as i32,
                ))
                .or_default()
                .push(i as u32);
            chunks
                .entry((
                    libm::floorf(tr.x / CHUNK) as i32,
                    libm::floorf(tr.y / CHUNK) as i32,
                ))
                .or_default()
                .push(i as u32);
        }
        Woods {
            trees,
            buckets,
            chunks,
            z_lo: land.min_height().min(0.0),
            z_hi: land.max_height() + 24.0,
        }
    }

    /// Trees still standing.
    pub fn standing(&self) -> usize {
        self.trees.iter().filter(|t| t.standing).count()
    }

    /// Every chunk cell that has ever held a tree.
    pub fn cells(&self) -> Vec<Cell> {
        self.chunks.keys().copied().collect()
    }

    fn clear_where(
        &mut self,
        lo: Pt,
        hi: Pt,
        hit: impl Fn(f64, f64, f64) -> bool,
    ) -> BTreeSet<Cell> {
        let mut changed = BTreeSet::new();
        let (bx0, by0) = (
            libm::floor(lo.x / BUCKET as f64) as i32,
            libm::floor(lo.y / BUCKET as f64) as i32,
        );
        let (bx1, by1) = (
            libm::floor(hi.x / BUCKET as f64) as i32,
            libm::floor(hi.y / BUCKET as f64) as i32,
        );
        for by in by0..=by1 {
            for bx in bx0..=bx1 {
                let Some(list) = self.buckets.get(&(bx, by)) else {
                    continue;
                };
                for &i in list {
                    let tr = &mut self.trees[i as usize];
                    if tr.standing && hit(tr.x as f64, tr.y as f64, tr.r as f64) {
                        tr.standing = false;
                        changed.insert((
                            libm::floorf(tr.x / CHUNK) as i32,
                            libm::floorf(tr.y / CHUNK) as i32,
                        ));
                    }
                }
            }
        }
        changed
    }

    /// Fell every tree whose crown reaches within `radius` of `c`. Returns the chunks to redraw.
    pub fn clear_disc(&mut self, c: Pt, radius: f64) -> BTreeSet<Cell> {
        let reach = radius + 6.0;
        self.clear_where(
            pt(c.x - reach, c.y - reach),
            pt(c.x + reach, c.y + reach),
            |x, y, r| {
                let (dx, dy) = (x - c.x, y - c.y);
                let d = radius + r * 0.8;
                dx * dx + dy * dy < d * d
            },
        )
    }

    /// Fell every tree whose crown reaches within `half` of the segment `a` to `b`.
    pub fn clear_along(&mut self, a: Pt, b: Pt, half: f64) -> BTreeSet<Cell> {
        let reach = half + 6.0;
        let (abx, aby) = (b.x - a.x, b.y - a.y);
        let len2 = (abx * abx + aby * aby).max(1e-9);
        self.clear_where(
            pt(a.x.min(b.x) - reach, a.y.min(b.y) - reach),
            pt(a.x.max(b.x) + reach, a.y.max(b.y) + reach),
            |x, y, r| {
                let t = (((x - a.x) * abx + (y - a.y) * aby) / len2).clamp(0.0, 1.0);
                let (dx, dy) = (x - (a.x + abx * t), y - (a.y + aby * t));
                let d = half + r * 0.8;
                dx * dx + dy * dy < d * d
            },
        )
    }

    /// Thin the trees inside `ring` to about `keep` of them (land being built up keeps a few
    /// old trees between the houses). Which trees stay depends only on the tree.
    pub fn thin_ring(&mut self, ring: &[Pt], keep: f32) -> BTreeSet<Cell> {
        let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
        for p in ring {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
        self.clear_where(lo, hi, |x, y, _| {
            let stays =
                Rng::new(0x7417, ((x * 16.0) as u64) << 24 | (y * 16.0) as u64).f32() < keep;
            !stays && in_ring(pt(x, y), ring)
        })
    }

    /// The chunk of one cell as it stands now (empty meshes when every tree is felled).
    pub fn chunk(&self, cell: Cell, rev: u64) -> Chunk {
        let mut mesh = Mesh::default();
        let mut far = Mesh::default();
        for &i in self.chunks.get(&cell).into_iter().flatten() {
            let tr = &self.trees[i as usize];
            if tr.standing {
                near_tree(&mut mesh, tr);
                far_tree(&mut far, tr);
            }
        }
        let lo = Vec3::new(
            cell.0 as f32 * CHUNK - 8.0,
            cell.1 as f32 * CHUNK - 8.0,
            self.z_lo,
        );
        let hi = Vec3::new(
            (cell.0 + 1) as f32 * CHUNK + 8.0,
            (cell.1 + 1) as f32 * CHUNK + 8.0,
            self.z_hi,
        );
        Chunk {
            rev,
            mesh,
            far: Some(far),
            lo,
            hi,
        }
    }
}

// ---- models ----

fn vertex(mesh: &mut Mesh, pos: [f32; 3], normal: Vec3, colour: Rgb) -> u32 {
    let n = normal.normalize_or_zero();
    mesh.vertices.push(Vertex {
        pos,
        normal: [n.x, n.y, n.z],
        color: [colour[0], colour[1], colour[2], 255],
        id: ID_NONE,
    });
    mesh.vertices.len() as u32 - 1
}

/// A rounded crown lobe: rings from `levels` (height share, radius share) closed by a point at
/// the top and one underneath. Normals point away from the lobe's centre.
#[allow(clippy::too_many_arguments)]
fn lobe(
    mesh: &mut Mesh,
    c: Vec3,
    radius: f32,
    height: f32,
    sides: usize,
    levels: &[(f32, f32)],
    turn: f32,
    wobble: f32,
    colour: Rgb,
) {
    let centre = c + Vec3::new(0.0, 0.0, height * 0.42);
    let mut rings: Vec<Vec<u32>> = Vec::with_capacity(levels.len());
    for (li, &(hs, rs)) in levels.iter().enumerate() {
        let mut ring = Vec::with_capacity(sides);
        for s in 0..sides {
            let a =
                turn + std::f32::consts::TAU * (s as f32 + 0.5 * (li % 2) as f32) / sides as f32;
            let bulge = 1.0 + wobble * libm::sinf(2.0 * a + 1.7 * li as f32 + turn * 3.0);
            let p = c + Vec3::new(
                radius * rs * bulge * libm::cosf(a),
                radius * rs * bulge * libm::sinf(a),
                height * hs,
            );
            let mut n = p - centre;
            n.z = n.z * (radius / height.max(0.1)).min(1.4) + 0.18 * radius;
            ring.push(vertex(mesh, p.to_array(), n, colour));
        }
        rings.push(ring);
    }
    let top = vertex(
        mesh,
        (c + Vec3::new(0.0, 0.0, height)).to_array(),
        Vec3::Z,
        colour,
    );
    let under = vertex(
        mesh,
        (c + Vec3::new(0.0, 0.0, height * levels[0].0 * 0.55)).to_array(),
        Vec3::new(0.0, 0.0, -1.0),
        colour,
    );
    for s in 0..sides {
        let t = (s + 1) % sides;
        mesh.cast
            .extend_from_slice(&[under, rings[0][t], rings[0][s]]);
        for l in 0..rings.len() - 1 {
            let (a, b, c2, d) = (rings[l][s], rings[l][t], rings[l + 1][t], rings[l + 1][s]);
            mesh.cast.extend_from_slice(&[a, b, c2, a, c2, d]);
        }
        let last = rings.len() - 1;
        mesh.cast
            .extend_from_slice(&[rings[last][s], rings[last][t], top]);
    }
}

fn trunk(mesh: &mut Mesh, tr: &Tree, width: f32, height: f32) {
    let base = Vec3::new(tr.x, tr.y, tr.z - 0.4);
    let mut low = [0u32; 3];
    let mut high = [0u32; 3];
    for k in 0..3 {
        let a = tr.turn + std::f32::consts::TAU * k as f32 / 3.0;
        let out = Vec3::new(libm::cosf(a), libm::sinf(a), 0.0);
        low[k] = vertex(mesh, (base + out * width).to_array(), out, TRUNK);
        high[k] = vertex(
            mesh,
            (base + out * width * 0.7 + Vec3::new(0.0, 0.0, height + 0.4)).to_array(),
            out,
            TRUNK,
        );
    }
    for k in 0..3 {
        let t = (k + 1) % 3;
        mesh.cast
            .extend_from_slice(&[low[k], low[t], high[t], low[k], high[t], high[k]]);
    }
}

fn near_tree(mesh: &mut Mesh, tr: &Tree) {
    let at = Vec3::new(tr.x, tr.y, tr.z);
    match tr.kind {
        Kind::Broadleaf => {
            bough_tree(mesh, tr, false);
            let crown = at + Vec3::new(0.0, 0.0, tr.h * 0.24);
            lobe(
                mesh,
                crown,
                tr.r,
                tr.h * 0.76,
                6,
                &[(0.16, 0.62), (0.42, 1.0), (0.72, 0.78)],
                tr.turn,
                0.10,
                tr.colour,
            );
            // a second, smaller mass to one side breaks the round outline
            let side = Vec3::new(libm::cosf(tr.turn), libm::sinf(tr.turn), 0.0) * tr.r * 0.52;
            lobe(
                mesh,
                crown + side + Vec3::new(0.0, 0.0, tr.h * 0.02),
                tr.r * 0.66,
                tr.h * 0.5,
                5,
                &[(0.45, 1.0)],
                tr.turn + 0.9,
                0.06,
                tr.colour,
            );
        }
        Kind::Tall => {
            bough_tree(mesh, tr, false);
            lobe(
                mesh,
                at + Vec3::new(0.0, 0.0, tr.h * 0.2),
                tr.r,
                tr.h * 0.8,
                6,
                &[(0.12, 0.6), (0.4, 1.0), (0.74, 0.7)],
                tr.turn,
                0.08,
                tr.colour,
            );
        }
        Kind::Conifer => {
            trunk(mesh, tr, 0.24, tr.h * 0.24);
            for k in 0..3 {
                let f = k as f32;
                cone(
                    mesh,
                    at + Vec3::new(0.0, 0.0, tr.h * (0.16 + 0.25 * f)),
                    tr.r * (1.0 - 0.27 * f),
                    tr.h * (0.42 - 0.03 * f),
                    5,
                    tr.turn + 0.5 * f,
                    tr.colour,
                );
            }
        }
    }
}

/// The bare skeleton of a broadleaf tree: trunk, limbs and twigs in bark colour. It sits inside
/// the crown all year; in winter the shader folds the crown away and this is what shows. The far
/// form keeps only the trunk and the main limbs, each ending in a point.
fn bough_tree(mesh: &mut Mesh, tr: &Tree, far: bool) {
    let seed = bough::seed_at(tr.x, tr.y);
    let habit = bough::pick(seed, tr.kind == Kind::Tall);
    let base = Vec3::new(tr.x, tr.y, tr.z - 0.35);
    let mut limbs = 0;
    for mut t in bough::skeleton(habit, tr.r, tr.h, seed) {
        if far {
            // a pointed trunk and up to two pointed limbs: nine triangles at most
            if t.level == 2 {
                continue;
            }
            if t.level == 1 {
                limbs += 1;
                if limbs > 2 {
                    continue;
                }
            }
            t.rb = 0.0;
        }
        t.a += base;
        t.b += base;
        for (pts, n) in bough::faces(&t) {
            let ids: Vec<u32> = pts
                .iter()
                .map(|p| vertex(mesh, p.to_array(), n, bough::BARK))
                .collect();
            for k in 1..ids.len() - 1 {
                mesh.cast.extend_from_slice(&[ids[0], ids[k], ids[k + 1]]);
            }
        }
    }
}

/// An open cone skirt with normals leaning outward and up.
fn cone(
    mesh: &mut Mesh,
    base: Vec3,
    radius: f32,
    height: f32,
    sides: usize,
    turn: f32,
    colour: Rgb,
) {
    let tip = vertex(
        mesh,
        (base + Vec3::new(0.0, 0.0, height)).to_array(),
        Vec3::Z,
        colour,
    );
    let mut ring = Vec::with_capacity(sides);
    for s in 0..sides {
        let a = turn + std::f32::consts::TAU * s as f32 / sides as f32;
        let out = Vec3::new(libm::cosf(a), libm::sinf(a), 0.0);
        ring.push(vertex(
            mesh,
            (base + out * radius).to_array(),
            out + Vec3::new(0.0, 0.0, 0.55),
            colour,
        ));
    }
    for s in 0..sides {
        mesh.cast
            .extend_from_slice(&[ring[s], ring[(s + 1) % sides], tip]);
    }
}

/// The far form: one small rounded mass, or one cone.
fn far_tree(mesh: &mut Mesh, tr: &Tree) {
    let at = Vec3::new(tr.x, tr.y, tr.z);
    match tr.kind {
        Kind::Conifer => cone(
            mesh,
            at + Vec3::new(0.0, 0.0, tr.h * 0.12),
            tr.r,
            tr.h * 0.88,
            5,
            tr.turn,
            tr.colour,
        ),
        _ => {
            bough_tree(mesh, tr, true);
            // the far crown carries a colour family of its own (blue + 8): in leaf it is the
            // same crown, and with the leaves down the shader shows it as a thin haze of twigs
            // instead of folding it away, so distant bare woods do not shimmer as single lines
            let hazy = [tr.colour[0], tr.colour[1], tr.colour[2] + 8];
            let c = at + Vec3::new(0.0, 0.0, tr.h * 0.2);
            let (height, sides) = (tr.h * 0.8, 5);
            let centre = c + Vec3::new(0.0, 0.0, height * 0.4);
            let top = vertex(
                mesh,
                (c + Vec3::new(0.0, 0.0, height)).to_array(),
                Vec3::Z,
                hazy,
            );
            let mut ring = Vec::with_capacity(sides);
            let mut foot = Vec::with_capacity(sides);
            for s in 0..sides {
                let a = tr.turn + std::f32::consts::TAU * s as f32 / sides as f32;
                let out = Vec3::new(libm::cosf(a), libm::sinf(a), 0.0);
                let p = c + out * tr.r + Vec3::new(0.0, 0.0, height * 0.45);
                ring.push(vertex(mesh, p.to_array(), p - centre + Vec3::Z * 0.6, hazy));
                let q = c + out * tr.r * 0.5;
                foot.push(vertex(mesh, q.to_array(), out - Vec3::Z * 0.5, hazy));
            }
            for s in 0..sides {
                let t = (s + 1) % sides;
                mesh.cast.extend_from_slice(&[ring[s], ring[t], top]);
                mesh.cast
                    .extend_from_slice(&[foot[s], foot[t], ring[t], foot[s], ring[t], ring[s]]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use us_gen::generate;

    #[test]
    fn woods_are_the_same_for_the_same_town_and_clear_for_building() {
        let town = generate(Preset::Grid, 3);
        let land = Land::from_town(&town);
        let mut a = Woods::plant(&town, &land);
        let b = Woods::plant(&town, &land);
        assert!(
            a.standing() > 2000,
            "a grid town has woods: {}",
            a.standing()
        );
        assert_eq!(a.standing(), b.standing());
        for (x, y) in a.trees.iter().zip(&b.trees) {
            assert_eq!((x.x, x.y, x.r, x.h), (y.x, y.y, y.r, y.h));
        }
        let first = a.trees.iter().find(|t| t.standing).copied().unwrap();
        let at = pt(first.x as f64, first.y as f64);
        let before = a.standing();
        let changed = a.clear_disc(at, 20.0);
        assert!(!changed.is_empty());
        assert!(a.standing() < before);
        assert!(
            a.trees
                .iter()
                .all(|t| { !t.standing || pt(t.x as f64, t.y as f64).dist(at) >= 20.0 })
        );
        // clearing again changes nothing
        assert!(a.clear_disc(at, 20.0).is_empty());
        let before = a.standing();
        let far = pt(at.x + 300.0, at.y + 40.0);
        a.clear_along(at, far, 9.0);
        assert!(a.standing() <= before);
        assert!(a.trees.iter().all(|t| {
            let p = pt(t.x as f64, t.y as f64);
            let s = ((p.x - at.x) * (far.x - at.x) + (p.y - at.y) * (far.y - at.y))
                / at.dist(far).powi(2);
            !t.standing || p.dist(at.lerp(far, s.clamp(0.0, 1.0))) >= 9.0
        }));
    }

    #[test]
    fn no_tree_stands_in_water_or_on_a_field() {
        for preset in [Preset::LakeTown, Preset::Coast, Preset::Crossroads] {
            let town = generate(preset, 2);
            let land = Land::from_town(&town);
            let woods = Woods::plant(&town, &land);
            for t in &woods.trees {
                let p = pt(t.x as f64, t.y as f64);
                assert!(land.water_at(p).is_none(), "{preset:?}: tree in water");
                for a in &town.additions {
                    if matches!(a.zone, Zone::Farm | Zone::Park) {
                        assert!(
                            !in_ring(p, &a.polygon),
                            "{preset:?}: tree on a field or park"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn chunk_meshes_are_well_formed() {
        let town = generate(Preset::Valley, 2);
        let land = Land::from_town(&town);
        let woods = Woods::plant(&town, &land);
        let mut near = 0;
        let mut far = 0;
        for cell in woods.cells() {
            let c = woods.chunk(cell, 1);
            let n = c.mesh.vertices.len() as u32;
            assert!(c.mesh.cast.iter().all(|&i| i < n));
            let f = c.far.as_ref().unwrap();
            assert!(f.cast.iter().all(|&i| i < f.vertices.len() as u32));
            near += c.mesh.triangles();
            far += f.triangles();
        }
        assert!(
            far * 4 < near,
            "the far form is much lighter: {far} against {near}"
        );
    }

    #[test]
    fn every_style_keeps_some_evergreens_and_the_mix_differs() {
        let mut shares = Vec::new();
        for preset in [
            Preset::Grid,
            Preset::Winding,
            Preset::Crossroads,
            Preset::LakeTown,
            Preset::Gorge,
            Preset::Valley,
            Preset::Coast,
        ] {
            let town = generate(preset, 3);
            let land = Land::from_town(&town);
            let woods = Woods::plant(&town, &land);
            let standing: Vec<_> = woods.trees.iter().filter(|t| t.standing).collect();
            assert!(standing.len() > 300, "{preset:?} has woods");
            let pines = standing.iter().filter(|t| t.kind == Kind::Conifer).count();
            let share = pines as f32 / standing.len() as f32;
            assert!(
                (0.18..=0.85).contains(&share),
                "{preset:?} conifer share {share:.2}"
            );
            eprintln!("{preset:?} conifer share {share:.2}");
            shares.push(share);
        }
        let lo = shares.iter().cloned().fold(f32::MAX, f32::min);
        let hi = shares.iter().cloned().fold(f32::MIN, f32::max);
        assert!(hi - lo > 0.12, "shares {shares:?} should differ by style");
    }
}
