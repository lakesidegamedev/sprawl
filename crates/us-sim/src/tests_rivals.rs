//! Three rival towns: names and personalities, taxes that react to the mayor, trade of
//! households, shops and firms both ways, and a save section that round-trips.

use super::*;
use us_gen::{Preset, generate};

fn new_sim(seed: u64) -> Sim {
    let t = generate(Preset::Grid, 3);
    Sim::new_with_rules(&t, hall_sites(&t)[0], seed, 2, Rules::sandbox())
}

fn run(s: &mut Sim, days: i32) {
    for _ in 0..days {
        s.step_day();
    }
}

#[test]
fn there_are_three_rivals_with_distinct_names_and_personalities() {
    for seed in 1..20u64 {
        let s = new_sim(seed);
        let r = s.rival_towns();
        assert_eq!(r.len(), 3);
        assert_eq!(r[0].name, s.rival().name);
        assert_eq!(r[0].personality, Personality::Suburb);
        assert_eq!(r[1].personality, Personality::Port);
        assert_eq!(r[2].personality, Personality::Mall);
        assert!(r[0].name != r[1].name && r[1].name != r[2].name && r[0].name != r[2].name);
        assert!(r.iter().all(|t| t.pop > 1000));
    }
}

#[test]
fn rivals_drift_to_their_character() {
    let mut s = new_sim(5);
    run(&mut s, 365 * 8);
    let r = s.rival_towns();
    // on average each is cheapest in its own class
    assert!(r[0].tax.homes < r[0].tax.industry, "{:?}", r[0].tax);
    assert!(r[1].tax.industry < r[1].tax.homes, "{:?}", r[1].tax);
    assert!(r[2].tax.shops < r[2].tax.homes, "{:?}", r[2].tax);
}

#[test]
fn a_rival_undercuts_a_tax_rise_and_says_so() {
    let mut s = new_sim(9);
    let mut cut = false;
    s.set_tax_x10(50);
    for round in 0..12 {
        s.set_tax_x10(55 + round * 5);
        run(&mut s, 31);
        cut |= s
            .take_events()
            .iter()
            .any(|e| e.kind == EventKind::Rival && e.text.contains("to undercut yours"));
    }
    assert!(cut);
}

#[test]
fn the_town_loses_people_to_a_cheaper_rival_and_the_rival_grows() {
    let mut s = new_sim(7);
    let before: Vec<i64> = s.rival_towns().iter().map(|t| t.pop).collect();
    s.set_tax_x10(110);
    run(&mut s, 365 * 2);
    let r = s.rival_towns();
    let took: i32 = r
        .iter()
        .map(|t| {
            Class::ALL
                .iter()
                .map(|&c| t.took.get(c) + t.took_last.get(c))
                .sum::<i32>()
        })
        .sum();
    assert!(took > 0, "nothing left for a rival");
    assert!(s.rival_taking_most().is_some());
    assert!(r.iter().zip(&before).any(|(t, b)| t.pop > *b + 20));
}

#[test]
fn a_cheap_well_run_town_wins_people_back() {
    let mut s = new_sim(11);
    s.set_tax_x10(40);
    run(&mut s, 365 * 2);
    let r = s.rival_towns();
    let won: i32 = r
        .iter()
        .map(|t| {
            Class::ALL
                .iter()
                .map(|&c| t.took.get(c).min(0) + t.took_last.get(c).min(0))
                .sum::<i32>()
        })
        .sum();
    assert!(won < 0, "the town won nothing back at 4% tax");
}

#[test]
fn the_why_panel_names_the_rival() {
    let mut s = new_sim(7);
    run(&mut s, 400);
    s.set_tax_x10(110);
    let i = (0..s.lots.len())
        .find(|&i| {
            matches!(s.lots[i].phase, Phase::Built { .. })
                && !s.lots[i].exempt
                && s.lots[i].civic.is_none()
                && class_of(s.lots[i].kind).is_some()
        })
        .expect("a building");
    let w = s.why(i);
    assert!(w.reason.contains("may leave for"), "{}", w.reason);
}

#[test]
fn rivals_save_and_load_mid_game() {
    let mut s = new_sim(7);
    s.set_tax_x10(95);
    run(&mut s, 500);
    let took: i32 = s
        .rival_towns()
        .iter()
        .map(|t| t.took.homes + t.took.shops + t.took.industry + t.took_last.homes)
        .sum();
    let bytes = s.save();
    let mut l = Sim::load(&bytes).unwrap();
    assert_eq!(l.fingerprint(), s.fingerprint());
    for (a, b) in l.rival_towns().iter().zip(s.rival_towns()) {
        assert_eq!(
            (a.name, a.pop, a.tax, a.took),
            (b.name, b.pop, b.tax, b.took)
        );
    }
    run(&mut s, 200);
    run(&mut l, 200);
    assert_eq!(l.fingerprint(), s.fingerprint());
    let _ = took;
}

#[test]
fn an_old_save_without_the_section_gets_three_rivals_from_its_seed() {
    let mut s = new_sim(7);
    let fresh = s.rival_towns();
    let mut l = Sim::load(&s.save()).unwrap();
    l.rivals = rivals::Rivals::new(7);
    let again = l.rival_towns();
    for (a, b) in fresh.iter().zip(&again) {
        assert_eq!((a.name, a.pop), (b.name, b.pop));
    }
}
