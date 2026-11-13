use super::*;
use us_gen::{Preset, generate};

fn town() -> Town {
    generate(Preset::Grid, 3)
}

fn sim_after(town: &Town, days: i32) -> Sim {
    let mut s = Sim::new(town, hall_sites(town)[0], 7);
    for _ in 0..days {
        s.step_day();
    }
    s
}

fn levels(s: &Sim) -> [usize; 6] {
    let mut lv = [0; 6];
    for l in s.lots() {
        if matches!(l.phase, Phase::Built { .. }) && !l.exempt {
            lv[l.level as usize] += 1;
        }
    }
    lv
}

#[test]
fn buildings_grow_through_a_construction_phase_and_pay_more() {
    let t = town();
    let mut s = Sim::new(&t, hall_sites(&t)[0], 7);
    let mut saw_site = false;
    for _ in 0..1200 {
        s.step_day();
        saw_site |= s
            .lots()
            .iter()
            .any(|l| matches!(l.phase, Phase::Building { .. }) && l.next_level > l.level);
    }
    assert!(
        saw_site,
        "no lot was rebuilt bigger through a construction phase"
    );
    let lv = levels(&s);
    assert!(lv[2] > 20, "levels {lv:?}");
    for l in s.lots() {
        assert!(l.level >= 1 && l.level <= max_level_of(l.kind));
        if l.exempt {
            assert_eq!(l.level, 1);
        }
    }
    // a level 2 building is worth more tax than a level 1 one of the same kind
    assert!(econ_value(LotKind::House, 2) > econ_value(LotKind::House, 1));
    let ev = s.take_events();
    assert!(ev.iter().any(|e| e.kind == EventKind::Upgrade));
}

fn max_level_of(k: LotKind) -> u8 {
    growth_max(k)
}
fn growth_max(k: LotKind) -> u8 {
    crate::growth::max_level(k)
}
fn econ_value(k: LotKind, level: u8) -> i64 {
    kind_value(k) * crate::econ::level_value_pct(level) / 100
}

#[test]
fn tall_buildings_need_services() {
    let t = town();
    let mut s = Sim::new(&t, hall_sites(&t)[0], 7);
    for d in Dept::ALL {
        s.set_budget(d, 0);
    }
    for _ in 0..1500 {
        s.step_day();
    }
    let lv = levels(&s);
    assert!(lv[1] + lv[2] > 100);
    assert_eq!(lv[3] + lv[4] + lv[5], 0, "levels {lv:?}");
}

#[test]
fn poor_land_declines() {
    let t = town();
    let mut s = sim_after(&t, 1100);
    let before = levels(&s);
    assert!(before[2] > 0);
    for v in s.lot_value.iter_mut() {
        *v = 0.0;
    }
    for _ in 0..300 {
        s.day += 1;
        s.evolve_buildings();
    }
    let after = levels(&s);
    assert!(after[2] < before[2], "{before:?} -> {after:?}");
    assert!(s.take_events().iter().any(|e| e.kind == EventKind::Decline));
}

#[test]
fn a_big_building_takes_empty_lots_beside_it() {
    let t = town();
    let mut s = sim_after(&t, 500);
    let pairs: Vec<(usize, usize)> = {
        let l = s.lots();
        let mut v = Vec::new();
        for i in 0..l.len() {
            if !matches!(l[i].phase, Phase::Built { .. })
                || l[i].exempt
                || class_of(l[i].kind) != Some(Class::Homes)
            {
                continue;
            }
            for j in 0..l.len() {
                if i != j
                    && class_of(l[j].kind) == Some(Class::Homes)
                    && l[j].pos.dist(l[i].pos) < 30.0
                    && l[j].phase != Phase::Hidden
                    && !l[j].exempt
                {
                    v.push((i, j));
                }
            }
        }
        v
    };
    let (i, j) = pairs[0];
    s.lots[j].phase = Phase::Vacant;
    s.vacant.push(j);
    let r = s.lots[i].radius;
    s.merge_neighbours(i, Class::Homes);
    assert_eq!(s.lots()[j].phase, Phase::Hidden);
    assert!(!s.vacant.contains(&j));
    assert!(s.lots()[i].radius > r);
}

