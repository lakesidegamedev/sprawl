//! Which tower model stands on which downtown lot. The choice is made once for the whole town so
//! that no model repeats within `SPACING` metres, spires and crowns are kept for the one to three
//! tallest buildings, and the height falls off steeply from the centre. It depends only on the
//! town, so a lot always shows the same tower.
//!
//! What counts as "a model": only the SILHOUETTE (plan shape, setbacks, crown), see `classes`. A
//! recolour, a new wall material or a few floors more or less is the same model. When the choice
//! runs out near a lot, the lot gets a 6 to 12 floor street-wall block instead (`mid_rise`), which
//! is not under the cap. A lot is never left empty and the cap is never broken.
use crate::rng::Rng;
use std::sync::OnceLock;
use us_gen::{LotKind, Town, Zone};

/// The same tower model (family and plan) never stands twice within this many metres.
pub const SPACING: f64 = 300.0;
/// Density at and above which a lot is part of the downtown tower district.
pub const DOWNTOWN: f32 = 0.78;
/// Most of one tower model in a district (a `DISTRICT`-metre cell).
pub const PER_DISTRICT: usize = 2;
/// Side of a district cell, metres.
pub const DISTRICT: f64 = 400.0;
/// Most spired or crowned towers in a town.
pub const MAX_CROWNS: usize = 3;
const PLANS: usize = crate::architecture::PLANS;

/// Families without a spire, crown or mast: the everyday downtown tower.
pub const PLAIN: [usize; 12] = [13, 16, 17, 18, 19, 22, 5, 7, 8, 9, 11, 12];
/// Families with a spire or crown: only the tallest one to three towers of the town.
pub const CROWNED: [usize; 5] = [14, 15, 20, 21, 10];

/// A choice of building for one downtown lot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TowerSlot {
    pub family: usize,
    pub plan: usize,
    /// Index into the wall materials (see `skyline::tower_wall`).
    pub wall: usize,
    /// Stretch of the floor count: well under 1 at the edge of downtown, over 1 for the tallest.
    pub scale: f32,
    pub crown: bool,
    /// No tower model was free here: the lot gets a street-wall block of `floors` storeys.
    pub mid_rise: bool,
    /// Storeys of the mid-rise block (6 to 12); unused for a tower.
    pub floors: usize,
}

/// Whether a lot of this kind in this town may carry a downtown tower.
fn eligible(l: &us_gen::Lot) -> bool {
    l.density as f32 >= DOWNTOWN
        && matches!(
            l.kind,
            LotKind::Office
                | LotKind::Apartments
                | LotKind::House
                | LotKind::Row
                | LotKind::Workshop
                | LotKind::Warehouse
        )
}

/// Density at and above which a lot counts as downtown for its ground, colours and use.
pub const CORE_EDGE: f32 = 0.62;

/// Whether a lot is downtown for ground, colour and use: the core zone, or dense enough.
pub fn is_downtown(zone: Zone, density: f32) -> bool {
    zone == Zone::Core || density >= CORE_EDGE
}

/// A tower silhouette: every (family, plan) with the same outline, at full scale.
pub struct Class {
    pub members: Vec<(usize, usize)>,
    pub crown: bool,
}

/// The distinct silhouettes of the tower families. Two (family, plan) pairs are the same model
/// when their outlines match at ten slices of height (width and depth at each, in tenths of the
/// widest) and in overall width and depth to 4 m. Colour, material and floor count are not part
/// of it.
pub fn classes() -> &'static [Class] {
    static C: OnceLock<Vec<Class>> = OnceLock::new();
    C.get_or_init(|| {
        let mut seen: Vec<(Vec<i32>, usize)> = Vec::new();
        let mut out: Vec<Class> = Vec::new();
        for (crown, pool) in [(false, &PLAIN[..]), (true, &CROWNED[..])] {
            for &f in pool {
                for p in 0..PLANS {
                    let o = crate::architecture::build_with(f, p, 5, 1.0, None);
                    let sig = silhouette(&o);
                    match seen.iter().find(|(s, _)| *s == sig) {
                        Some(&(_, i)) => out[i].members.push((f, p)),
                        None => {
                            seen.push((sig, out.len()));
                            out.push(Class {
                                members: vec![(f, p)],
                                crown,
                            });
                        }
                    }
                }
            }
        }
        out
    })
}

