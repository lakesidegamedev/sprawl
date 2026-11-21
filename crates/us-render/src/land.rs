//! The lie of the land as the renderer sees it: a height grid the ground mesh is cut from, the
//! water level of each river band or lake, and the bridges roads need where they cross water.
//!
//! The grid is sampled with the same two-triangle split the ground mesh uses, so anything placed
//! with [`Land::height`] sits exactly on the drawn surface.

use glam::Vec3;
use std::sync::Arc;
use us_gen::{Pt, Town, pt};

/// Samples per side of the height grid (a 16 m step on the 4 km map).
pub const GRID: usize = 257;

/// How far a bridge deck stands above the water, metres.
pub const DECK_CLEAR: f32 = 5.0;
/// Length over which a road climbs from the ground to a bridge deck, metres.
pub const RAMP: f32 = 44.0;
/// Extra deck beyond the water's edge on each side, metres.
const ABUTMENT: f32 = 6.0;
/// Spacing of the points a road is resampled to, metres.
pub const BED_STEP: f32 = 5.0;

#[derive(Clone, Debug)]
struct WaterBody {
    ring: Vec<Pt>,
    lo: Pt,
    hi: Pt,
    level: f32,
}

#[derive(Clone, Debug)]
struct Inner {
    n: usize,
    size: f32,
    step: f32,
    data: Vec<f32>,
    water: Vec<WaterBody>,
    max: f32,
    min: f32,
    rock: Option<crate::rock::RockLook>,
    /// Roads cross deep gaps on a deck at rim height (the river gorge).
    high_bridges: bool,
}

/// Height grid and water of one town. Cheap to clone.
#[derive(Clone, Debug)]
pub struct Land {
    inner: Arc<Inner>,
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

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Land {
    /// Level ground of a map this big: for scenes that have no terrain.
    pub fn flat(size: f32) -> Land {
        Land::from_grid(2, size, vec![0.0; 4], Vec::new())
    }

    /// Heights from the town's terrain, with its water bodies.
    pub fn from_town(t: &Town) -> Land {
        // the coast has sheer cliffs: a grid twice as fine keeps their faces clean
        let n = if t.preset == us_gen::Preset::Coast {
            2 * (GRID - 1) + 1
        } else {
            GRID
        };
        let data = t.terrain.heightfield(n);
        Land::from_grid(n, t.size as f32, data, t.water.clone())
            .with_rock(crate::rock::RockLook::of(t.preset))
            .with_high_bridges(t.preset == us_gen::Preset::Gorge)
    }

    /// The same land where roads cross deep gaps on a high deck (or not).
    pub fn with_high_bridges(&self, on: bool) -> Land {
        let mut inner = (*self.inner).clone();
        inner.high_bridges = on;
        Land {
            inner: Arc::new(inner),
        }
    }

    /// The same land drawn with a rock look (or none).
    pub fn with_rock(&self, rock: Option<crate::rock::RockLook>) -> Land {
        let mut inner = (*self.inner).clone();
        inner.rock = rock;
        Land {
            inner: Arc::new(inner),
        }
    }

    /// How steep ground is drawn here, if this land has mountains or cliffs.
    pub fn rock(&self) -> Option<crate::rock::RockLook> {
        self.inner.rock
    }

    /// A grid of `n` by `n` heights over a square map of `size` metres, row by row from y = 0,
    /// with the water polygons that sit on it.
    pub fn from_grid(n: usize, size: f32, data: Vec<f32>, water: Vec<Vec<Pt>>) -> Land {
        assert!(n >= 2 && data.len() == n * n);
        let step = size / (n - 1) as f32;
        let (min, max) = data
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        let mut inner = Inner {
            n,
            size,
            step,
            data,
            water: Vec::new(),
            max,
            min,
            rock: None,
            high_bridges: false,
        };
        let mut bodies = Vec::new();
        for ring in water {
            if ring.len() < 3 {
                continue;
            }
            let (mut lo, mut hi) = (pt(f64::MAX, f64::MAX), pt(f64::MIN, f64::MIN));
            for p in &ring {
                lo = pt(lo.x.min(p.x), lo.y.min(p.y));
                hi = pt(hi.x.max(p.x), hi.y.max(p.y));
            }
            bodies.push(WaterBody {
                ring,
                lo,
                hi,
                level: 0.0,
            });
        }
        let probe = Land {
            inner: Arc::new(inner.clone()),
        };
        // the surface stands just above the highest ground inside the polygon
        for b in &mut bodies {
            let mut top = f32::MIN;
            let s = if b.ring.len() > 400 {
                inner.step
            } else {
                inner.step * 0.5
            } as f64;
            let lim = inner.size as f64;
            let mut y = b.lo.y.max(0.0);
            while y <= b.hi.y.min(lim) {
                let mut x = b.lo.x.max(0.0);
                while x <= b.hi.x.min(lim) {
                    if in_ring(pt(x, y), &b.ring) {
                        top = top.max(probe.height(x as f32, y as f32));
                    }
                    x += s;
                }
                y += s;
            }
            for p in &b.ring {
                // vertices off the map would read the held edge of the land
                if p.x >= 0.0 && p.y >= 0.0 && p.x <= inner.size as f64 && p.y <= inner.size as f64
                {
                    top = top.max(probe.height(p.x as f32, p.y as f32));
                }
            }
            b.level = top + 0.12;
        }
        inner.water = bodies;
        Land {
            inner: Arc::new(inner),
        }
    }

    pub fn size(&self) -> f32 {
        self.inner.size
    }

    /// Samples per side of the grid.
    pub(crate) fn shares_grid_with(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.inner, &other.inner)
    }

