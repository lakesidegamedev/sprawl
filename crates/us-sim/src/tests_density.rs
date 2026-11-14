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
        assert!(s.built_count() > 250, "built {}", s.built_count());
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
