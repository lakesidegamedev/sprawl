//! Towns are about twice as dense as before: growth, jobs, money and traffic must stay sane.

use super::*;
use us_gen::{Preset, generate};

fn run(town: &Town, days: i32) -> Sim {
    let mut s = Sim::new_with_rules(town, hall_sites(town)[0], 7, 2, Rules::sandbox());
    s.set_tax_x10(70);
    let mut round = 0;
    for d in 0..days {
        s.step_day();
        if d % 30 == 5 {
            crate::tests_balance::tend(&mut s, &mut round);
        }
    }
    s
}

#[test]
fn dense_towns_grow_and_stay_solvent_without_gridlock() {
    for (preset, seed) in [
        (Preset::Grid, 3),
        (Preset::Sunbelt, 2),
        (Preset::Winding, 1),
    ] {
        let town = generate(preset, seed);
        let s = run(&town, 3000);
        let (income, expense) = s.monthly_flow();
        eprintln!(
            "{} {seed}: day {} built {} pop {} jobs {} treasury {} income {income} expense {expense} cong {:.2} left {}",
            preset.name(),
            s.day,
            s.built_count(),
            s.population(),
            s.jobs(),
            s.treasury,
            s.avg_congestion(),
            s.additions_left()
        );
        // denser central lots, and floods and fires in river towns, mean fewer buildings for the
        // same people; the people and jobs checks below say the town grew
        assert!(s.built_count() > 200, "built {}", s.built_count());
        assert!(s.population() > 1500, "population {}", s.population());
        assert!(s.jobs() > 300, "jobs {}", s.jobs());
        // the town pays its way: income covers at least most of what it spends
        assert!(
            income * 10 > expense * 8,
            "income {income} expense {expense}"
        );
        assert!(
            s.avg_congestion() < 0.8,
            "congestion {}",
            s.avg_congestion()
        );
    }
}

#[test]
fn tall_buildings_gather_at_the_centres_and_suburbs_stay_low() {
    let town = generate(Preset::Grid, 3);
    let s = run(&town, 365 * 20);
    let (mut hi, mut nh, mut lo, mut nl) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut tall_central = 0;
    for (i, l) in s.lots().iter().enumerate() {
        if !matches!(l.phase, Phase::Built { .. }) || class_of(l.kind).is_none() || l.exempt {
            continue;
        }
        let d = s.lot_density(i);
        assert!(
            l.level <= max_level_at(l.kind, d),
            "{:?} at level {} on density {d:.2}",
            l.kind,
            l.level
        );
        if d >= 0.6 {
            hi += l.level as f64;
            nh += 1.0;
            tall_central += (l.level >= 4) as usize;
        } else if d < 0.2 {
            lo += l.level as f64;
            nl += 1.0;
        }
    }
    let (hi, lo) = (hi / nh.max(1.0), lo / nl.max(1.0));
    eprintln!(
        "central mean level {hi:.2} over {nh}, suburban {lo:.2} over {nl}, tall central {tall_central}"
    );
    assert!(nh > 30.0 && nl > 100.0);
    assert!(hi > lo + 0.6, "central {hi:.2}, suburban {lo:.2}");
    assert!(
        tall_central >= 5,
        "only {tall_central} tall buildings downtown"
    );
}

#[test]
fn builders_fill_streets_from_the_buildings_already_standing() {
    let town = generate(Preset::Winding, 2);
    let mut s = Sim::new_with_rules(&town, hall_sites(&town)[0], 7, 2, Rules::sandbox());
    let mut round = 0;
    let (mut started, mut beside) = (0usize, 0usize);
    for d in 0..900 {
        let before: Vec<bool> = s
            .lots()
            .iter()
            .map(|l| matches!(l.phase, Phase::Building { .. } | Phase::Built { .. }))
            .collect();
        s.step_day();
        if d % 30 == 5 {
            crate::tests_balance::tend(&mut s, &mut round);
        }
        for i in 0..before.len() {
            let now = matches!(s.lots()[i].phase, Phase::Building { .. });
            if now && !before[i] && s.lots()[i].civic.is_none() {
                started += 1;
                let nb = s.near[i].iter().any(|&j| before[j as usize]);
                beside += nb as usize;
            }
        }
    }
    eprintln!("{beside} of {started} new buildings went up beside one already there");
    // without the preference about half go up beside a standing building (rival towns shift
    // which buildings start, so the bar sits a little under the usual 60 percent)
    assert!(started > 80);
    assert!(beside * 100 >= started * 55, "{beside} of {started}");
}

