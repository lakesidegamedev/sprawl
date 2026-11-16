//! The opening of a game: an untended town fills in during its first year and money holds.
//! Also the growth alerts, the goal list and the approval cost of a blocked town.

use super::*;
use us_gen::{Preset, generate};

const TOWNS: [(Preset, u64); 4] = [
    (Preset::Grid, 3),
    (Preset::Sunbelt, 2),
    (Preset::Winding, 1),
    (Preset::LakeTown, 5),
];

fn untended(p: Preset, ts: u64) -> Sim {
    let t = generate(p, ts);
    Sim::new_with_rules(&t, hall_sites(&t)[0], 7, 2, Rules::standard())
}

/// Buildings started or standing (homes, shops and industry), not counting the town's own.
fn buildings(s: &Sim) -> usize {
    s.built_count() + s.building_count()
}

/// What one untended town did: buildings started by day 10, people at day 365, the most people
/// seen in the last quarter of the first year, and the lowest treasury over `years`.
struct Opening {
    b10: usize,
    p365: i64,
    peak_q4: i64,
    low: i64,
    end: i64,
    over: bool,
}

fn probe(p: Preset, ts: u64, years: i32) -> Opening {
    let mut s = untended(p, ts);
    let mut o = Opening {
        b10: 0,
        p365: 0,
        peak_q4: 0,
        low: i64::MAX,
        end: 0,
        over: false,
    };
    for d in 1..=365 * years {
        s.step_day();
        if d == 10 {
            o.b10 = buildings(&s);
        }
        if (270..=365).contains(&d) {
            o.peak_q4 = o.peak_q4.max(s.population());
        }
        if d == 365 {
            o.p365 = s.population();
        }
        o.low = o.low.min(s.treasury);
    }
    o.end = s.treasury;
    o.over = s.game_over.is_some();
    eprintln!(
        "{p:?} {ts}: day 10 buildings {}, day 365 people {} (peak in last quarter {}), lowest treasury {}, end {}, game over {}",
        o.b10, o.p365, o.peak_q4, o.low, o.end, o.over
    );
    o
}

fn check_opening(o: &Opening, name: &str) {
    assert!(o.b10 >= 10, "{name}: {} buildings started by day 10", o.b10);
    // a lightning storm can burn the first town on the very day, so the last quarter counts
    assert!(
        (600..=1100).contains(&o.peak_q4),
        "{name}: peak people in the last quarter was {}",
        o.peak_q4
    );
    assert!(o.p365 <= 1100, "{name}: {} people at day 365", o.p365);
    assert!(!o.over, "{name}: the game ended");
}

/// Prints the opening of every test town over four years; run with
/// `cargo test -p us-sim --release probe_opening_report -- --ignored --nocapture`.
#[test]
#[ignore]
fn probe_opening_report() {
    for (p, t) in TOWNS {
        let o = probe(p, t, 4);
        check_opening(&o, &format!("{p:?}"));
        assert!(o.end > -400_000, "{p:?}: treasury ended at {}", o.end);
    }
}

#[test]
fn an_untended_town_fills_in_during_its_first_year() {
    for (p, t) in TOWNS {
        let o = probe(p, t, 1);
        check_opening(&o, &format!("{p:?}"));
    }
}

#[test]
fn builders_start_on_day_one() {
    let mut s = untended(Preset::Grid, 3);
    s.step_day();
    assert!(
        s.building_count() >= 3,
        "{} started on day one",
        s.building_count()
    );
}

#[test]
fn the_settlers_wave_fades_out_after_a_year() {
    let mut s = untended(Preset::Grid, 3);
    assert!(s.settlers() > 0.99);
    s.day = 200;
    assert!(s.settlers() > 0.3 && s.settlers() < 0.6);
    s.day = 400;
    assert_eq!(s.settlers(), 0.0);
}

#[test]
fn an_untended_town_survives_four_years_on_every_test_town() {
    for (p, t) in TOWNS {
        let o = probe(p, t, 4);
        assert!(!o.over, "{p:?} went bankrupt");
        assert!(o.end > -400_000, "{p:?}: treasury ended at {}", o.end);
    }
}

fn run_until_stalled(s: &mut Sim) {
    for _ in 0..365 * 6 {
        s.step_day();
        if !s.growth_stalls().is_empty() {
            return;
        }
    }
}

#[test]
fn a_full_capacity_shows_an_alert_that_clears_when_it_is_built() {
    let mut s = untended(Preset::Grid, 3);
    run_until_stalled(&mut s);
    let alerts = s.growth_alerts();
    assert!(
        alerts.iter().any(|a| a.blocked),
        "no blocked alert: {alerts:?}"
    );
    let a = alerts.iter().find(|a| a.blocked).unwrap();
    assert!(a.headline.starts_with("Growth stopped"), "{}", a.headline);
    assert!(a.cost > 0 && a.advice.contains('$'), "{}", a.advice);
    assert_eq!(a.build, a.capacity.civic());
    // pay for the building and let it open: that alert goes away
    s.treasury += 5_000_000;
    let kind = a.build;
    let cap = a.capacity;
    for _ in 0..40 {
        let spot = s
            .lots()
            .iter()
            .filter(|l| l.phase == Phase::Vacant)
            .map(|l| l.pos)
            .find(|&p| s.check_civic(kind, p).is_ok());
        if let Some(p) = spot {
            s.place_civic(kind, p).expect("placed");
        }
        for _ in 0..90 {
            s.step_day();
        }
        if !s.growth_alerts().iter().any(|a| a.capacity == cap) {
            return;
        }
    }
    panic!("the alert for {cap:?} never cleared");
}

