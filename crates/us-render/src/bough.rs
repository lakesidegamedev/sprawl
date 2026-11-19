//! Bare-tree skeletons: a trunk, a fork of limbs and twigs, in a few habits.
//!
//! Every broadleaf tree carries its skeleton inside the crown all year. The shader folds the
//! leaf-coloured crown away once the leaves are down, so what is left is this: a thin silhouette
//! with sky showing through, and a thin broken shadow. The skeleton is a list of tapered
//! three-sided tubes in a local frame (trunk foot at the origin, z up) and is built only from the
//! seed given, so a town draws the same trees every time.

use crate::rng::Rng;
use glam::Vec3;

/// Colour of bark and twigs. Not a leaf colour, so the shader never folds it away.
pub const BARK: [u8; 3] = [134, 114, 98];

/// One limb: a tapered tube from `a` to `b`. `rb` of zero ends in a point.
#[derive(Clone, Copy, Debug)]
pub struct Tube {
    pub a: Vec3,
    pub b: Vec3,
    pub ra: f32,
    pub rb: f32,
    /// 0 trunk or leader, 1 limb, 2 twig.
    pub level: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Habit {
    /// A rounded crown; four forms (0 to 3).
    Round(u8),
    /// A tall narrow crown (poplar).
    Columnar,
    /// A wide low crown (oak).
    Spreading,
}

fn dir(az: f32, elev: f32) -> Vec3 {
    let (ce, se) = (libm::cosf(elev), libm::sinf(elev));
    Vec3::new(ce * libm::cosf(az), ce * libm::sinf(az), se)
}

/// A direction near `d`, turned by up to `spread` radians in a random direction.
fn near(rng: &mut Rng, d: Vec3, spread: f32) -> Vec3 {
    let helper = if d.z.abs() > 0.9 { Vec3::X } else { Vec3::Z };
    let u = d.cross(helper).normalize_or_zero();
    let v = d.cross(u);
    let a = rng.range(0.0, std::f32::consts::TAU);
    let s = rng.range(0.4, 1.0) * spread;
    (d * libm::cosf(s) + (u * libm::cosf(a) + v * libm::sinf(a)) * libm::sinf(s))
        .normalize_or_zero()
}

struct Builder<'a> {
    rng: &'a mut Rng,
    out: Vec<Tube>,
}

impl Builder<'_> {
    fn tube(&mut self, a: Vec3, b: Vec3, ra: f32, rb: f32, level: u8) {
        self.out.push(Tube {
            a,
            b,
            ra,
            rb,
            level,
        });
    }

    /// A limb from `from` along `d` for `len`, with twigs along it and at its end.
    fn limb(&mut self, from: Vec3, d: Vec3, len: f32, ra: f32, twigs: usize) {
        let to = from + d * len;
        self.tube(from, to, ra, ra * 0.38, 1);
        let mid = from + d * len * 0.55;
        let spread = 0.9;
        let t = self.rng.range(0.6, 1.0);
        let dm = near(self.rng, d, spread);
        let dm = Vec3::new(dm.x, dm.y, dm.z.max(0.1)).normalize();
        self.tube(mid, mid + dm * len * 0.42 * t, ra * 0.36, 0.0, 2);
        for _ in 0..twigs {
            let dt = near(self.rng, d, 0.75);
            let dt = Vec3::new(dt.x, dt.y, dt.z.max(0.12)).normalize();
            let l = len * self.rng.range(0.32, 0.5);
            self.tube(to, to + dt * l, ra * 0.3, 0.0, 2);
        }
    }
}