#[test]
fn mountain_towns_grow_stay_solvent_and_floods_spare_the_high_ground() {
    for (preset, seed) in [(Preset::Gorge, 1), (Preset::Valley, 1)] {
        let town = generate(preset, seed);
        let s = run(&town, 7500);
        let (income, expense) = s.monthly_flow();
        eprintln!(
            "{} {seed}: day {} built {} pop {} jobs {} treasury {} income {income} expense {expense} cong {:.2} left {}",
            preset.name(),
            s.day,
            s.built_count(),
            s.population(),
            s.jobs(),
            s.treasury,
            s.avg_congestion(),
            s.additions_left()
        );
        assert!(s.built_count() > 150, "built {}", s.built_count());
        assert!(s.population() > 10_000, "population {}", s.population());
        assert!(s.jobs() > 300, "jobs {}", s.jobs());
        assert!(
            income * 10 > expense * 7,
            "income {income} expense {expense}"
        );
        assert!(
            s.avg_congestion() < 0.8,
            "congestion {}",
            s.avg_congestion()
        );
        // enough buildable land for a town of about ten thousand
        let lots = town.lots.len();
        assert!(lots > 3000, "{} has {lots} lots", preset.name());
    }
}

#[test]
fn towers_require_a_mature_served_cluster() {
    let town = generate(Preset::Grid, 3);
    let mut sim = Sim::new_with_rules(&town, hall_sites(&town)[0], 7, 2, Rules::sandbox());
    let i = sim
        .lots
        .iter()
        .enumerate()
        .find(|(i, lot)| {
            lot.kind == LotKind::Office && sim.lot_density(*i) >= 0.78 && sim.near[*i].len() >= 3
        })
        .map(|(i, _)| i)
        .expect("central office lot");
    for lot in &mut sim.lots {
        if class_of(lot.kind) == Some(Class::Homes) {
            lot.phase = Phase::Built {
                since: 0,
                tax_x10: TAX_DEFAULT_X10,
            };
            lot.level = 3;
        }
    }
    sim.lot_value[i] = 0.8;
    for service in [
        Service::Police,
        Service::Fire,
        Service::Schools,
        Service::Health,
    ] {
        sim.coverage[service.index()][i] = 0.9;
    }
    let near = sim.near[i][..3].to_vec();
    for &j in &near {
        sim.lots[j as usize].phase = Phase::Vacant;
    }
    sim.near[i] = near.clone();
    assert_eq!(sim.lot_cap(i, sim.population()), 3);
    for j in near {
        sim.lots[j as usize].phase = Phase::Built {
            since: 0,
            tax_x10: TAX_DEFAULT_X10,
        };
        sim.lots[j as usize].level = 3;
    }
    assert_eq!(sim.lot_cap(i, sim.population()), 5);
    // A planned office parcel has no generator lot. Its zoning density must
    // qualify it for the same skyline tier when the neighborhood supports it.
    sim.rules.city_planning = true;
    sim.lots[i].src = sim.town.lots.len() + i;
    assert!(sim.is_planned_lot(i));
    assert_eq!(sim.lot_cap(i, sim.population()), 5);
    sim.lot_value[i] = 0.62;
    assert_eq!(sim.lot_cap(i, sim.population()), 4);
    for service in [
        Service::Police,
        Service::Fire,
        Service::Schools,
        Service::Health,
    ] {
        sim.coverage[service.index()][i] = 0.0;
    }
    assert_eq!(sim.lot_cap(i, sim.population()), 3);
}