#[test]
fn a_town_close_to_full_gets_a_softer_warning() {
    let mut s = untended(Preset::Grid, 3);
    let mut seen_soft = false;
    for _ in 0..365 * 4 {
        s.step_day();
        if s.growth_alerts().iter().any(|a| !a.blocked) {
            seen_soft = true;
            break;
        }
    }
    assert!(seen_soft, "no early warning before the wall");
}

#[test]
fn a_town_blocked_for_months_loses_a_little_approval() {
    let mut s = untended(Preset::Grid, 3);
    run_until_stalled(&mut s);
    s.goal.blocked_months = 0;
    let base = s.compute_approval().total;
    s.goal.blocked_months = 8;
    let after = s.compute_approval().total;
    assert!(after < base && base - after <= 5, "{base} -> {after}");
    assert!(s.blocked_approval() < 0.0);
}

#[test]
fn goals_tick_off_and_pay_through_the_ledger() {
    let mut s = untended(Preset::Grid, 3);
    assert_eq!(s.goals().len(), 6);
    assert!(s.goals_visible());
    let before = s.treasury;
    s.set_tax_x10(TAX_DEFAULT_X10 + 5);
    for _ in 0..70 {
        s.step_day();
    }
    let done: Vec<_> = s
        .goals()
        .into_iter()
        .filter(|g| g.done)
        .map(|g| g.goal)
        .collect();
    assert!(done.contains(&Goal::Budget), "{done:?}");
    assert!(
        s.take_events()
            .iter()
            .any(|e| e.text.starts_with("Goal met"))
    );
    // the grant was paid at a monthly ledger and shows there
    let mut paid = 0;
    let mut s2 = untended(Preset::Grid, 3);
    s2.set_tax_x10(TAX_DEFAULT_X10 + 5);
    for _ in 0..70 {
        s2.step_day();
        paid = paid.max(s2.ledger_now().grants);
    }
    assert_eq!(paid, 5_000);
    let _ = before;
}

#[test]
fn the_card_closes_and_stays_closed() {
    let mut s = untended(Preset::Grid, 3);
    s.dismiss_goals();
    assert!(!s.goals_visible());
    let l = Sim::load(&s.save()).expect("loads");
    assert!(!l.goals_visible());
}

#[test]
fn goal_state_survives_a_save_and_both_copies_step_alike() {
    let mut s = untended(Preset::Grid, 3);
    s.set_tax_x10(TAX_DEFAULT_X10 + 5);
    for _ in 0..200 {
        s.step_day();
    }
    assert!(s.goals().iter().any(|g| g.done));
    let mut l = Sim::load(&s.save()).expect("loads");
    assert_eq!(l.fingerprint(), s.fingerprint());
    for _ in 0..150 {
        s.step_day();
        l.step_day();
    }
    assert_eq!(l.fingerprint(), s.fingerprint());
    assert_eq!(
        l.goals().iter().filter(|g| g.done).count(),
        s.goals().iter().filter(|g| g.done).count()
    );
}

#[test]
fn an_old_big_town_is_not_nagged_with_goals() {
    let mut s = untended(Preset::Grid, 3);
    for _ in 0..400 {
        s.step_day();
    }
    s.goal = Default::default();
    s.goals_for_old_save();
    assert!(!s.goals_visible());
    assert!(s.goals().iter().all(|g| g.done));
    assert_eq!(s.goal.grant_due, 0);
}

#[test]
fn an_old_small_town_keeps_goals_it_has_not_met() {
    let mut s = untended(Preset::Grid, 3);
    for _ in 0..100 {
        s.step_day();
    }
    s.goal = Default::default();
    s.goals_for_old_save();
    assert!(s.goals_visible());
    assert_eq!(s.goal.grant_due, 0);
}

/// The first four years of an untended town under every difficulty stay in their order.
#[test]
fn difficulty_still_scales_the_opening() {
    let town = generate(Preset::Winding, 1);
    let at = |d: Difficulty| {
        let mut s = Sim::new_with_rules(
            &town,
            hall_sites(&town)[0],
            7,
            2,
            Rules::standard().with_difficulty(d),
        );
        for _ in 0..365 {
            s.step_day();
        }
        s.population().max(s.peak_population)
    };
    let (e, n, h) = (
        at(Difficulty::Easy),
        at(Difficulty::Normal),
        at(Difficulty::Hard),
    );
    eprintln!("easy {e} normal {n} hard {h}");
    assert!(e >= n && n >= h, "{e} {n} {h}");
}