/// The skeleton of a tree `h` metres tall with a crown `r` metres in radius, in a form picked by
/// `habit`, turned and leaned from `seed`.
pub fn skeleton(habit: Habit, r: f32, h: f32, seed: u64) -> Vec<Tube> {
    let mut rng = Rng::new(seed, 0x6272_616e_6368);
    let turn = rng.range(0.0, std::f32::consts::TAU);
    let lean = Vec3::new(rng.range(-1.0, 1.0), rng.range(-1.0, 1.0), 0.0) * h * 0.035;
    // a wide spread of size and of crown breadth, so a stand is not one tree repeated. The
    // skeleton only ever shrinks inside the crown mesh (the top of each range matches the old
    // one), so a limb never pokes out of the leaves.
    let size = rng.range(0.72, 1.06);
    let (r, h) = (
        r * size * rng.range(0.85, 1.15),
        h * size * rng.range(0.92, 1.06),
    );
    let trunk_r = (0.034 * h).clamp(0.13, 0.5);
    let mut b = Builder {
        rng: &mut rng,
        out: Vec::with_capacity(32),
    };
    match habit {
        Habit::Round(0) => {
            // vase: a short trunk and four limbs rising steeply
            let fork = Vec3::new(0.0, 0.0, h * 0.36);
            b.tube(Vec3::ZERO, fork, trunk_r, trunk_r * 0.72, 0);
            for k in 0..4 {
                let az = turn + std::f32::consts::TAU * k as f32 / 4.0 + b.rng.range(-0.3, 0.3);
                let el = b.rng.range(0.9, 1.25);
                let reach = r * b.rng.range(0.6, 0.95);
                let len = reach / libm::cosf(el);
                b.limb(fork, dir(az, el), len, trunk_r * 0.62, 2);
            }
        }
        Habit::Round(1) => {
            // spreading: a low fork of three wide limbs and a short leader
            let fork = Vec3::new(0.0, 0.0, h * 0.32);
            b.tube(Vec3::ZERO, fork, trunk_r, trunk_r * 0.78, 0);
            for k in 0..3 {
                let az = turn + std::f32::consts::TAU * k as f32 / 3.0 + b.rng.range(-0.35, 0.35);
                let el = b.rng.range(0.5, 0.85);
                let reach = r * b.rng.range(0.85, 1.1);
                let len = reach / libm::cosf(el);
                b.limb(fork, dir(az, el), len, trunk_r * 0.7, 3);
            }
            let top = fork + Vec3::new(0.0, 0.0, h * 0.3);
            b.tube(fork, top, trunk_r * 0.5, trunk_r * 0.2, 0);
            let d = near(b.rng, Vec3::Z, 0.5);
            b.limb(top, d, h * 0.2, trunk_r * 0.28, 2);
        }
        Habit::Round(2) => {
            // central leader with limbs stepping up it, shorter towards the top
            let top = Vec3::new(0.0, 0.0, h * 0.94);
            b.tube(Vec3::ZERO, top, trunk_r, trunk_r * 0.16, 0);
            let mut az = turn;
            for k in 0..6 {
                let f = k as f32 / 5.0;
                let from = Vec3::new(0.0, 0.0, h * (0.36 + 0.5 * f));
                az += 2.4 + b.rng.range(-0.3, 0.3);
                let el = b.rng.range(0.3, 0.62) + 0.3 * f;
                let reach = r * (1.0 - 0.7 * f) * b.rng.range(0.8, 1.05);
                let len = reach / libm::cosf(el);
                b.limb(from, dir(az, el), len, trunk_r * (0.55 - 0.3 * f), 1);
            }
        }
        Habit::Round(_) => {
            // twin trunk: two limbs part low, each forks again
            let fork = Vec3::new(0.0, 0.0, h * 0.28);
            b.tube(Vec3::ZERO, fork, trunk_r * 1.05, trunk_r * 0.9, 0);
            for k in 0..2 {
                let az = turn + std::f32::consts::PI * k as f32 + b.rng.range(-0.25, 0.25);
                let d = dir(az, b.rng.range(1.05, 1.3));
                let len = h * b.rng.range(0.26, 0.34);
                let top = fork + d * len;
                b.tube(fork, top, trunk_r * 0.7, trunk_r * 0.5, 0);
                for j in 0..2 {
                    let az2 = az + (j as f32 * 2.0 - 1.0) * b.rng.range(0.7, 1.2);
                    let el = b.rng.range(0.55, 0.95);
                    let reach = r * b.rng.range(0.7, 1.0);
                    b.limb(top, dir(az2, el), reach / libm::cosf(el), trunk_r * 0.45, 2);
                }
            }
        }
        Habit::Columnar => {
            let fork = Vec3::new(0.0, 0.0, h * 0.28);
            b.tube(Vec3::ZERO, fork, trunk_r * 0.9, trunk_r * 0.6, 0);
            let top = Vec3::new(0.0, 0.0, h * 0.97);
            b.tube(fork, top, trunk_r * 0.6, trunk_r * 0.12, 0);
            for k in 0..6 {
                let f = k as f32 / 5.0;
                let from = Vec3::new(0.0, 0.0, h * (0.32 + 0.5 * f));
                let az = turn + 2.4 * k as f32 + b.rng.range(-0.3, 0.3);
                let el = b.rng.range(1.05, 1.3);
                let reach = r * (0.95 - 0.5 * f) * b.rng.range(0.8, 1.1);
                b.limb(from, dir(az, el), reach / libm::cosf(el), trunk_r * 0.4, 1);
            }
        }
        Habit::Spreading => {
            let fork = Vec3::new(0.0, 0.0, h * 0.3);
            b.tube(Vec3::ZERO, fork, trunk_r * 1.15, trunk_r * 0.9, 0);
            for k in 0..4 {
                let az = turn + std::f32::consts::TAU * k as f32 / 4.0 + b.rng.range(-0.4, 0.4);
                let el = b.rng.range(0.35, 0.8);
                let reach = r * b.rng.range(0.85, 1.15);
                b.limb(fork, dir(az, el), reach / libm::cosf(el), trunk_r * 0.7, 3);
            }
            let d = near(b.rng, Vec3::Z, 0.4);
            b.limb(fork, d, h * 0.45, trunk_r * 0.5, 2);
        }
    }
    let mut out = b.out;
    // a limb never rises above the crown: squash the whole skeleton if it would
    let top = out.iter().fold(0.0f32, |m, t| m.max(t.a.z).max(t.b.z));
    if top > h * 0.96 {
        let k = h * 0.96 / top;
        for t in &mut out {
            t.a.z *= k;
            t.b.z *= k;
        }
    }
    // and no tip strays further sideways than twice the crown radius
    let limit = r * 2.0;
    for t in &mut out {
        for p in [&mut t.a, &mut t.b] {
            let d = p.truncate().length();
            if d > limit {
                let k = limit / d;
                p.x *= k;
                p.y *= k;
            }
        }
    }
    for t in &mut out {
        // lean: the higher a point, the further it is shifted, so the tree is not plumb
        t.a += lean * (t.a.z / h);
        t.b += lean * (t.b.z / h);
    }
    out
}

