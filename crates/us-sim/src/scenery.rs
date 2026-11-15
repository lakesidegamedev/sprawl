//! Views of the sea and of the rock: land beside the water of a coastal town, or beside the mesas
//! and buttes of a red rock town, is worth more. Towns of the other styles have none of it.

use crate::*;
use us_gen::{Preset, Relief};

/// How far the sea can be seen from, metres.
const SEA_REACH: f64 = 400.0;
/// How far from the foot of a rock formation its view carries, metres.
const ROCK_REACH: f64 = 450.0;
/// Land value added right at the water's edge, and at the foot of the tallest rock.
const SEA_VALUE: f64 = 0.08;
const ROCK_VALUE: f64 = 0.05;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum View {
    Sea,
    Rock,
}

impl Sim {
    /// What a point looks out on and how strongly (0 to 1), if anything.
    fn view_at(&self, p: Pt) -> Option<(View, f64)> {
        match (self.town.preset, self.town.terrain.relief()) {
            (Preset::Coast, _) => {
                let ring = self.town.water.first()?;
                let d = dist_pt_ring_edge(p, ring);
                (d < SEA_REACH).then(|| (View::Sea, 1.0 - d / SEA_REACH))
            }
            (Preset::Mesa, Relief::Mesa(m)) => {
                let mut best: Option<f64> = None;
                for f in &m.formations {
                    let e = f.edge_dist(p.x, p.y) - f.wall - 0.7 * f.skirt;
                    if e < ROCK_REACH {
                        let s = (1.0 - e.max(0.0) / ROCK_REACH) * (f.top / 250.0).clamp(0.5, 1.0);
                        best = Some(best.map_or(s, |b| b.max(s)));
                    }
                }
                best.map(|s| (View::Rock, s))
            }
            _ => None,
        }
    }

    /// Land value a lot gains from its view.
    pub(crate) fn scenery_bonus(&self, i: usize) -> f64 {
        match self.view_at(self.lots[i].pos) {
            Some((View::Sea, s)) => SEA_VALUE * s,
            Some((View::Rock, s)) => ROCK_VALUE * s,
            None => 0.0,
        }
    }

    /// A line for the why panel when the view adds to the value of a lot.
    pub(crate) fn scenery_line(&self, i: usize) -> Option<String> {
        let l = self.lots.get(i)?;
        if l.phase == Phase::Hidden || l.exempt {
            return None;
        }
        match self.view_at(l.pos)? {
            (View::Sea, s) if s > 0.05 => {
                Some("Sea views: land value here is higher near the water.".into())
            }
            (View::Rock, s) if s > 0.05 => {
                Some("Rock views: land value here is higher near the mesas and buttes.".into())
            }
            _ => None,
        }
    }
}