fn silhouette(o: &crate::obj::Obj) -> Vec<i32> {
    let v: Vec<[f32; 3]> = o.polys.iter().flat_map(|p| p.pts.iter().copied()).collect();
    let h = v.iter().map(|p| p[2]).fold(0.0f32, f32::max).max(1.0);
    let fp = o.footprint();
    let (w, d) = (fp[2] - fp[0], fp[3] - fp[1]);
    let mut sig = vec![(w / 4.0).round() as i32, (d / 4.0).round() as i32];
    for k in 0..10 {
        let (z0, z1) = (h * k as f32 / 10.0, h * (k + 1) as f32 / 10.0);
        let (mut x0, mut x1, mut y0, mut y1) = (f32::MAX, f32::MIN, f32::MAX, f32::MIN);
        for p in v.iter().filter(|p| p[2] >= z0 && p[2] <= z1) {
            x0 = x0.min(p[0]);
            x1 = x1.max(p[0]);
            y0 = y0.min(p[1]);
            y1 = y1.max(p[1]);
        }
        if x1 < x0 {
            sig.push(-1);
            sig.push(-1);
        } else {
            sig.push((((x1 - x0) / w.max(1.0)) * 10.0).round() as i32);
            sig.push((((y1 - y0) / d.max(1.0)) * 10.0).round() as i32);
        }
    }
    sig
}

fn district(l: &us_gen::Lot) -> (i32, i32) {
    (
        (l.centre.x / DISTRICT).floor() as i32,
        (l.centre.y / DISTRICT).floor() as i32,
    )
}

