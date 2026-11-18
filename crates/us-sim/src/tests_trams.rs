//! Tram lines on the transit stops.

use super::*;
use us_gen::{Preset, generate};

fn grown() -> Sim {
    let t = generate(Preset::Grid, 3);
    let mut s = Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox_unlimited());
    for _ in 0..500 {
        s.step_day();
    }
    s.peak_population = 20_000;
    s.step_unlocks();
    s
}

/// Place transit stops until two of them can be joined by a line; finish them.
fn with_stops(s: &mut Sim) -> (usize, usize) {
    let mut spots: Vec<Pt> = s
        .lots()
        .iter()
        .filter(|l| l.phase == Phase::Vacant)
        .map(|l| l.pos)
        .filter(|&p| s.check_civic(CivicKind::Transit, p).is_ok())
        .collect();
    spots.sort_by(|a, b| a.dist(s.hall).total_cmp(&b.dist(s.hall)));
    let a = s.place_civic(CivicKind::Transit, spots[0]).unwrap();
    let mut b = None;
    for &p in &spots[1..] {
        if p.dist(spots[0]) > 200.0
            && p.dist(spots[0]) < 700.0
            && s.check_civic(CivicKind::Transit, p).is_ok()
        {
            b = s.place_civic(CivicKind::Transit, p).ok();
            if b.is_some() {
                break;
            }
        }
    }
    let b = b.expect("a second stop");
    for _ in 0..90 {
        s.step_day();
    }
    s.recompute_values();
    (a, b)
}

#[test]
fn a_line_needs_two_finished_stops_that_a_road_joins() {
    let mut s = grown();
    assert_eq!(s.check_tram(&[]).err(), Some(TramError::NeedsStops));
    assert_eq!(s.check_tram(&[0]).err(), Some(TramError::NeedsStops));
    assert_eq!(s.check_tram(&[0, 1]).err(), Some(TramError::NotAStop));
    let (a, b) = with_stops(&mut s);
    assert_eq!(s.check_tram(&[a, a]).err(), Some(TramError::TooClose));
    let plan = s.check_tram(&[a, b]).expect("a line");
    assert!(plan.length > 80.0 && plan.cost > 0 && plan.upkeep > 0);
    assert_eq!(plan.stops.len(), 2);
    assert_eq!(plan.stops[0], plan.points[0]);
    assert_eq!(plan.stops[1], *plan.points.last().unwrap());
}

#[test]
fn a_line_costs_money_is_built_in_stages_and_opens_with_news() {
    let mut s = grown();
    let (a, b) = with_stops(&mut s);
    s.take_events();
    s.set_rules(Rules::sandbox());
    let before = s.treasury;
    let plan = s.check_tram(&[a, b]).unwrap();
    let i = s.build_tram(&[a, b]).unwrap();
    assert_eq!(s.treasury, before - plan.cost);
    assert_eq!(s.tram_lines()[i].state, TramState::Track);
    assert_eq!(s.trams_open(), 0);
    assert_eq!(s.tram_upkeep(), 0);
    let v = s.trams_version();
    let mut saw_wires = false;
    for _ in 0..plan.days + 2 {
        s.step_day();
        saw_wires |= s.tram_lines()[i].state == TramState::Wires;
    }
    assert!(saw_wires);
    assert_eq!(s.tram_lines()[i].state, TramState::Open);
    assert!(s.trams_version() > v);
    assert_eq!(s.trams_open(), 1);
    assert_eq!(s.tram_upkeep(), plan.upkeep);
    let news = s.take_events();
    assert!(news.iter().any(|e| e.text.contains("Tram line 1 opened")));
    assert_eq!(s.tram_lines().len(), 1);
}

#[test]
fn an_open_line_helps_the_streets_it_follows_and_the_land_near_its_stops() {
    let mut s = grown();
    let (a, b) = with_stops(&mut s);
    let pre_bonus = s.tram_bonuses();
    assert!(pre_bonus.iter().all(|&v| v == 0.0));
    let pre_arrivals = s.arrival_bonus();
    let pre_cong = s.civic_adjusted_congestion(0.5);
    let plan = s.check_tram(&[a, b]).unwrap();
    s.build_tram(&[a, b]).unwrap();
    for _ in 0..plan.days + 2 {
        s.step_day();
    }
    s.recompute_values();
    assert!(s.arrival_bonus() > pre_arrivals);
    assert!(s.civic_adjusted_congestion(0.5) < pre_cong);
    let near = s
        .lots()
        .iter()
        .position(|l| l.phase != Phase::Hidden && l.pos.dist(plan.stops[0]) < 100.0)
        .unwrap();
    assert!(s.tram_bonuses()[near] > 0.0);
    assert!(s.why(near).reason.contains("tram stop"));
    assert!(s.tram_along(plan.points[plan.points.len() / 2]));
    assert!(s.tram_rate() > 0.0);
}

#[test]
fn a_town_can_have_only_a_few_lines_and_a_line_can_be_closed() {
    let mut s = grown();
    let (a, b) = with_stops(&mut s);
    for _ in 0..MAX_LINES_FOR_TEST {
        s.build_tram(&[a, b]).unwrap();
    }
    assert_eq!(s.build_tram(&[a, b]).err(), Some(TramError::TooManyLines));
    s.remove_tram(0).unwrap();
    assert_eq!(s.tram_lines().len(), MAX_LINES_FOR_TEST - 1);
    assert_eq!(s.remove_tram(9).err(), Some(TramError::NoLine));
}

const MAX_LINES_FOR_TEST: usize = 4;

#[test]
fn frequency_follows_population_and_budget() {
    assert!(crate::trams::frequency_for(9000, 100) > crate::trams::frequency_for(500, 100));
    assert!(crate::trams::frequency_for(9000, 50) < crate::trams::frequency_for(9000, 100));
    assert_eq!(
        crate::trams::frequency_for(9000, 200),
        crate::trams::frequency_for(9000, 100)
    );
    assert!(crate::trams::share_for(8.0) > crate::trams::share_for(1.0));
}

#[test]
fn tram_save_mid_build_and_open_matches() {
    let mut s = grown();
    let (a, b) = with_stops(&mut s);
    s.build_tram(&[a, b]).unwrap();
    for _ in 0..10 {
        s.step_day();
    }
    let bytes = s.save();
    let mut l = Sim::load(&bytes).unwrap();
    assert_eq!(l.fingerprint(), s.fingerprint());
    assert_eq!(l.tram_lines().len(), 1);
    for _ in 0..200 {
        s.step_day();
        l.step_day();
    }
    assert_eq!(l.trams_open(), 1);
    assert_eq!(l.fingerprint(), s.fingerprint());
    assert_eq!(l.save(), s.save());
}

#[test]
fn a_town_without_lines_has_no_section_and_no_effect() {
    let mut s = grown();
    assert_eq!(s.tram_upkeep(), 0);
    assert_eq!(s.tram_relief(), 0.0);
    assert_eq!(s.tram_arrival_bonus(), 0.0);
    assert_eq!(s.tram_shop_demand(), 0.0);
    assert!(
        crate::save_ext::sections(&s)
            .iter()
            .all(|(t, _)| t != b"TRAM")
    );
    let l = Sim::load(&s.save()).unwrap();
    assert_eq!(l.fingerprint(), s.fingerprint());
}
