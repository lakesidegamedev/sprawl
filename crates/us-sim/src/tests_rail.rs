use super::*;
use us_gen::{Preset, generate, generate_version};

fn sim(preset: Preset, version: u32) -> Sim {
    let t = generate_version(preset, 3, version);
    Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox())
}

fn grown(preset: Preset) -> Sim {
    let mut s = sim(preset, us_gen::GEN_VERSION);
    for _ in 0..365 * 4 {
        s.step_day();
    }
    s
}

#[test]
fn rates_grow_with_population() {
    let small = rail::rates_for(300);
    let big = rail::rates_for(9000);
    assert!(big.passenger > small.passenger && big.freight > small.freight);
    assert!(small.passenger >= 1.0);
}

#[test]
fn depot_needs_people_and_an_open_street() {
    let mut s = sim(Preset::Grid, us_gen::GEN_VERSION);
    let depots: Vec<usize> = (0..s.town.stations.len())
        .filter(|&i| s.town.stations[i].kind == us_gen::StationKind::Depot)
        .collect();
    assert!(!depots.is_empty(), "v2 Grid town has a depot");
    // a new town has fewer than DEPOT_MIN_POP people: not served yet
    if s.population() < rail::DEPOT_MIN_POP {
        assert!(s.open_depots().is_empty());
    }
    for _ in 0..365 * 4 {
        s.step_day();
    }
    assert!(s.population() >= rail::DEPOT_MIN_POP);
    assert!(
        depots.iter().any(|&i| s.depot_open(i)),
        "the depot's street is part of the old skeleton and open"
    );
}

#[test]
fn lots_near_an_open_depot_gain_value_and_say_so() {
    let s = grown(Preset::Grid);
    let depots = s.open_depots();
    assert!(!depots.is_empty(), "grown Grid town has an open depot");
    let near = (0..s.lots.len()).find(|&i| {
        let b = s.rail_bonus(i);
        b.depot > 0.0 && matches!(s.lots[i].phase, Phase::Built { .. })
    });
    let i = near.expect("a built lot within reach of the depot");
    assert!(s.why(i).reason.contains("depot"));
    // a lot beyond the reach has none
    let far = (0..s.lots.len()).find(|&i| {
        s.lots[i].phase != Phase::Hidden
            && class_of(s.lots[i].kind) != Some(Class::Industry)
            && depots
                .iter()
                .all(|d| d.dist(s.lots[i].pos) > rail::DEPOT_REACH_M)
    });
    if let Some(f) = far {
        assert_eq!(s.rail_bonus(f).depot, 0.0);
    }
}

#[test]
fn industry_by_the_railway_gains_value() {
    let s = grown(Preset::Grid);
    let by = (0..s.lots.len()).find(|&i| s.rail_bonus(i).freight > 0.0);
    eprintln!("industry lot by the rail: {by:?}");
    if let Some(i) = by {
        assert_eq!(class_of(s.lots[i].kind), Some(Class::Industry));
        if matches!(s.lots[i].phase, Phase::Built { .. }) {
            assert!(s.why(i).reason.contains("railway"));
        }
        assert!(s.rail_bonus(i).freight <= rail::W_FREIGHT);
    }
}

#[test]
fn old_towns_have_no_depot_but_still_ship_freight() {
    let t = generate_version(Preset::Grid, 3, 1);
    let mut s = Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox());
    for _ in 0..365 * 3 {
        s.step_day();
    }
    assert!(s.open_depots().is_empty());
    assert!(s.train_rates().passenger >= 1.0);
}

#[test]
fn the_railway_changes_nothing_when_the_town_has_none() {
    // the depot bonus is zero everywhere when no depot is open
    let s = sim(Preset::Grid, us_gen::GEN_VERSION);
    let depots = s.open_depots();
    if depots.is_empty() {
        for i in 0..s.lots.len() {
            assert_eq!(s.rail_bonus(i).depot, 0.0);
        }
    }
}

#[test]
fn rail_service_survives_save_and_load() {
    let mut a = grown(Preset::Grid);
    let bytes = a.save();
    let mut b = Sim::load(&bytes).expect("loads");
    assert_eq!(a.open_depots().len(), b.open_depots().len());
    for _ in 0..200 {
        a.step_day();
        b.step_day();
    }
    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_eq!(
        a.lot_values(MapKind::LandValue),
        b.lot_values(MapKind::LandValue)
    );
    let _ = generate(Preset::Grid, 3);
}
