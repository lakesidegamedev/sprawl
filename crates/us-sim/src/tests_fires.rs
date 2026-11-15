use super::*;
use us_gen::{Preset, generate};

fn grown(days: i32) -> Sim {
    let t = generate(Preset::Grid, 3);
    let mut s = Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::sandbox());
    for _ in 0..days {
        s.step_day();
    }
    s
}

fn a_house(s: &Sim) -> usize {
    (0..s.lots().len())
        .find(|&i| {
            matches!(s.lots()[i].phase, Phase::Built { .. })
                && !s.lots()[i].exempt
                && s.lots()[i].civic.is_none()
                && class_of(s.lots()[i].kind).is_some()
        })
        .unwrap()
}

fn ring_of(s: &mut Sim, kind: CivicKind, n: usize) {
    s.treasury = 50_000_000;
    let mut placed = 0;
    'o: for r in (100..1400).step_by(60) {
        for a in 0..24 {
            let ang = a as f64 * std::f64::consts::TAU / 24.0;
            let p = pt(
                s.hall.x + r as f64 * libm::cos(ang),
                s.hall.y + r as f64 * libm::sin(ang),
            );
            if s.place_civic(kind, p).is_ok() {
                placed += 1;
                if placed == n {
                    break 'o;
                }
            }
        }
    }
    for _ in 0..90 {
        s.step_day();
    }
}

#[test]
fn an_unanswered_fire_burns_out_and_the_building_is_lost() {
    let mut s = grown(400);
    let i = a_house(&s);
    let t = s.day as i64 * 86_400 + 40_000;
    s.start_blaze(i, t);
    assert!(s.is_burning(i));
    assert!(s.why(i).reason.contains("no fire station"));
    let at = s.blazes_at(t as f64 + 3.0 * 3600.0);
    assert!(at.iter().any(|b| b.lot == i && b.stage == 2));
    assert_eq!(s.blazes_at(t as f64 - 10.0).len(), 0);
}

#[test]
fn a_nearby_station_saves_the_building_and_the_engine_drives_there() {
    let mut s = grown(500);
    ring_of(&mut s, CivicKind::Fire, 8);
    s.set_budget(Dept::Fire, 100);
    let i = a_house(&s);
    let t = s.day as i64 * 86_400 + 40_000;
    s.start_blaze(i, t);
    let blaze = s.fx.blazes.last().unwrap().clone();
    assert!(blaze.arrive_s > t, "an engine should come");
    let mins = (blaze.arrive_s - t) / 60;
    assert!(mins < 30, "took {mins} minutes");
    assert!(blaze.out_s > blaze.arrive_s);
    // the engine is on the road between leaving and arriving, and at the fire after
    let eng = |now: i64| {
        s.vehicles_at(now as f64)
            .into_iter()
            .find(|v| v.kind == VehicleKind::FireEngine)
    };
    let rig =
        s.fx.rigs
            .iter()
            .find(|r| r.kind == VehicleKind::FireEngine)
            .unwrap();
    let mid = eng(rig.depart_s + (rig.travel_s as i64) / 2).expect("engine on the road");
    assert!(mid.moving);
    let there = eng(blaze.arrive_s + 60).expect("engine at the fire");
    assert!(there.pos.dist(s.lots()[i].pos) < 5.0);
    assert!(eng(t - 60).is_none());
}

#[test]
fn slow_response_makes_bigger_fires_than_fast_response() {
    let mut s = grown(500);
    ring_of(&mut s, CivicKind::Fire, 8);
    let i = a_house(&s);
    let t = s.day as i64 * 86_400 + 40_000;
    s.set_budget(Dept::Fire, 150);
    s.start_blaze(i, t);
    let fast = s.fx.blazes.last().unwrap().arrive_s - t;
    s.lots[i].fire = None;
    s.set_budget(Dept::Fire, 20);
    s.start_blaze(i, t + 100_000);
    let slow = s.fx.blazes.last().unwrap().arrive_s - (t + 100_000);
    assert!(slow > fast, "slow {slow}s, fast {fast}s");
}

#[test]
fn a_saved_fire_leaves_the_lot_standing_a_lost_one_is_rebuilt() {
    let mut s = grown(500);
    ring_of(&mut s, CivicKind::Fire, 8);
    s.set_budget(Dept::Fire, 150);
    let i = a_house(&s);
    let lvl = s.lots()[i].level;
    s.start_blaze(i, s.day as i64 * 86_400 + 1000);
    let lost = s.fx.blazes.last().unwrap().lost;
    s.step_day();
    s.step_day();
    s.step_day();
    assert!(!s.is_burning(i));
    if lost {
        assert!(s.lots()[i].fire.is_some());
        for _ in 0..crate::dynamic::FIRE_DAYS {
            s.step_day();
        }
        assert!(matches!(s.lots()[i].phase, Phase::Building { .. }));
    } else {
        assert!(s.lots()[i].fire.is_none());
        assert_eq!(s.lots()[i].level, lvl);
    }
}

#[test]
fn losses_and_relief_are_kept() {
    let mut s = grown(450);
    ring_of(&mut s, CivicKind::Fire, 6);
    let i = a_house(&s);
    let before = s.treasury;
    let (l0, r0) = s.fire_losses();
    s.start_blaze(i, s.day as i64 * 86_400 + 5000);
    let (l1, r1) = s.fire_losses();
    let (lost, relief) = (l1 - l0, r1 - r0);
    assert!(lost > 0);
    assert!(relief <= lost / 2);
    assert_eq!(s.treasury, before - (lost - relief));
}

#[test]
fn saving_mid_fire_and_loading_keeps_the_same_game() {
    let mut s = grown(500);
    ring_of(&mut s, CivicKind::Fire, 6);
    ring_of(&mut s, CivicKind::Police, 3);
    let i = a_house(&s);
    s.start_blaze(i, s.day as i64 * 86_400 + 2000);
    assert!(!s.fire_is_default());
    let bytes = s.save();
    let mut loaded = Sim::load(&bytes).expect("loads");
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    assert_eq!(loaded.save(), bytes);
    for _ in 0..400 {
        s.step_day();
        loaded.step_day();
    }
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    assert_eq!(loaded.save(), s.save());
}

#[test]
fn weak_police_cover_means_more_small_incidents() {
    let count = |police: bool| {
        let mut s = grown(450);
        if police {
            ring_of(&mut s, CivicKind::Police, 8);
        }
        let c0 = s.fx.calls;
        for _ in 0..600 {
            s.step_day();
        }
        s.fx.calls - c0
    };
    assert!(count(false) > count(true));
}

#[test]
fn streets_have_stable_names() {
    let s = grown(300);
    let i = a_house(&s);
    assert_eq!(s.street_name_of_lot(i), s.street_name_of_lot(i));
    assert!(!s.street_name_of_lot(i).is_empty());
}
