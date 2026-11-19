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
pub const PLAIN: [usize; 15] = [13, 16, 17, 18, 19, 22, 5, 7, 8, 9, 11, 12, 23, 24, 25];
/// Families with a spire or crown: only the tallest one to three towers of the town.
pub const CROWNED: [usize; 5] = [14, 15, 20, 21, 10];

// ---- how tall a lot is allowed to be ----

/// (population, storeys of the tallest possible building) anchors: the storey cap of a town is
/// read off this line, so a few thousand people have a modest centre and only a big town earns
/// real towers.
const CAP_LINE: [(f32, f32); 9] = [
    (0.0, 3.0),
    (1000.0, 4.0),
    (2500.0, 7.0),
    (4000.0, 9.0),
    (7000.0, 14.0),
    (12000.0, 20.0),
    (20000.0, 30.0),
    (40000.0, 44.0),
    (80000.0, 60.0),
];
/// A building of at least this many storeys is drawn as a tower model; lower ones are street-wall
/// blocks.
pub const TOWER_MIN: usize = 14;
/// A spire or crown needs at least this many storeys.
pub const CROWN_MIN: usize = 30;

/// Storeys of the tallest building a town of this population can have (a level 5 lot in the very
/// middle of downtown): 3 at nothing, 9 at 4000 people, 14 at 7000, 30 at 20000, 60 at 80000.
pub fn storey_cap(pop: u32) -> u8 {
    let p = pop as f32;
    let mut out = CAP_LINE[CAP_LINE.len() - 1].1;
    for w in CAP_LINE.windows(2) {
        if p < w[1].0 {
            out = w[0].1 + (w[1].1 - w[0].1) * (p - w[0].0) / (w[1].0 - w[0].0);
            break;
        }
    }
    out.round() as u8
}

/// Share of the storey cap a lot of this building level reaches.
pub fn level_share(level: u8) -> f32 {
    [0.3, 0.45, 0.62, 0.85, 1.0][usize::from(level.clamp(1, 5)) - 1]
}

/// Share of the storey cap a lot of this density reaches: steep, so the rim of downtown is low
/// and only the middle is tall.
pub fn centre_share(density: f32) -> f32 {
    let e = ((density - CORE_EDGE) / (1.0 - CORE_EDGE)).clamp(0.0, 1.0);
    0.18 + 0.82 * e.powf(2.2)
}

/// Storeys of a downtown building: the town's cap times what the lot's level and its place in
/// downtown allow, a little roll for variety, never under 2.
pub fn lot_storeys(cap: u8, level: u8, density: f32, roll: f32) -> usize {
    let s = f32::from(cap) * level_share(level) * centre_share(density) * (0.88 + 0.24 * roll);
    (s.round() as usize).max(2)
}

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
    /// No tower model was free here: the lot gets a street-wall block.
    pub mid_rise: bool,
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
            // a lot with no model free gets a street-wall block, never nothing
            for (i, l) in t.lots.iter().enumerate() {
                if eligible(l) {
                    let s = slots[i].expect("every eligible core lot has a building");
                    assert!(!(s.mid_rise && s.crown));
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
                60,
                &mut rng,
            );
            assert!(!o.polys.is_empty(), "lot {i} was left unbuilt");
            assert!(
                crate::levels::top_of(&o) > 6.0,
                "lot {i} is a real building"
            );
            built += 1;
        }
        assert!(built > 100 && mid > 0, "{built} lots, {mid} mid-rise");
    }

    #[test]
    fn the_storey_cap_follows_the_population() {
        let caps: Vec<u8> = [
            0u32, 1000, 2500, 4000, 6000, 7000, 12000, 20000, 40000, 80000, 200000,
        ]
        .iter()
        .map(|&p| storey_cap(p))
        .collect();
        assert!(caps.windows(2).all(|w| w[0] <= w[1]), "{caps:?}");
        assert_eq!(storey_cap(0), 3);
        assert!(
            storey_cap(5800) < TOWER_MIN as u8,
            "a small town has no towers"
        );
        assert!(storey_cap(20000) >= 30);
        assert_eq!(storey_cap(1_000_000), 60);
    }

    #[test]
    fn height_falls_off_from_the_centre_and_grows_with_level() {
        for cap in [9u8, 20, 44] {
            let mut last = usize::MAX;
            for d in [1.0f32, 0.9, 0.8, 0.7, 0.62] {
                let n = lot_storeys(cap, 4, d, 0.5);
                assert!(n <= last, "cap {cap}: density {d} gives {n}, was {last}");
                last = n;
            }
            assert!(lot_storeys(cap, 5, 1.0, 0.5) >= lot_storeys(cap, 3, 1.0, 0.5));
            // the rim is low whatever the town: well under a third of the cap
            assert!(lot_storeys(cap, 4, 0.62, 0.5) * 3 <= usize::from(cap).max(6));
        }
    }

    #[test]
    fn every_tower_model_builds_at_every_forced_floor_count() {
        for &f in PLAIN.iter().chain(CROWNED.iter()) {
            for p in 0..PLANS {
                for n in [8usize, 9, 11, 14, 17, 22, 30, 45, 60] {
                    let o = crate::skyline::with_tower(3.5, Some(n), || {
                        crate::architecture::build_with(f, p, 5, 1.0, None)
                    });
                    let h = crate::levels::top_of(&o);
                    assert!(h.is_finite() && h > n as f32 * 3.0, "{f}/{p} at {n}: {h} m");
                    assert!(
                        o.polys
                            .iter()
                            .flat_map(|q| &q.pts)
                            .all(|v| v.iter().all(|c| c.is_finite()))
                    );
                }
            }
        }
    }

    #[test]
    fn every_downtown_lot_builds_at_every_cap_and_level() {
        let t = us_gen::generate(us_gen::Preset::Grid, 3);
        let slots = plan(&t);
        for (i, l) in t.lots.iter().enumerate() {
            if !is_downtown(l.zone, l.density as f32) {
                continue;
            }
            for cap in [3u8, 9, 14, 30, 60] {
                for level in [1u8, 3, 4, 5] {
                    let mut rng = Rng::new(9, i as u64);
                    let o = crate::build_lot_level_district(
                        t.preset,
                        l.kind,
                        level,
                        l.radius as f32,
                        l.density as f32,
                        true,
                        slots[i].as_ref(),
                        cap,
                        &mut rng,
                    );
                    let top = crate::levels::top_of(&o);
                    assert!(top.is_finite(), "lot {i} cap {cap} level {level}");
                }
            }
        }
    }
}