/// Fires in a day, and the biggest share of the town burned in one month, over `years`.
fn fire_probe(p: Preset, ts: u64, years: i32) -> (u32, u32, f64) {
    let mut s = untended(p, ts);
    let (mut worst_day, mut strikes_days) = (0u32, 0u32);
    let mut worst_month = 0.0f64;
    let mut month_start = (0u32, 1usize);
    let mut prev = 0u32;
    for d in 1..=365 * years {
        s.step_day();
        let today = s.fires_total - prev;
        prev = s.fires_total;
        worst_day = worst_day.max(today);
        if !s.lightning_strikes().is_empty() {
            strikes_days += 1;
        }
        if d % 30 == 0 {
            let lost = s.fires_total - month_start.0;
            let share = lost as f64 / month_start.1.max(1) as f64;
            if month_start.1 > 40 {
                worst_month = worst_month.max(share);
            }
            month_start = (s.fires_total, buildings(&s));
        }
    }
    let bolts = s
        .take_events()
        .iter()
        .filter(|e| e.text.starts_with("Lightning hit"))
        .count();
    eprintln!("  lightning fires {bolts}");
    eprintln!(
        "{p:?} {ts}: fires total {}, worst day {worst_day}, storm days with a strike {strikes_days}, worst month share of buildings on fire {:.3}",
        s.fires_total, worst_month
    );
    (s.fires_total, worst_day, worst_month)
}

#[test]
#[ignore]
fn probe_fires() {
    for (p, t) in TOWNS {
        fire_probe(p, t, 4);
    }
}

#[test]
fn a_storm_starts_only_a_few_fires() {
    for (p, t) in TOWNS {
        let mut s = untended(p, t);
        let (mut worst, mut total, mut storm_days) = (0usize, 0usize, 0usize);
        for _ in 0..365 * 4 {
            s.step_day();
            let bolts = s
                .take_events()
                .iter()
                .filter(|e| e.text.starts_with("Lightning hit"))
                .count();
            worst = worst.max(bolts);
            total += bolts;
            if !s.lightning_strikes().is_empty() {
                storm_days += 1;
            }
        }
        assert!(worst <= 3, "{p:?}: {worst} lightning fires in one day");
        assert!(
            total <= storm_days / 3 + 1,
            "{p:?}: {total} lightning fires over {storm_days} storm days"
        );
    }
}

#[test]
fn an_untended_town_never_loses_much_of_itself_to_fire_in_a_month() {
    for (p, t) in TOWNS {
        let (_, worst_day, worst_month) = fire_probe(p, t, 4);
        assert!(worst_day <= 3, "{p:?}: {worst_day} fires on one day");
        assert!(
            worst_month <= 0.12,
            "{p:?}: {:.0}% of the buildings burned in one month",
            worst_month * 100.0
        );
    }
}

#[test]
fn the_first_fire_in_a_town_without_a_station_says_so() {
    let mut s = untended(Preset::Grid, 3);
    let built = |s: &Sim| {
        (0..s.lots.len()).find(|&i| {
            matches!(s.lots[i].phase, Phase::Built { .. }) && class_of(s.lots[i].kind).is_some()
        })
    };
    while built(&s).is_none() {
        s.step_day();
    }
    assert_eq!(s.fires_total, 0);
    let i = built(&s).unwrap();
    s.take_events();
    s.ignite(i, 0.0);
    assert!(
        s.take_events()
            .iter()
            .any(|e| e.text.contains("No fire station: fires will spread")),
        "no warning"
    );
}

#[test]
fn growth_stops_when_demand_reaches_supply_not_a_month_later() {
    for (p, t) in TOWNS {
        let mut s = untended(p, t);
        let mut worst = 0.0f64;
        for _ in 0..365 * 3 {
            s.step_day();
            for c in Capacity::ALL {
                let st = s.capacity_with(c, false);
                if st.supply > 0 {
                    worst = worst.max(st.ratio());
                }
            }
        }
        assert!(
            worst <= 1.10,
            "{p:?}: a capacity reached {:.0}% of its supply",
            worst * 100.0
        );
    }
}

/// Where to look for pictures: the first day a capacity blocks growth, the first day a warning
/// shows, and the day after the first lightning fire, on each test town.
#[test]
#[ignore]
fn probe_picture_days() {
    for (p, t) in TOWNS {
        let mut s = untended(p, t);
        let (mut block, mut warn, mut bolt) = (0, 0, 0);
        for d in 1..=365 * 3 {
            s.step_day();
            if block == 0 && !s.growth_stalls().is_empty() {
                block = d;
            }
            if warn == 0 && s.growth_alerts().iter().any(|a| !a.blocked) {
                warn = d;
            }
            if bolt == 0
                && s.take_events()
                    .iter()
                    .any(|e| e.text.starts_with("Lightning hit"))
            {
                bolt = d;
            }
        }
        eprintln!(
            "{p:?} {t}: first warning day {warn}, first block day {block}, first bolt fire day {bolt}"
        );
    }
}