    pub fn n(&self) -> usize {
        self.inner.n
    }

    /// Grid spacing in metres.
    pub fn step(&self) -> f32 {
        self.inner.step
    }

    pub(crate) fn uses_high_bridges(&self) -> bool {
        self.inner.high_bridges
    }

    pub fn is_flat(&self) -> bool {
        self.inner.max - self.inner.min < 1e-4
    }

    /// Highest ground on the map, metres.
    pub fn max_height(&self) -> f32 {
        self.inner.max
    }

    pub fn min_height(&self) -> f32 {
        self.inner.min
    }

    fn sample(&self, i: usize, j: usize) -> f32 {
        let n = self.inner.n;
        self.inner.data[j.min(n - 1) * n + i.min(n - 1)]
    }

    /// Ground height at a point, metres. Outside the map it holds the edge value.
    pub fn height(&self, x: f32, y: f32) -> f32 {
        let i = &*self.inner;
        let fx = (x / i.step).clamp(0.0, (i.n - 1) as f32);
        let fy = (y / i.step).clamp(0.0, (i.n - 1) as f32);
        let (ix, iy) = (
            (fx.floor() as usize).min(i.n - 2),
            (fy.floor() as usize).min(i.n - 2),
        );
        let (u, v) = (fx - ix as f32, fy - iy as f32);
        let h00 = self.sample(ix, iy);
        let h11 = self.sample(ix + 1, iy + 1);
        if u >= v {
            let h10 = self.sample(ix + 1, iy);
            h00 + (h10 - h00) * u + (h11 - h10) * v
        } else {
            let h01 = self.sample(ix, iy + 1);
            h00 + (h01 - h00) * v + (h11 - h01) * u
        }
    }

    /// Whether a line starting on or above the ground stays above the drawn triangles.
    pub fn segment_above_ground(&self, start: Vec3, end: Vec3) -> bool {
        let scale = f64::from(self.step());
        let start64 = start.as_dvec3();
        let offset = end.as_dvec3() - start64;
        let a = start64 / scale;
        let d = offset / scale;

        // The height along the segment is linear between crossings of these three sets of
        // grid lines. Beyond the map edge, height() holds the edge and only x or y matters.
        let crossings = |origin: f64, delta: f64| {
            if delta == 0.0 {
                return (f64::INFINITY, f64::INFINITY);
            }
            let boundary = if delta > 0.0 {
                origin.floor() + 1.0
            } else {
                origin.ceil() - 1.0
            };
            ((boundary - origin) / delta, delta.abs().recip())
        };
        let (mut x, dx) = crossings(a.x, d.x);
        let (mut y, dy) = crossings(a.y, d.y);
        let (mut diagonal, dd) = crossings(a.x - a.y, d.x - d.y);
        loop {
            let t = x.min(y).min(diagonal).min(1.0);
            let p = (start64 + offset * t).as_vec3();
            if p.z < self.height(p.x, p.y) {
                return false;
            }
            if t == 1.0 || (end.z >= start.z && p.z > self.max_height()) {
                return true;
            }
            if x == t {
                x += dx;
            }
            if y == t {
                y += dy;
            }
            if diagonal == t {
                diagonal += dd;
            }
        }
    }

    /// Ground height at a point of the town.
    pub fn height_pt(&self, p: Pt) -> f32 {
        self.height(p.x as f32, p.y as f32)
    }

    /// Surface normal at a point (smoothed over a few metres, unit length).
    pub fn normal(&self, x: f32, y: f32) -> [f32; 3] {
        if self.is_flat() {
            return [0.0, 0.0, 1.0];
        }
        let e = self.inner.step * 0.5;
        let gx = (self.height(x + e, y) - self.height(x - e, y)) / (2.0 * e);
        let gy = (self.height(x, y + e) - self.height(x, y - e)) / (2.0 * e);
        let l = (gx * gx + gy * gy + 1.0).sqrt();
        [-gx / l, -gy / l, 1.0 / l]
    }

