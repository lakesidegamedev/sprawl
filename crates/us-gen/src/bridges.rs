//! High bridges for deep gorges. A road that goes down into a gap and up the far side is carried
//! across it instead, on a deck at rim height that runs level (or at a gentle grade) from rim to
//! rim. The renderer and the generator tests share this one rule, so what is drawn is what is
//! checked.

/// How far below both rims the ground must be, metres, before a road is carried over it.
pub const GAP_DROP: f32 = 30.0;
/// How far to each side the rims are looked for, metres.
const LOOK: f32 = 900.0;
/// How far ahead a wall's top is looked for, and how much higher ground there must be to count
/// as still climbing, metres.
const AHEAD: f32 = 60.0;
const CLIMB: f32 = 12.0;
/// Length over which the deck ramps in beyond the rim, metres.
pub const RAMP: f32 = 24.0;

/// The deck of one road: where it is carried, and at what height.
#[derive(Clone, Debug, Default)]
pub struct Decks {
    /// 1 on a deck, 0 on open ground, between them on the ramp.
    pub weight: Vec<f32>,
    /// Deck height where the weight is above 0.
    pub z: Vec<f32>,
}

fn smooth(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Decks over the gaps of a road with ground heights `ground` at distances `cum` along it.
pub fn high_decks(ground: &[f32], cum: &[f32]) -> Decks {
    let n = ground.len();
    let mut out = Decks {
        weight: vec![0.0; n],
        z: vec![0.0; n],
    };
    if n < 3 {
        return out;
    }
    // gap vertices: far below the best ground on both sides within reach
    let mut gap = vec![false; n];
    let mut lmax = vec![f32::MIN; n];
    let mut j = 0;
    for i in 0..n {
        while cum[i] - cum[j] > LOOK {
            j += 1;
        }
        lmax[i] = ground[j..=i].iter().cloned().fold(f32::MIN, f32::max);
    }
    let mut j = n - 1;
    for i in (0..n).rev() {
        while cum[j] - cum[i] > LOOK {
            j -= 1;
        }
        let rmax = ground[i..=j].iter().cloned().fold(f32::MIN, f32::max);
        gap[i] = ground[i] < lmax[i].min(rmax) - GAP_DROP;
    }
    let mut i = 0;
    while i < n {
        if !gap[i] {
            i += 1;
            continue;
        }
        let mut e = i;
        while e + 1 < n && gap[e + 1] {
            e += 1;
        }
        // walk out to where each wall levels off: a ledge does not end the wall while the
        // ground a little further on is much higher
        let mut pa = i;
        loop {
            let mut best = pa;
            let mut k = pa;
            while k > 0 && cum[pa] - cum[k - 1] <= AHEAD {
                k -= 1;
                if ground[k] > ground[best] {
                    best = k;
                }
            }
            if ground[best] > ground[pa] + CLIMB {
                pa = best;
            } else {
                break;
            }
        }
        let mut pb = e;
        loop {
            let mut best = pb;
            let mut k = pb;
            while k + 1 < n && cum[k + 1] - cum[pb] <= AHEAD {
                k += 1;
                if ground[k] > ground[best] {
                    best = k;
                }
            }
            if ground[best] > ground[pb] + CLIMB {
                pb = best;
            } else {
                break;
            }
        }
        let (za, zb) = (ground[pa], ground[pb]);
        let span = (cum[pb] - cum[pa]).max(1e-3);
        for k in pa..=pb {
            let u = (cum[k] - cum[pa]) / span;
            out.weight[k] = 1.0;
            out.z[k] = za + (zb - za) * u;
        }
        // ramps beyond the rims
        let mut k = pa;
        while k > 0 && cum[pa] - cum[k - 1] < RAMP {
            k -= 1;
            let w = smooth(1.0 - (cum[pa] - cum[k]) / RAMP);
            if w > out.weight[k] {
                out.weight[k] = w;
                out.z[k] = za;
            }
        }
        let mut k = pb;
        while k + 1 < n && cum[k + 1] - cum[pb] < RAMP {
            k += 1;
            let w = smooth(1.0 - (cum[k] - cum[pb]) / RAMP);
            if w > out.weight[k] {
                out.weight[k] = w;
                out.z[k] = zb;
            }
        }
        i = pb.max(e) + 1;
    }
    out
}

/// Steepest grade (rise over run) of the road surface along a line: ground where the weight is
/// 0, the deck where it is 1, a blend on the ramps. `step` is only used for the unit of `cum`.
pub fn surface_grade(ground: &[f32], cum: &[f32], decks: &Decks) -> f32 {
    let z = |i: usize| ground[i] + (decks.z[i] - ground[i]) * decks.weight[i];
    let mut worst = 0.0f32;
    for i in 1..ground.len() {
        let d = (cum[i] - cum[i - 1]).max(1e-3);
        worst = worst.max(((z(i) - z(i - 1)) / d).abs());
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(f: impl Fn(f32) -> f32, len: f32) -> (Vec<f32>, Vec<f32>) {
        let n = (len / 5.0) as usize + 1;
        let cum: Vec<f32> = (0..n).map(|i| i as f32 * 5.0).collect();
        (cum.iter().map(|&s| f(s)).collect(), cum)
    }

    #[test]
    fn a_gorge_is_bridged_at_rim_height() {
        // plateau at 300, a gorge 300 m deep and 200 m wide with near vertical walls
        let (g, c) = line(
            |s| {
                let d = (s - 500.0).abs();
                if d < 100.0 {
                    0.0
                } else {
                    (300.0 * ((d - 100.0) / 20.0)).min(300.0)
                }
            },
            1000.0,
        );
        let d = high_decks(&g, &c);
        let mid = g.len() / 2;
        assert!(d.weight[mid] > 0.999);
        assert!((d.z[mid] - 300.0).abs() < 4.0, "deck at {}", d.z[mid]);
        assert!(surface_grade(&g, &c, &d) < 0.2);
    }

    #[test]
    fn a_plain_slope_is_not_bridged() {
        let (g, c) = line(|s| 300.0 - s * 0.2, 1000.0);
        let d = high_decks(&g, &c);
        assert!(d.weight.iter().all(|&w| w == 0.0));
    }
}
