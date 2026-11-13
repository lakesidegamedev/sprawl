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

/// A free spot beside a revealed road, found the way the UI would: try points around the hall.
fn free_spot(s: &mut Sim, kind: CivicKind) -> Option<(usize, Pt)> {
    for r in (120..900).step_by(30) {
        for a in 0..24 {
            let ang = a as f64 * std::f64::consts::TAU / 24.0;
            let p = pt(
                s.hall.x + r as f64 * libm::cos(ang),
                s.hall.y + r as f64 * libm::sin(ang),
            );
            if let Ok(i) = s.place_civic(kind, p) {
                return Some((i, p));
            }
        }
    }
    None
}

#[test]
fn tax_classes_are_separate_and_set_all_still_works() {
    let t = town();
    let mut s = sim_after(&t, 700);
    s.set_tax_x10(70);
    assert_eq!(s.tax_x10(), 70);
    let base = s.compute_ledger();
    s.set_tax_class(Class::Homes, 140);
    let up = s.compute_ledger();
    assert!(up.income.homes > base.income.homes * 3 / 2);
    assert_eq!(up.income.shops, base.income.shops);
    assert_eq!(up.income.industry, base.income.industry);
    s.set_tax_class(Class::Shops, 0);
    assert_eq!(s.compute_ledger().income.shops, 0);
    s.set_tax_x10(500);
    assert_eq!(s.tax_rates(), ByClass::all(TAX_MAX_X10));
}

#[test]
fn a_class_tax_only_lowers_its_own_demand() {
    let t = town();
    let mut a = sim_after(&t, 600);
    let mut b = sim_after(&t, 600);
    a.set_tax_class(Class::Industry, 180);
    a.recompute_values();
    b.recompute_values();
    assert!(a.class_demand(Class::Industry) < b.class_demand(Class::Industry));
    assert_eq!(a.class_demand(Class::Homes), b.class_demand(Class::Homes));
}

#[test]
fn budgets_scale_spending_and_coverage() {
    let t = town();
    let mut s = sim_after(&t, 700);
    let base = s.compute_ledger();
    s.set_budget(Dept::Police, 150);
    s.set_budget(Dept::Roads, 50);
    let l = s.compute_ledger();
    assert!(l.spending[Dept::Police.index()] > base.spending[Dept::Police.index()]);
    assert!(l.spending[Dept::Roads.index()] < base.spending[Dept::Roads.index()]);
    assert_eq!(
        l.spending[Dept::Fire.index()],
        base.spending[Dept::Fire.index()]
    );
    s.set_budget(Dept::Fire, 999);
    assert_eq!(s.budget_pct(Dept::Fire), BUDGET_MAX_PCT);
    // starving a department empties its coverage
    s.set_budget(Dept::Schools, 0);
    s.recompute_values();
    assert!(
        s.lot_values(MapKind::Coverage(Service::Schools))
            .iter()
            .all(|&v| v == 0.0)
    );
}

#[test]
fn bonds_bring_cash_then_cost_money_until_repaid() {
    let t = town();
    let mut s = sim_after(&t, 400);
    let cash = s.treasury;
    let net = s.monthly_net();
    s.take_loan().unwrap();
    assert_eq!(s.treasury, cash + BOND_SIZE);
    assert_eq!(s.debt(), BOND_SIZE);
    assert!(s.monthly_net() < net, "bond payments should cost money");
    let before = s.history().len();
    for _ in 0..365 {
        s.step_day();
    }
    assert!(s.debt() < BOND_SIZE && s.debt() > 0);
    let last = s.history().last().unwrap();
    assert!(last.debt_service > 0 && last.debt == s.debt());
    assert!(s.history().len() > before);
    // pay it off
    s.treasury = 1_000_000;
    assert_eq!(s.repay_loan().unwrap(), s.history().last().unwrap().debt);
    assert_eq!(s.debt(), 0);
    assert_eq!(s.repay_loan(), Err(LoanError::NothingToRepay));
}

