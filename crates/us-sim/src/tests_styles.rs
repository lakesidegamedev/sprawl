//! The red rock and coastal styles play like the others: they grow, stay solvent, keep their lots
//! off steep rock and sea, and the weather reaches the low ground by the harbour.

use super::*;
use crate::tests_balance::build_ahead;
use us_gen::{Preset, generate};

fn grow(preset: Preset, tseed: u64, years: i32) -> Sim {
    let town = generate(preset, tseed);
    let mut s = Sim::new_with_rules(&town, hall_sites(&town)[0], 7, 2, Rules::sandbox());
    let mut round = 0;
    for d in 0..365 * years {
        s.step_day();
        if d % 30 == 5 {
            build_ahead(&mut s, &mut round);
        }
    }
    eprintln!(
        "{preset:?} seed {tseed}: pop {} treasury {}",
        s.population(),
        s.treasury
    );
    s
}

#[test]
fn red_rock_and_coast_towns_grow_to_ten_thousand() {
    for (p, seed) in [
        (Preset::Mesa, 1),
        (Preset::Mesa, 4),
        (Preset::Coast, 1),
        (Preset::Coast, 2),
    ] {
        let s = grow(p, seed, 14);
        assert!(
            s.population() >= 9_000,
            "{p:?} seed {seed}: pop {}",
            s.population()
        );
        assert!(
            s.treasury > -2_000_000,
            "{p:?} seed {seed}: treasury {}",
            s.treasury
        );
    }
}

#[test]
fn no_lot_stands_on_steep_ground_or_in_the_sea() {
    for p in [Preset::Mesa, Preset::Coast] {
        for seed in 1..=3 {
            let t = generate(p, seed);
            for l in &t.lots {
                assert!(
                    t.slope_at(l.centre) < 0.15 + 1e-9,
                    "{p:?} {seed}: lot on a steep face"
                );
                assert!(
                    !t.water.iter().any(|w| us_gen_point_in(l.centre, w)),
                    "{p:?} {seed}: lot in the water"
                );
            }
        }
    }
}

fn us_gen_point_in(p: us_gen::Pt, ring: &[us_gen::Pt]) -> bool {
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

#[test]
fn a_storm_floods_the_low_ground_by_the_harbour() {
    let mut s = grow(Preset::Coast, 2, 16);
    s.wx.soak = 8.0;
    s.step_day();
    assert!(
        !s.flooded().is_empty(),
        "no building by the sea was reached"
    );
    // only low ground floods: nothing on the cliff tops
    for &i in s.flooded() {
        let h = s.town.height_at(s.lots()[i].pos);
        assert!(h < 20.0, "a lot {h:.0} m up flooded");
    }
}

#[test]
fn saves_of_the_new_styles_reload_and_play_on_the_same() {
    for (p, seed) in [(Preset::Mesa, 3), (Preset::Coast, 3)] {
        let mut a = grow(p, seed, 3);
        let mut b = Sim::load(&a.save()).expect("loads");
        assert_eq!(a.fingerprint(), b.fingerprint(), "{p:?} reloaded");
        for _ in 0..120 {
            a.step_day();
            b.step_day();
        }
        assert_eq!(a.fingerprint(), b.fingerprint(), "{p:?} played on");
    }
}

#[test]
fn views_of_the_sea_and_the_rock_raise_land_value_and_say_so() {
    for (p, seed, word) in [
        (Preset::Coast, 2, "Sea views"),
        (Preset::Mesa, 4, "Rock views"),
    ] {
        let mut s = grow(p, seed, 16);
        s.step_day();
        let with: Vec<usize> = (0..s.lots().len())
            .filter(|&i| s.scenery_bonus(i) > 0.02)
            .collect();
        assert!(
            with.len() > 20,
            "{p:?}: only {} lots have a view",
            with.len()
        );
        let i = with
            .iter()
            .copied()
            .find(|&i| !matches!(s.lots()[i].phase, Phase::Hidden) && !s.lots()[i].exempt)
            .expect("a built lot with a view");
        assert!(s.why(i).reason.contains(word), "{p:?}: {}", s.why(i).reason);
        // far from the view there is nothing to say
        let far = (0..s.lots().len())
            .find(|&k| s.scenery_bonus(k) == 0.0 && !matches!(s.lots()[k].phase, Phase::Hidden))
            .expect("a lot with no view");
        assert!(!s.why(far).reason.contains("views"), "{p:?}");
    }
    // the five 1.0 styles have no views at all
    let town = generate(Preset::Grid, 3);
    let s = Sim::new_with_rules(&town, hall_sites(&town)[0], 7, 2, Rules::sandbox());
    assert!((0..s.lots().len()).all(|i| s.scenery_bonus(i) == 0.0));
}