/// One slot per lot of the town (indexed like `Town::lots`), `None` where no tower may stand.
/// The most central lots choose first, so the tallest buildings and the free silhouettes go to
/// the middle of downtown.
pub fn plan(t: &Town) -> Vec<Option<TowerSlot>> {
    let mut out = vec![None; t.lots.len()];
    let mut dense: Vec<usize> = (0..t.lots.len())
        .filter(|&i| eligible(&t.lots[i]))
        .collect();
    dense.sort_by(|&a, &b| {
        t.lots[b]
            .density
            .total_cmp(&t.lots[a].density)
            .then(a.cmp(&b))
    });
    let mut crowns: Vec<usize> = Vec::new();
    for &i in &dense {
        let l = &t.lots[i];
        if crowns.len() < MAX_CROWNS
            && l.density >= 0.9
            && crowns
                .iter()
                .all(|&j| l.centre.dist(t.lots[j].centre) > 140.0)
        {
            crowns.push(i);
        }
    }
    let cls = classes();
    // (lot, class) of the towers placed so far
    let mut placed: Vec<(usize, usize)> = Vec::new();
    for &i in &dense {
        let l = &t.lots[i];
        let mut rng = Rng::new(t.seed ^ 0x70_77_e5, i as u64 + 1);
        let crown = crowns.contains(&i);
        let edge = ((l.density as f32 - DOWNTOWN) / (1.0 - DOWNTOWN)).clamp(0.0, 1.0);
        let wall = rng.below(10);
        let free: Vec<usize> = (0..cls.len())
            .filter(|&c| cls[c].crown == crown)
            .filter(|&c| {
                let near = placed
                    .iter()
                    .filter(|&&(j, pc)| pc == c && l.centre.dist(t.lots[j].centre) < SPACING)
                    .count();
                let here = placed
                    .iter()
                    .filter(|&&(j, pc)| pc == c && district(&t.lots[j]) == district(l))
                    .count();
                near == 0 && here < PER_DISTRICT
            })
            .collect();
        // towers belong to the middle: the outer rim of downtown is mid-rise whatever is free
        let tower_ok = crown || edge >= 0.18;
        let mut slot = TowerSlot {
            family: 0,
            plan: 0,
            wall,
            scale: 0.0,
            crown,
            mid_rise: true,
            floors: 6 + (6.0 * edge.powf(1.5)).round() as usize,
        };
        if !free.is_empty() && tower_ok {
            let c = free[rng.below(free.len())];
            let (f, p) = cls[c].members[rng.below(cls[c].members.len())];
            slot.family = f;
            slot.plan = p;
            slot.mid_rise = false;
            slot.scale = if crown {
                1.35 + rng.range(0.0, 0.15)
            } else {
                // steep falloff: the middle rises, the rim stays low
                (0.4 + 0.9 * edge.powf(1.9)) * rng.range(0.9, 1.1)
            };
            placed.push((i, c));
        } else {
            slot.crown = false;
            slot.floors = (slot.floors + rng.below(2)).min(12);
        }
        out[i] = Some(slot);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOWNS: [(us_gen::Preset, u64); 4] = [
        (us_gen::Preset::Grid, 3),
        (us_gen::Preset::Winding, 4),
        (us_gen::Preset::LakeTown, 4),
        (us_gen::Preset::Sunbelt, 9),
    ];

    fn class_of(s: &TowerSlot) -> usize {
        classes()
            .iter()
            .position(|c| c.members.contains(&(s.family, s.plan)))
            .expect("every chosen model has a silhouette class")
    }

    #[test]
    fn there_are_enough_distinct_silhouettes() {
        assert!(classes().len() >= 24, "{} silhouettes", classes().len());
        assert!(classes().iter().any(|c| c.crown));
    }

    #[test]
    fn no_tower_model_repeats_within_the_radius() {
        let mut checked = 0;
        for (preset, seed) in TOWNS {
            let t = us_gen::generate(preset, seed);
            let slots = plan(&t);
            let towers: Vec<(usize, TowerSlot)> = slots
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.filter(|s| !s.mid_rise).map(|s| (i, s)))
                .collect();
            for (a, (i, s)) in towers.iter().enumerate() {
                for (j, u) in &towers[a + 1..] {
                    if t.lots[*i].centre.dist(t.lots[*j].centre) < SPACING {
                        assert!(
                            class_of(s) != class_of(u),
                            "{preset:?} {seed}: silhouette {} repeats within {SPACING} m",
                            class_of(s)
                        );
                        checked += 1;
                    }
                }
            }
            // at most two of one model in a district
            let mut count = std::collections::HashMap::new();
            for (i, s) in &towers {
                *count
                    .entry((class_of(s), district(&t.lots[*i])))
                    .or_insert(0) += 1;
            }
            assert!(count.values().all(|&n| n <= PER_DISTRICT), "district cap");
            // spires and crowns: the one to three tallest only
            let crowned = towers.iter().filter(|(_, s)| s.crown).count();
            assert!(crowned <= MAX_CROWNS, "{crowned} crowned towers");
            for (_, s) in &towers {
                assert!(s.crown == CROWNED.contains(&s.family));
            }
            // a lot with no model free gets a mid-rise block of 6 to 12 floors, never nothing
            for (i, l) in t.lots.iter().enumerate() {
                if eligible(l) {
                    let s = slots[i].expect("every eligible core lot has a building");
                    if s.mid_rise {
                        assert!((6..=12).contains(&s.floors), "{} floors", s.floors);
                        assert!(!s.crown);
                    }
                }
            }
        }
        assert!(checked > 20, "the test towns have towers close together");
    }

    #[test]
    fn a_big_downtown_falls_back_to_mid_rise_blocks_and_builds_every_lot() {
        let t = us_gen::generate(us_gen::Preset::LakeTown, 4);
        let slots = plan(&t);
        let mut mid = 0;
        let mut built = 0;
        for (i, l) in t.lots.iter().enumerate() {
            let Some(s) = slots[i] else { continue };
            mid += usize::from(s.mid_rise);
            let mut rng = Rng::new(7, i as u64);
            let o = crate::build_lot_level_district(
                t.preset,
                l.kind,
                5,
                l.radius as f32,
                l.density as f32,
                true,
                Some(&s),
                &mut rng,
            );
            assert!(!o.polys.is_empty(), "lot {i} was left unbuilt");
            assert!(
                crate::levels::top_of(&o) > 12.0,
                "lot {i} is a real building"
            );
            built += 1;
        }
        assert!(built > 100 && mid > 0, "{built} lots, {mid} mid-rise");
    }
}