    /// Rise over run at a point.
    pub fn slope(&self, x: f32, y: f32) -> f32 {
        let n = self.normal(x, y);
        (1.0 - n[2] * n[2]).max(0.0).sqrt() / n[2].max(0.01)
    }

    /// True on the bare wet rock shelf at the foot of a sea cliff, where nothing grows.
    pub fn wet_shelf(&self, x: f32, y: f32) -> bool {
        let Some((_, below)) = self.inner.rock.and_then(|r| r.ground).and_then(|g| g.shelf) else {
            return false;
        };
        self.height(x, y) < below + 1.2
    }

    /// Level of the water at a point, if it is inside a river band or lake.
    pub fn water_at(&self, p: Pt) -> Option<f32> {
        self.inner
            .water
            .iter()
            .find(|b| {
                p.x >= b.lo.x
                    && p.x <= b.hi.x
                    && p.y >= b.lo.y
                    && p.y <= b.hi.y
                    && in_ring(p, &b.ring)
            })
            .map(|b| b.level)
    }

    /// Flat level to draw the water polygon `ring` at (it must be one of the town's).
    pub fn water_level(&self, ring: &[Pt]) -> f32 {
        self.inner
            .water
            .iter()
            .find(|b| b.ring.len() == ring.len() && b.ring.first() == ring.first())
            .map_or_else(
                || {
                    ring.iter()
                        .map(|p| self.height_pt(*p))
                        .fold(f32::MIN, f32::max)
                        + 0.12
                },
                |b| b.level,
            )
    }

    /// Floor height for a building at (x, y): the highest ground under its middle, so the
    /// downhill side shows a tall foundation and nothing is buried or left floating.
    pub fn sit_height(&self, x: f32, y: f32) -> f32 {
        let mut top = self.height(x, y);
        for (dx, dy) in [
            (5.5, 0.0),
            (-5.5, 0.0),
            (0.0, 5.5),
            (0.0, -5.5),
            (4.0, 4.0),
            (-4.0, 4.0),
            (4.0, -4.0),
            (-4.0, -4.0),
        ] {
            top = top.max(self.height(x + dx, y + dy));
        }
        top
    }

    /// Ground height range over a rectangle, sampled coarsely.
    pub fn range_over(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> (f32, f32) {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        let s = self.inner.step;
        let mut y = y0;
        while y <= y1 + s {
            let mut x = x0;
            while x <= x1 + s {
                let h = self.height(x.min(x1), y.min(y1));
                lo = lo.min(h);
                hi = hi.max(h);
                x += s;
            }
            y += s;
        }
        (lo, hi)
    }

    /// A road line cut into short steps with the height it should follow: the ground, or a
    /// bridge deck where it crosses water.
    pub fn bed(&self, line: &[Pt]) -> Bed {
        let mut pts: Vec<Pt> = Vec::with_capacity(line.len() * 2);
        for w in line.windows(2) {
            let l = w[0].dist(w[1]);
            let k = ((l / BED_STEP as f64).ceil() as usize).max(1);
            for s in 0..k {
                let p = w[0].lerp(w[1], s as f64 / k as f64);
                if pts.last().is_none_or(|q: &Pt| q.dist(p) > 1e-6) {
                    pts.push(p);
                }
            }
        }
        if let Some(l) = line.last()
            && pts.last().is_none_or(|q| q.dist(*l) > 1e-6)
        {
            pts.push(*l);
        }
        let n = pts.len();
        let mut cum = Vec::with_capacity(n);
        let mut s = 0.0f32;
        for i in 0..n {
            if i > 0 {
                s += pts[i - 1].dist(pts[i]) as f32;
            }
            cum.push(s);
        }
        let mut weight = vec![0.0f32; n];
        let mut deck = vec![0.0f32; n];
        if self.inner.high_bridges {
            let ground: Vec<f32> = pts.iter().map(|p| self.height_pt(*p)).collect();
            let d = us_gen::bridges::high_decks(&ground, &cum);
            weight = d.weight;
            deck = d.z;
        }
        if !self.inner.water.is_empty() && !self.is_flat() {
            // water crossings, with the level of the water they cross
            let wet: Vec<Option<f32>> = pts.iter().map(|p| self.water_at(*p)).collect();
            let mut i = 0;
            while i < n {
                let Some(lv) = wet[i] else {
                    i += 1;
                    continue;
                };
                let mut j = i;
                while j + 1 < n && wet[j + 1].is_some() {
                    j += 1;
                }
                let d = lv + DECK_CLEAR;
                let (s0, s1) = (cum[i] - ABUTMENT, cum[j] + ABUTMENT);
                // ramp out to both sides of the span
                let mut a = i;
                while a > 0 && cum[a - 1] > s0 - RAMP {
                    a -= 1;
                }
                let mut b = j;
                while b + 1 < n && cum[b + 1] < s1 + RAMP {
                    b += 1;
                }
                for (k, (wt, dk)) in weight
                    .iter_mut()
                    .zip(deck.iter_mut())
                    .enumerate()
                    .take(b + 1)
                    .skip(a)
                {
                    let off = if cum[k] < s0 {
                        s0 - cum[k]
                    } else if cum[k] > s1 {
                        cum[k] - s1
                    } else {
                        0.0
                    };
                    let w = smooth(1.0 - off / RAMP);
                    if w > *wt {
                        *wt = w;
                        *dk = d;
                    }
                }
                i = j + 1;
            }
        }
        Bed {
            pts,
            cum,
            weight,
            deck,
        }
    }
}

/// A road line resampled for draping. `weight` is 1 on a bridge deck, 0 on open ground and
/// between them on the approach ramps; `deck` is the deck height where the weight is above 0.
#[derive(Clone, Debug)]
pub struct Bed {
    pub pts: Vec<Pt>,
    pub cum: Vec<f32>,
    pub weight: Vec<f32>,
    pub deck: Vec<f32>,
}

impl Bed {
    /// Height of the road surface at the point `at` beside centreline vertex `i`.
    pub fn z(&self, land: &Land, i: usize, x: f32, y: f32) -> f32 {
        let g = land.height(x, y);
        let w = self.weight[i];
        if w <= 0.0 {
            g
        } else {
            // At an abutment the surveyed deck can sit slightly below a rise
            // in the hillside. Drape the approach where the ground is higher.
            g + (self.deck[i] - g).max(0.0) * w
        }
    }

