//! Growth has limits: arrivals follow game time, capacities hold a town back until the mayor
//! builds, jobs and homes must balance, and growth is paid for up front.

use super::*;
use crate::tests_balance::{build_ahead, tend};
use us_gen::{Preset, generate};

fn new_sim(d: Difficulty) -> Sim {
    let t = generate(Preset::Grid, 3);
    Sim::new_with_rules(
        &t,
        hall_sites(&t)[0],
        7,
        2,
        Rules::sandbox().with_difficulty(d),
    )
}

/// Population at the end of each game year.
fn years(s: &mut Sim, n: i32, managed: bool) -> Vec<i64> {
    let mut round = 0;
    let mut out = Vec::new();
    for d in 0..365 * n {
        s.step_day();
        if managed && d % 30 == 5 {
            tend(s, &mut round);
        }
        if d % 365 == 364 {
            out.push(s.population());
        }
    }
    out
}

#[test]
fn a_passive_town_stalls_on_capacity_however_fast_time_runs() {
    // speed only changes how fast game days pass, so fifteen game years is fifteen game years
    let mut s = new_sim(Difficulty::Normal);
    let y = years(&mut s, 15, false);
    assert!(y[14] < 2000, "a town nobody looks after reached {}", y[14]);
    assert!(
        y[14] < y[9] * 12 / 10 + 50,
        "it kept growing: {} to {}",
        y[9],
        y[14]
    );
    let stalls = s.growth_stalls();
    assert!(!stalls.is_empty(), "nothing held it back");
    let news = s.take_events();
    let line = news
        .iter()
        .find(|e| e.kind == EventKind::Limit && e.text.starts_with("Growth has stalled"))
        .expect("a news line says growth stalled");
    assert!(line.text.contains("Build a "), "{}", line.text);
    for c in &stalls {
        assert!(s.capacity(*c).is_full() || s.capacity(*c).ratio() >= 0.97);
    }
}

#[test]
fn a_town_that_is_looked_after_grows_steadily_to_ten_thousand_in_ten_to_fifteen_years() {
    let mut s = new_sim(Difficulty::Normal);
    let y = years(&mut s, 16, true);
    eprintln!("managed Normal: {y:?}");
    for w in y.windows(2) {
        assert!(w[1] >= w[0] * 98 / 100, "it shrank: {y:?}");
    }
    let first = y.iter().position(|&p| p >= 10_000).expect("10,000 people");
    assert!(
        (9..=14).contains(&first),
        "10,000 people in year {}",
        first + 1
    );
    assert!(s.growth_stalls().is_empty());
}

#[test]
fn the_boom_rate_is_bounded_per_game_year_and_follows_difficulty() {
    let mut finals = Vec::new();
    for d in Difficulty::ALL {
        let mut s = new_sim(d);
        let y = years(&mut s, 12, true);
        let mut prev = 0;
        for (n, &p) in y.iter().enumerate() {
            let cap = 600.0 * libm::pow(1.06, n as f64) * d.arrival_pct() as f64 / 100.0 * 2.0;
            assert!(
                ((p - prev) as f64) < cap,
                "{d:?} year {}: {} new people against a bound of {cap:.0}",
                n + 1,
                p - prev
            );
            prev = p;
        }
        finals.push(y[11]);
    }
    assert!(finals[0] > finals[1] && finals[1] > finals[2], "{finals:?}");
}

#[test]
fn families_leave_when_there_are_no_jobs() {
    let mut s = new_sim(Difficulty::Normal);
    let _ = years(&mut s, 7, true);
    let before = s.departures;
    for l in s.lots.iter_mut() {
        if matches!(l.phase, Phase::Built { .. })
            && matches!(class_of(l.kind), Some(Class::Shops | Class::Industry))
        {
            l.phase = Phase::Cleared { since: s.day };
        }
    }
    let homes_before = s.population();
    let mut round = 0;
    for d in 0..150 {
        s.step_day();
        if d % 30 == 5 {
            build_ahead(&mut s, &mut round);
        }
    }
    assert!(s.departures > before, "nobody left");
    assert!(s.population() < homes_before);
    assert!(
        s.take_events()
            .iter()
            .any(|e| e.kind == EventKind::Limit && e.text.contains("not enough jobs"))
    );
}

#[test]
fn growth_is_paid_for_before_the_taxes_arrive() {
    let mut easy = new_sim(Difficulty::Easy);
    let mut hard = new_sim(Difficulty::Hard);
    let t0 = easy.treasury;
    let _ = years(&mut easy, 2, false);
    let _ = years(&mut hard, 2, false);
    assert!(
        easy.growth_spending() > 100_000,
        "{}",
        easy.growth_spending()
    );
    assert!(hard.growth_spending() > 0);
    // the same street costs more on a harder setting
    let s = (0..easy.streets.len())
        .find(|&s| easy.streets[s].addition.is_some() && easy.streets[s].len > 50.0)
        .unwrap();
    assert!(hard.street_cost(s) > easy.street_cost(s) * 2);
    assert!(hard.hookup_cost(LotKind::House) > easy.hookup_cost(LotKind::House));
    assert!(easy.treasury < t0 + 3_000_000);
}

#[test]
fn capacity_comes_from_buildings_the_mayor_places() {
    let mut s = new_sim(Difficulty::Normal);
    let before = s.capacity(Capacity::Power).supply;
    let spot = (0..400)
        .map(|k| {
            let a = k as f64 * 0.7;
            pt(
                s.hall.x + 60.0 + 3.0 * k as f64 * libm::cos(a),
                s.hall.y + 60.0 + 3.0 * k as f64 * libm::sin(a),
            )
        })
        .find(|&p| s.check_civic(CivicKind::Power, p).is_ok())
        .expect("a place for a plant");
    s.treasury = 5_000_000;
    s.place_civic(CivicKind::Power, spot).unwrap();
    for _ in 0..90 {
        s.step_day();
    }
    assert!(s.capacity(Capacity::Power).supply > before + 4000);
    assert_eq!(Capacity::Water.build_hint(), "Build a water tower.");
    assert!(
        Capacity::Water
            .stall_text()
            .contains("water supply is full")
    );
}

#[test]
fn difficulty_is_saved() {
    let mut s = new_sim(Difficulty::Hard);
    for _ in 0..40 {
        s.step_day();
    }
    let bytes = s.save();
    let l = Sim::load(&bytes).unwrap();
    assert_eq!(l.difficulty(), Difficulty::Hard);
    assert_eq!(l.rules(), s.rules());
    assert_eq!(l.fingerprint(), s.fingerprint());
}