#[test]
fn credit_limit_follows_the_tax_base() {
    let t = town();
    let mut s = Sim::new(&t, hall_sites(&t)[0], 7);
    assert_eq!(s.credit_limit(), MIN_CREDIT_LIMIT);
    assert!(s.take_loan().is_ok());
    assert_eq!(s.take_loan(), Err(LoanError::LimitReached));
    let mut grown = sim_after(&t, 1200);
    assert!(grown.credit_limit() > MIN_CREDIT_LIMIT);
    let expect = grown.credit_limit() / BOND_SIZE;
    let mut n = 0;
    while grown.take_loan().is_ok() {
        n += 1;
        assert!(n <= expect);
    }
    assert!(n >= 2 && n == expect, "{n} bonds, limit allows {expect}");
    assert!(grown.debt() <= grown.credit_limit());
}

#[test]
fn history_has_one_sample_per_month_for_a_long_game() {
    let t = town();
    let mut s = sim_after(&t, 365 * 12);
    let h = s.history();
    assert!((143..=145).contains(&h.len()), "len {}", h.len());
    for w in h.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        let months = (b.year - a.year) * 12 + b.month as i32 - a.month as i32;
        assert_eq!(
            months, 1,
            "{}/{} then {}/{}",
            a.year, a.month, b.year, b.month
        );
        assert!(b.day > a.day);
    }
    let last = h.last().unwrap();
    assert!(last.population > 0 && last.jobs > 0 && last.built > 100);
    assert!(last.income_total() > 0 && last.spending_total() > 0);
    assert!(last.avg_land_value > 0.1 && last.avg_land_value < 1.0);
    assert_eq!(last.tax_x10, s.tax_rates());
    let _ = s.take_events();
}

#[test]
fn land_value_rises_with_a_park_and_a_school() {
    let t = town();
    let mut with = sim_after(&t, 600);
    let mut without = sim_after(&t, 600);
    let (_, p) = free_spot(&mut with, CivicKind::School).expect("a spot for a school");
    let (_, q) = free_spot(&mut with, CivicKind::Park).expect("a spot for a park");
    for _ in 0..150 {
        with.step_day();
        without.step_day();
    }
    let near = |s: &Sim, c: Pt, r: f64, kind: MapKind| -> f64 {
        let v = s.lot_values(kind);
        let idx: Vec<usize> = (0..s.lots().len().min(without.lots().len()))
            .filter(|&i| {
                s.lots()[i].pos.dist(c) < r
                    && matches!(s.lots()[i].phase, Phase::Built { .. })
                    && !s.lots()[i].exempt
                    && s.lots()[i].civic.is_none()
            })
            .collect();
        assert!(!idx.is_empty());
        idx.iter().map(|&i| v[i] as f64).sum::<f64>() / idx.len() as f64
    };
    assert!(near(&with, p, 400.0, MapKind::Coverage(Service::Schools)) > 0.1);
    assert!(near(&without, p, 400.0, MapKind::Coverage(Service::Schools)) < 0.05);
    assert!(near(&with, q, 300.0, MapKind::Coverage(Service::Parks)) > 0.1);
    let (lw, lo) = (
        near(&with, p, 300.0, MapKind::LandValue),
        near(&without, p, 300.0, MapKind::LandValue),
    );
    assert!(lw > lo, "school area {lw} vs {lo}");
    let (kw, ko) = (
        near(&with, q, 250.0, MapKind::LandValue),
        near(&without, q, 250.0, MapKind::LandValue),
    );
    assert!(kw > ko, "park area {kw} vs {ko}");
}

