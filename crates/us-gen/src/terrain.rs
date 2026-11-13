//! A deterministic heightfield: flat ground round the town hall and along the highway, a gentle
//! valley with flat floor for the river, hills further out. Pure function of the seed.

use crate::geom::{Pt, atan2, cos, pt, sin};
use crate::rng::Rng;
use crate::world::{HALL, HWY_Y, River, S};

const WAVES: usize = 6;

/// Height of the land in metres at any point of the map. Cheap to evaluate, so renderers can
/// sample it per vertex rather than storing a grid.
#[derive(Clone, Debug)]
pub struct Terrain {
    river: River,
    /// Lake basin (centre, radius) that pulls the ground down and keeps it flat.
    basin: Option<(Pt, f64)>,
    /// (kx, ky, phase, amplitude) of each wave.
    waves: [(f64, f64, f64, f64); WAVES],
}

fn smooth(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl Terrain {
    pub(crate) fn new(seed: u64, river: &River, basin: Option<(Pt, f64)>) -> Terrain {
        let mut rng = Rng::new(seed, 0x7e44);
        let mut waves = [(0.0, 0.0, 0.0, 0.0); WAVES];
        for (i, w) in waves.iter_mut().enumerate() {
            // three scales: broad hills, ridges, knolls
            let (len, amp) = match i / 2 {
                0 => (rng.uniform(1400.0, 2200.0), 20.0),
                1 => (rng.uniform(700.0, 1000.0), 6.0),
                _ => (rng.uniform(300.0, 420.0), 2.0),
            };
            let dir = rng.uniform(0.0, std::f64::consts::TAU);
            let k = std::f64::consts::TAU / len;
            *w = (
                k * cos(dir),
                k * sin(dir),
                rng.uniform(0.0, std::f64::consts::TAU),
                amp * rng.uniform(0.8, 1.2),
            );
        }
        Terrain {
            river: river.clone(),
            basin,
            waves,
        }
    }

    /// Ground height in metres (0 is roughly the river's floor).
    pub fn height(&self, x: f64, y: f64) -> f64 {
        let mut hills = 0.0;
        for (kx, ky, ph, a) in &self.waves {
            hills += a * sin(kx * x + ky * y + ph);
        }
        let d_hall = ((x - HALL.x).powi(2) + (y - HALL.y).powi(2)).sqrt();
        let dy = (y - self.river.y(x.clamp(0.0, S))).abs();
        let mut mask = smooth(450.0, 1500.0, d_hall)
            * smooth(HWY_Y + 120.0, HWY_Y + 700.0, y)
            * smooth(160.0, 520.0, dy);
        let mut base = 30.0 + 0.0 * x;
        // the valley: the land falls to the river over about half a kilometre each side
        base -= 28.0 * exp_neg((dy / 420.0).powi(2));
        if let Some((c, r)) = self.basin {
            let d = ((x - c.x).powi(2) + (y - c.y).powi(2)).sqrt();
            mask *= smooth(r * 1.4, r * 2.6, d);
            base -= 14.0 * exp_neg((d / (r * 2.2)).powi(2));
        }
        base + hills * mask
    }

    /// Height at a point.
    pub fn height_at(&self, p: Pt) -> f64 {
        self.height(p.x, p.y)
    }

    /// Gradient (rise per metre east, rise per metre north) by central differences.
    pub fn gradient(&self, p: Pt) -> Pt {
        let e = 6.0;
        pt(
            (self.height(p.x + e, p.y) - self.height(p.x - e, p.y)) / (2.0 * e),
            (self.height(p.x, p.y + e) - self.height(p.x, p.y - e)) / (2.0 * e),
        )
    }

    /// Steepness at a point: rise over run, so 0.1 is a ten percent grade.
    pub fn slope(&self, p: Pt) -> f64 {
        self.gradient(p).len()
    }

    /// Direction (radians, in [0, pi)) of the contour line through a point: along the hillside
    /// rather than up or down it. `None` on ground flatter than `min_slope`.
    pub fn contour_angle(&self, p: Pt, min_slope: f64) -> Option<f64> {
        let g = self.gradient(p);
        if g.len() < min_slope {
            return None;
        }
        let a = atan2(g.x, -g.y);
        Some(a.rem_euclid(std::f64::consts::PI))
    }

    /// Heights on an `n` by `n` grid over the whole map, row by row from y = 0, as f32.
    /// Sample (i, j) sits at x = i * size / (n - 1), y = j * size / (n - 1).
    pub fn heightfield(&self, n: usize) -> Vec<f32> {
        let n = n.max(2);
        let step = S / (n - 1) as f64;
        let mut out = Vec::with_capacity(n * n);
        for j in 0..n {
            for i in 0..n {
                out.push(self.height(i as f64 * step, j as f64 * step) as f32);
            }
        }
        out
    }
}

fn exp_neg(v: f64) -> f64 {
    libm::exp(-v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_at_hall_and_hilly_far_out() {
        let river = River::new(&mut Rng::new(1, 0x71e7));
        let t = Terrain::new(1, &river, None);
        assert!(t.slope(HALL) < 0.03);
        let mut steep = 0;
        for i in 0..40 {
            for j in 0..40 {
                if t.slope(pt(100.0 + i as f64 * 100.0, 400.0 + j as f64 * 60.0)) > 0.1 {
                    steep += 1;
                }
            }
        }
        assert!(steep > 10, "no steep ground at all: {steep}");
        assert!(steep < 800, "far too much steep ground: {steep}");
    }
}