    /// Runs of vertices that are fully on a deck: (first, last) indices.
    pub fn spans(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.pts.len() {
            if self.weight[i] > 0.999 {
                let mut j = i;
                while j + 1 < self.pts.len() && self.weight[j + 1] > 0.999 {
                    j += 1;
                }
                out.push((i, j));
                i = j + 1;
            } else {
                i += 1;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tilted() -> Land {
        // height = x / 10 over a 160 m map on a 17 sample grid (10 m step)
        let n = 17;
        let mut d = Vec::new();
        for _j in 0..n {
            for i in 0..n {
                d.push(i as f32);
            }
        }
        Land::from_grid(n, 160.0, d, Vec::new())
    }

    #[test]
    fn bridge_approach_stays_above_a_high_abutment() {
        let l = tilted();
        let b = Bed {
            pts: Vec::new(),
            cum: Vec::new(),
            weight: vec![0.5],
            deck: vec![8.0],
        };
        assert_eq!(b.z(&l, 0, 100.0, 0.0), 10.0);
        assert_eq!(b.z(&l, 0, 60.0, 0.0), 7.0);
    }

    #[test]
    fn height_interpolates_and_clamps() {
        let l = tilted();
        assert!((l.height(55.0, 40.0) - 5.5).abs() < 1e-4);
        assert!((l.height(-30.0, 10.0) - 0.0).abs() < 1e-4);
        assert!((l.height(500.0, 10.0) - 16.0).abs() < 1e-4);
    }

    #[test]
    fn normal_leans_downhill() {
        let l = tilted();
        let n = l.normal(80.0, 80.0);
        assert!(n[0] < -0.05 && n[1].abs() < 1e-4 && n[2] > 0.9);
        assert!(Land::flat(100.0).normal(5.0, 5.0) == [0.0, 0.0, 1.0]);
    }

    #[test]
    fn road_over_a_river_gets_a_deck() {
        let n = 17;
        let d = vec![2.0; n * n];
        let river = vec![
            pt(60.0, -10.0),
            pt(100.0, -10.0),
            pt(100.0, 170.0),
            pt(60.0, 170.0),
        ];
        // make it non-flat so bridging is active
        let mut d2 = d.clone();
        d2[0] = 3.0;
        let l = Land::from_grid(n, 160.0, d2, vec![river]);
        let bed = l.bed(&[pt(0.0, 80.0), pt(160.0, 80.0)]);
        assert!(bed.spans().len() == 1);
        let (a, b) = bed.spans()[0];
        assert!(bed.deck[a] > 4.5);
        assert!(bed.weight[0] == 0.0 && bed.weight[bed.pts.len() - 1] == 0.0);
        assert!(bed.pts[a].x < 60.0 && bed.pts[b].x >= 95.0);
    }
}
