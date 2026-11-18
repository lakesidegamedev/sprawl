use super::*;
use us_gen::{Preset, generate};

fn town() -> Town {
    generate(Preset::Grid, 3)
}

fn new_sim(t: &Town) -> Sim {
    Sim::new_with_rules(t, hall_sites(t)[0], 7, 2, Rules::sandbox())
}

fn run(s: &mut Sim, days: i32) {
    for _ in 0..days {
        s.step_day();
    }
}

/// One district holding every neighbourhood of the town.
fn whole_town(s: &mut Sim) -> usize {
    let d = s.create_district().unwrap();
    for a in s.order.clone() {
        s.toggle_addition(d, a);
    }
    d
}

fn max_level_built(s: &Sim) -> u8 {
    s.lots()
        .iter()
        .filter(|l| matches!(l.phase, Phase::Built { .. }) && !l.exempt)
        .map(|l| l.level)
        .max()
        .unwrap_or(0)
}

#[test]
fn districts_are_created_named_painted_and_deleted() {
    let t = town();
    let mut s = new_sim(&t);
    let a = s.create_district().unwrap();
    let b = s.create_district().unwrap();
    assert_ne!(s.districts()[a].name, s.districts()[b].name);
    assert_ne!(s.districts()[a].colour, s.districts()[b].colour);
    let add = s.order[0];
    assert!(s.toggle_addition(a, add));
    assert_eq!(s.district_of_addition(add), Some(a));
    // painting it into another district moves it
    assert!(s.toggle_addition(b, add));
    assert_eq!(s.district_of_addition(add), Some(b));
    assert!(s.districts()[a].additions.is_empty());
    // painting it again takes it out
    assert!(!s.toggle_addition(b, add));
    assert_eq!(s.district_of_addition(add), None);
    s.rename_district(a, "  Old Town  ");
    assert_eq!(s.districts()[a].name, "Old Town");
    s.toggle_addition(b, add);
    s.delete_district(a);
    assert_eq!(s.districts().len(), 1);
    assert_eq!(s.district_of_addition(add), Some(0));
    while s.create_district().is_some() {}
    assert_eq!(s.districts().len(), MAX_DISTRICTS);
    let mut colours: Vec<u8> = s.districts().iter().map(|d| d.colour).collect();
    colours.sort();
    colours.dedup();
    assert_eq!(colours.len(), MAX_DISTRICTS);
}

#[test]
fn scope_is_enforced() {
    let t = town();
    let mut s = new_sim(&t);
    let d = whole_town(&mut s);
    s.set_ordinance_town(Ordinance::HeightLimit, true);
    assert!(!s.ordinance_town(Ordinance::HeightLimit));
    s.set_ordinance_district(d, Ordinance::Tourism, true);
    assert!(!s.ordinance_in_district(d, Ordinance::Tourism));
    s.set_ordinance_town(Ordinance::Tourism, true);
    s.set_ordinance_district(d, Ordinance::HeightLimit, true);
    assert!(s.ordinance_town(Ordinance::Tourism));
    assert!(s.ordinance_in_district(d, Ordinance::HeightLimit));
}