#[test]
fn traffic_widens_roads_and_feeds_congestion() {
    let t = town();
    let before = Sim::new(&t, hall_sites(&t)[0], 7);
    let mut s = sim_after(&t, 365 * 4);
    assert_eq!(s.road_traffic().len(), s.road_runs().len());
    assert!(
        s.road_traffic()
            .iter()
            .any(|r| r.volume > 100.0 && r.vc > 0.0)
    );
    assert!(s.roads_version() > 0, "no road changed in four years");
    let rank = |c: RoadClass| match c {
        RoadClass::Local | RoadClass::Alley => 0,
        RoadClass::Collector => 1,
        _ => 2,
    };
    let metres = |s: &Sim, min: i32| -> f64 {
        s.road_runs()
            .iter()
            .filter(|r| rank(r.class) >= min)
            .map(|r| r.points.windows(2).map(|w| w[0].dist(w[1])).sum::<f64>())
            .sum()
    };
    assert!(metres(&s, 1) > metres(&before, 1), "no street was widened");
    assert!(s.road_runs().iter().all(|r| r.points.len() >= 2));
    let c = s.road_values(MapKind::Traffic);
    assert_eq!(c.len(), s.road_runs().len());
    assert!(c.iter().any(|&x| x > 0.0) && c.iter().all(|&x| (0.0..=1.0).contains(&x)));
    assert!(s.history().last().unwrap().congestion > 0.0);
    assert!(s.lot_values(MapKind::Traffic).iter().any(|&x| x > 0.0));
    assert!(s.take_events().iter().any(|e| e.kind == EventKind::Road));
}

#[test]
fn traffic_runs_do_not_depend_on_thread_timing() {
    let t = town();
    let a = sim_after(&t, 900);
    let b = sim_after(&t, 900);
    assert_eq!(a.fingerprint(), b.fingerprint());
    assert_eq!(a.roads_version(), b.roads_version());
}

#[test]
fn fires_burn_then_the_lot_is_rebuilt() {
    let t = town();
    let mut s = sim_after(&t, 400);
    let i = (0..s.lots().len())
        .find(|&i| {
            matches!(s.lots()[i].phase, Phase::Built { .. })
                && !s.lots()[i].exempt
                && class_of(s.lots()[i].kind).is_some()
        })
        .unwrap();
    s.ignite(i, 0.0);
    assert!(s.lots()[i].fire.is_some());
    for _ in 0..crate::dynamic::FIRE_DAYS + 1 {
        s.step_day();
    }
    let l = &s.lots()[i];
    assert!(l.fire.is_none() && l.level == 1);
    assert!(matches!(l.phase, Phase::Building { .. }));
    assert!(s.take_events().iter().any(|e| e.kind == EventKind::Fire));
}

#[test]
fn fire_stations_cut_the_number_of_fires() {
    let t = town();
    let run = |stations: bool| {
        let mut s = sim_after(&t, 500);
        if stations {
            s.treasury = 50_000_000;
            let mut placed = 0;
            'o: for r in (150..1400).step_by(70) {
                for a in 0..24 {
                    let ang = a as f64 * std::f64::consts::TAU / 24.0;
                    let p = pt(
                        s.hall.x + r as f64 * libm::cos(ang),
                        s.hall.y + r as f64 * libm::sin(ang),
                    );
                    if s.place_civic(CivicKind::Fire, p).is_ok() {
                        placed += 1;
                        if placed == 10 {
                            break 'o;
                        }
                    }
                }
            }
            assert!(placed >= 6);
        }
        let start = s.fires_total;
        for _ in 0..700 {
            s.step_day();
        }
        s.fires_total - start
    };
    let (without, with) = (run(false), run(true));
    assert!(with < without, "with stations {with}, without {without}");
}

#[test]
fn a_rival_with_lower_taxes_draws_people_away() {
    let t = town();
    let run = |tax: u32| {
        let mut s = sim_after(&t, 500);
        s.set_tax_x10(tax);
        for _ in 0..500 {
            s.step_day();
        }
        (s.departures, s.take_events())
    };
    let (high, ev) = run(110);
    let (low, _) = run(40);
    assert!(high > low, "departures at 11%: {high}, at 4%: {low}");
    assert!(
        ev.iter()
            .any(|e| e.kind == EventKind::Rival && e.text.contains("left for"))
    );
    assert!(
        ev.iter()
            .any(|e| e.kind == EventKind::Rival && e.text.contains("tax to"))
    );
}

#[test]
fn seasons_sway_demand() {
    let t = town();
    let mut s = Sim::new(&t, hall_sites(&t)[0], 7);
    let jan = s.season_pct(Class::Shops);
    for _ in 0..340 {
        s.step_day();
    }
    assert_eq!(s.date().month, 12);
    assert!(s.season_pct(Class::Shops) > jan + 10);
}

/// The work on the calling thread stays small: a day with a traffic run starting or landing
/// must not take much longer than an ordinary day.
#[test]
fn traffic_does_not_stall_a_day() {
    let t = town();
    let mut s = sim_after(&t, 365);
    let mut worst = std::time::Duration::ZERO;
    for _ in 0..365 {
        let t0 = std::time::Instant::now();
        s.step_day();
        worst = worst.max(t0.elapsed());
    }
    eprintln!("slowest day {worst:?}");
    assert!(worst < std::time::Duration::from_millis(400), "{worst:?}");
}