#[test]
fn placing_civic_buildings_checks_money_and_place() {
    let t = town();
    let mut s = sim_after(&t, 500);
    assert_eq!(
        s.place_civic(CivicKind::Park, pt(-5.0, 10.0)),
        Err(PlaceError::OutOfBounds)
    );
    assert_eq!(
        s.place_civic(CivicKind::Park, s.hall),
        Err(PlaceError::Occupied)
    );
    let cash = s.treasury;
    let (i, _) = free_spot(&mut s, CivicKind::Clinic).unwrap();
    assert_eq!(s.treasury, cash - CivicKind::Clinic.cost());
    assert!(matches!(s.lots()[i].phase, Phase::Building { .. }));
    assert_eq!(s.lots()[i].civic, Some(CivicKind::Clinic));
    // not enough money
    s.treasury = 100;
    let mut err = None;
    for r in (150..700).step_by(40) {
        for a in 0..16 {
            let ang = a as f64 * std::f64::consts::TAU / 16.0;
            let p = pt(
                s.hall.x + r as f64 * libm::cos(ang),
                s.hall.y + r as f64 * libm::sin(ang),
            );
            if let Err(PlaceError::TooPoor { cost }) = s.place_civic(CivicKind::School, p) {
                err = Some(cost);
            }
        }
    }
    assert_eq!(err, Some(CivicKind::School.cost()));
    // after it opens it is built, costs upkeep and covers lots
    s.treasury = 500_000;
    for _ in 0..120 {
        s.step_day();
    }
    assert!(matches!(s.lots()[i].phase, Phase::Built { .. }));
    let cov = s.lot_values(MapKind::Coverage(Service::Health));
    assert!(cov.iter().any(|&v| v > 0.3));
    assert!(s.compute_ledger().spending[Dept::Health.index()] >= CivicKind::Clinic.upkeep());
    let ev = s.take_events();
    assert!(
        ev.iter()
            .any(|e| e.kind == EventKind::Civic && e.text.contains("opened"))
    );
}

#[test]
fn events_log_neighbourhoods_and_budget_trouble() {
    let t = town();
    let mut s = Sim::new(&t, hall_sites(&t)[0], 7);
    for _ in 0..600 {
        s.step_day();
    }
    let ev = s.take_events();
    assert!(
        ev.iter()
            .any(|e| e.kind == EventKind::Neighborhood && e.place.is_some())
    );
    assert!(ev.windows(2).all(|w| w[0].day <= w[1].day));
    assert!(s.take_events().is_empty());
    s.treasury = 5_000_000;
    for _ in 0..35 {
        s.step_day();
    }
    let _ = s.take_events();
    s.treasury = -50_000_000;
    for _ in 0..35 {
        s.step_day();
    }
    assert!(s.take_events().iter().any(|e| e.kind == EventKind::Budget));
}

#[test]
fn map_layers_have_one_value_per_lot_and_stay_in_range() {
    let t = town();
    let s = sim_after(&t, 500);
    let n = s.lots().len();
    for k in [
        MapKind::LandValue,
        MapKind::BuildingLevel,
        MapKind::Coverage(Service::Police),
        MapKind::Coverage(Service::Fire),
        MapKind::Demand(Class::Homes),
        MapKind::Demand(Class::Shops),
        MapKind::Demand(Class::Industry),
        MapKind::Traffic,
    ] {
        let v = s.lot_values(k);
        assert_eq!(v.len(), n);
        assert!(v.iter().all(|x| (0.0..=1.0).contains(x)), "{k:?}");
    }
    let u = s.lot_values(MapKind::Use);
    assert!(u.contains(&1.0) && u.contains(&2.0));
    assert!(u.iter().all(|&x| (0.0..=5.0).contains(&x)));
    assert_eq!(s.road_values(MapKind::Traffic).len(), s.road_runs().len());
    assert!(s.lot_values(MapKind::LandValue).iter().any(|&x| x > 0.3));
}

#[test]
fn levers_keep_the_simulation_deterministic() {
    let t = town();
    let run = || {
        let mut s = sim_after(&t, 300);
        s.set_tax_class(Class::Shops, 90);
        s.set_budget(Dept::Parks, 130);
        s.take_loan().unwrap();
        free_spot(&mut s, CivicKind::Fire).unwrap();
        for _ in 0..400 {
            s.step_day();
        }
        (
            s.fingerprint(),
            s.history().len(),
            s.history().last().unwrap().treasury,
        )
    };
    assert_eq!(run(), run());
}