/// A tended game: the mayor keeps building services, as in the balance tests.
fn tended(t: &Town, ordinance: Option<Ordinance>, days: i32) -> Sim {
    let mut s = new_sim(t);
    s.set_tax_x10(70);
    if let Some(o) = ordinance {
        let d = whole_town(&mut s);
        s.set_ordinance_district(d, o, true);
    }
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
fn height_limit_and_heritage_stop_growth_upward() {
    let t = town();
    let free = tended(&t, None, 365 * 20);
    assert!(max_level_built(&free) >= 3, "the plain town grows tall");
    let low = tended(&t, Some(Ordinance::HeightLimit), 365 * 20);
    assert!(max_level_built(&low) <= 2);
    let old = tended(&t, Some(Ordinance::Heritage), 365 * 20);
    assert_eq!(max_level_built(&old), 1);
}

#[test]
fn upzoning_lets_central_lots_go_higher() {
    let t = town();
    let s = new_sim(&t);
    let i = (0..s.lots().len())
        .filter(|&i| class_of(s.lots()[i].kind) == Some(Class::Homes))
        .max_by(|&a, &b| s.lot_density(a).partial_cmp(&s.lot_density(b)).unwrap());
    let mut s = s;
    let Some(i) = i else { return };
    let d = whole_town(&mut s);
    let k = s.lots()[i].kind;
    let before = s.lot_cap(i, s.population());
    s.set_ordinance_district(d, Ordinance::Upzoning, true);
    let after = s.lot_cap(i, s.population());
    if s.lot_density(i) >= 0.35 && before < growth::max_level(k) {
        assert_eq!(after, before + 1);
    } else {
        assert_eq!(after, before);
    }
}

#[test]
fn costs_are_charged_monthly_and_shown() {
    let t = town();
    let mut s = new_sim(&t);
    run(&mut s, 900);
    let base_flow = s.monthly_flow();
    assert_eq!(s.ordinance_costs(), 0);
    s.set_ordinance_town(Ordinance::Recycling, true);
    s.set_ordinance_town(Ordinance::Tourism, true);
    let cost = s.ordinance_costs();
    assert!(cost > 0);
    assert_eq!(s.ledger_now().ordinances, cost);
    let flow = s.monthly_flow();
    assert_eq!(flow.1, base_flow.1 + cost);
    s.recompute_values();
    let t0 = s.treasury;
    let before_cost = s.ordinance_costs();
    // run to the next first of the month and check the treasury paid the fees
    loop {
        s.step_day();
        if s.date().day == 1 {
            break;
        }
    }
    assert!(s.dist.last_cost > 0);
    assert!(s.last_expense >= s.dist.last_cost);
    assert!(before_cost > 0 && t0 != s.treasury);
}

#[test]
fn business_break_lowers_tax_and_raises_demand() {
    let t = town();
    let mut s = new_sim(&t);
    run(&mut s, 1500);
    let tax0 = s.compute_ledger().income;
    let dem0 = (
        s.class_demand(Class::Shops),
        s.class_demand(Class::Industry),
    );
    s.set_ordinance_town(Ordinance::BusinessBreak, true);
    s.recompute_values();
    let tax1 = s.compute_ledger().income;
    assert!(tax1.shops < tax0.shops);
    assert!(tax1.industry <= tax0.industry);
    assert_eq!(tax1.homes, tax0.homes);
    assert!(s.class_demand(Class::Shops) >= dem0.0);
    assert!(s.class_demand(Class::Industry) >= dem0.1);
    assert!(s.ordinance_estimate(Ordinance::BusinessBreak, None) < 0);
}

#[test]
fn watch_raises_police_cover_and_alarms_halve_fire_risk() {
    let t = town();
    let mut s = new_sim(&t);
    run(&mut s, 1200);
    let sum = |s: &Sim| s.coverage[Service::Police.index()].iter().sum::<f32>();
    let before = sum(&s);
    s.set_ordinance_town(Ordinance::Watch, true);
    s.recompute_values();
    assert!(sum(&s) > before);
    s.set_ordinance_town(Ordinance::SmokeAlarms, true);
    let i = s
        .lots()
        .iter()
        .position(|l| class_of(l.kind).is_some() && matches!(l.phase, Phase::Built { .. }))
        .unwrap();
    assert_eq!(s.ord_fire_factor(i), 0.5);
}

#[test]
fn traffic_calming_and_clean_industry_change_land_value_terms() {
    let t = town();
    let mut s = new_sim(&t);
    run(&mut s, 1500);
    let d = whole_town(&mut s);
    let i = s
        .lots()
        .iter()
        .position(|l| {
            class_of(l.kind) == Some(Class::Homes) && matches!(l.phase, Phase::Built { .. })
        })
        .unwrap();
    s.set_ordinance_district(d, Ordinance::Heritage, true);
    assert!(s.ord_value_bonus(i) > 0.0);
    s.set_ordinance_district(d, Ordinance::CleanIndustry, true);
    assert_eq!(s.ord_industry_factor(i), 0.5);
    s.set_ordinance_district(d, Ordinance::TrafficCalming, true);
    assert!(s.lot_traffic_eff(i) <= s.lot_traffic(i));
    // approval: calming adds to the traffic part, heritage to services
    let pts = s.ordinance_approval();
    assert!(
        pts.iter()
            .any(|&(o, p)| o == Ordinance::TrafficCalming && p > 0)
    );
}

#[test]
fn why_lists_district_and_ordinances() {
    let t = town();
    let mut s = new_sim(&t);
    run(&mut s, 1200);
    let d = whole_town(&mut s);
    s.set_ordinance_district(d, Ordinance::Heritage, true);
    let i = s
        .lots()
        .iter()
        .position(|l| class_of(l.kind).is_some() && matches!(l.phase, Phase::Built { .. }))
        .unwrap();
    let w = s.why(i);
    assert!(w.ordinances.iter().any(|l| l.contains("district")));
    assert!(w.ordinances.iter().any(|l| l.starts_with("Heritage")));
    let st = s.district_stats(d).unwrap();
    assert!(st.people > 0 && st.buildings > 0 && !st.problem.is_empty());
    assert!(st.ordinance_cost > 0);
}

#[test]
fn ordinances_save_and_load_mid_game() {
    let t = town();
    let mut s = new_sim(&t);
    run(&mut s, 800);
    let d = whole_town(&mut s);
    let e = s.create_district().unwrap();
    s.rename_district(e, "Empty One");
    s.set_ordinance_district(d, Ordinance::Heritage, true);
    s.set_ordinance_district(d, Ordinance::TrafficCalming, true);
    s.set_ordinance_town(Ordinance::Recycling, true);
    s.set_ordinance_town(Ordinance::Tourism, true);
    run(&mut s, 200);
    let bytes = s.save();
    let mut loaded = Sim::load(&bytes).expect("loads");
    assert_eq!(loaded.save(), bytes);
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    assert_eq!(loaded.districts(), s.districts());
    run(&mut s, 400);
    run(&mut loaded, 400);
    assert_eq!(loaded.fingerprint(), s.fingerprint());
    assert_eq!(loaded.save(), s.save());
}

#[test]
fn a_game_without_districts_has_no_section() {
    let t = town();
    let mut s = new_sim(&t);
    run(&mut s, 100);
    assert!(
        save_ext::sections(&s).iter().all(|(tag, _)| tag != b"DIST"),
        "unused feature writes nothing"
    );
    s.create_district();
    assert!(save_ext::sections(&s).iter().any(|(tag, _)| tag == b"DIST"));
}

#[test]
fn challenger_can_promise_a_missing_ordinance() {
    let t = town();
    let mut s = new_sim(&t);
    assert!(
        s.challenger_line(Issue::Traffic)
            .contains("traffic calming")
    );
    s.create_district();
    s.set_ordinance_town(Ordinance::Recycling, true);
    assert!(s.challenger_line(Issue::Taxes).is_empty() || s.dist.last_cost > 0);
}