/// Pick a habit for a tree of the given crown from a seed: round forms most of the time.
pub fn pick(seed: u64, tall: bool) -> Habit {
    let mut rng = Rng::new(seed, 0x0068_6162_6974);
    if tall {
        return if rng.chance(0.6) {
            Habit::Columnar
        } else {
            Habit::Round(rng.below(4) as u8)
        };
    }
    if rng.chance(0.2) {
        Habit::Spreading
    } else {
        Habit::Round(rng.below(4) as u8)
    }
}

/// A seed from where a tree stands, the same for the same place on every platform.
pub fn seed_at(x: f32, y: f32) -> u64 {
    let a = (x * 16.0).round() as i64 as u64;
    let b = (y * 16.0).round() as i64 as u64;
    a.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ b.wrapping_mul(0xc2b2_ae3d_27d4_eb4f)
}

/// The three corners of a tube's two rings (or ring and tip) as faces facing outward:
/// `[a0, a1, b1, b0]` quads, or `[a0, a1, tip]` triangles when it ends in a point.
/// Each face comes with its outward normal.
pub fn faces(t: &Tube) -> Vec<(Vec<Vec3>, Vec3)> {
    let axis = (t.b - t.a).normalize_or_zero();
    if axis == Vec3::ZERO {
        return Vec::new();
    }
    let helper = if axis.z.abs() > 0.95 {
        Vec3::X
    } else {
        Vec3::Z
    };
    let u = axis.cross(helper).normalize();
    let v = axis.cross(u);
    let ring = |c: Vec3, rad: f32, k: usize| {
        let a = std::f32::consts::TAU * k as f32 / 3.0;
        let out = u * libm::cosf(a) + v * libm::sinf(a);
        (c + out * rad, out)
    };
    let mut faces = Vec::with_capacity(3);
    for k in 0..3 {
        let (a0, n0) = ring(t.a, t.ra, k);
        let (a1, n1) = ring(t.a, t.ra, (k + 1) % 3);
        let n = (n0 + n1).normalize_or_zero();
        if t.rb <= 0.001 {
            faces.push((vec![a0, a1, t.b], n));
        } else {
            let (b0, _) = ring(t.b, t.rb, k);
            let (b1, _) = ring(t.b, t.rb, (k + 1) % 3);
            faces.push((vec![a0, a1, b1, b0], n));
        }
    }
    faces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeletons_are_stable_and_stay_inside_a_sane_envelope() {
        for habit in [
            Habit::Round(0),
            Habit::Round(1),
            Habit::Round(2),
            Habit::Round(3),
            Habit::Columnar,
            Habit::Spreading,
        ] {
            for seed in 0..40u64 {
                let a = skeleton(habit, 3.5, 10.0, seed);
                let b = skeleton(habit, 3.5, 10.0, seed);
                assert_eq!(a.len(), b.len());
                assert!(
                    a.len() >= 6 && a.len() <= 60,
                    "{habit:?} has {} tubes",
                    a.len()
                );
                for (p, q) in a.iter().zip(&b) {
                    assert_eq!(p.a, q.a);
                    assert_eq!(p.b, q.b);
                }
                for t in &a {
                    for p in [t.a, t.b] {
                        assert!(p.is_finite());
                        assert!(p.z > -0.05 && p.z < 12.5, "{habit:?} z {}", p.z);
                        assert!(p.truncate().length() < 3.5 * 2.4, "{habit:?} reach");
                    }
                }
            }
        }
    }

    #[test]
    fn tube_faces_face_outward() {
        let t = Tube {
            a: Vec3::ZERO,
            b: Vec3::new(1.0, 0.5, 2.0),
            ra: 0.2,
            rb: 0.1,
            level: 1,
        };
        let mid = (t.a + t.b) * 0.5;
        for (pts, n) in faces(&t) {
            let c = pts.iter().fold(Vec3::ZERO, |s, p| s + *p) / pts.len() as f32;
            let geometric = (pts[1] - pts[0]).cross(pts[2] - pts[0]);
            assert!(geometric.dot(n) > 0.0, "winding disagrees with the normal");
            let axis = (t.b - t.a).normalize();
            let radial = (c - mid) - axis * (c - mid).dot(axis);
            assert!(radial.dot(n) > 0.0);
        }
    }
}
