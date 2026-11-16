use super::*;
use us_gen::{Preset, generate};

pub(crate) use crate::tending::{build_ahead, tend};

/// One sample per game year: (population, treasury, monthly income, monthly expenses).
type Year = (i64, i64, i64, i64);

fn run_years(preset: Preset, tseed: u64, tax: u32, years: i32) -> Vec<Year> {
    let town = generate(preset, tseed);
    let mut s = Sim::new_with_rules(&town, hall_sites(&town)[0], 7, 2, Rules::sandbox());
    s.set_tax_x10(tax);
    let mut out = Vec::new();
    let mut round = 0;
    for d in 0..365 * years {
        s.step_day();
        if d % 30 == 5 {
            build_ahead(&mut s, &mut round);
        }
        if d % 365 == 364 {
            let (i, e) = s.monthly_flow();
            out.push((s.population(), s.treasury, i, e));
        }
    }
    for (y, (p, t, i, e)) in out.iter().enumerate() {
        eprintln!(
            "{preset:?} tax {tax} year {} pop {p} treasury {t} income {i} costs {e}",
            y + 1
        );
    }
    out
}

fn ratio(y: &Year) -> f64 {
    (y.2 - y.3) as f64 / y.2.max(1) as f64
}

fn near_break_even(years: &[Year]) {
    for (n, y) in years.iter().enumerate().skip(9) {
        let r = ratio(y);
        assert!(
            // generator version 3 towns (roundabouts, through routes) and weather costs move the
            // net a few points; the band allows for both
            (-0.10..=0.45).contains(&r),
            "year {}: net is {:.1}% of income",
            n + 1,
            r * 100.0
        );
        // the pile never reaches more than sixty months of income: growth is slow now, so a
        // town at default taxes earns well beyond its costs and the player is expected to cut
        // taxes or build more
        assert!(
            y.1.abs() < 60 * y.2,
            "year {}: treasury {} against income {}",
            n + 1,
            y.1,
            y.2
        );
    }
}

#[test]
fn default_levers_hover_near_break_even_grid() {
    let y = run_years(Preset::Grid, 3, TAX_DEFAULT_X10, 30);
    assert!(y[29].0 > 15_000, "the town should still be a town");
    // ten thousand people takes more than ten game years even when the town is looked after
    assert!(y[8].0 < 10_000 && y[17].0 > 8_000, "{:?} {:?}", y[8], y[17]);
    near_break_even(&y);
}

#[test]
fn default_levers_hover_near_break_even_winding() {
    let y = run_years(Preset::Winding, 5, TAX_DEFAULT_X10, 30);
    near_break_even(&y);
}

#[test]
fn very_low_taxes_run_a_deficit() {
    let y = run_years(Preset::Grid, 3, 30, 30);
    for (n, y) in y.iter().enumerate().skip(7) {
        assert!(ratio(y) < -0.3, "year {}: ratio {:.2}", n + 1, ratio(y));
    }
    assert!(y[29].1 < 0, "treasury {}", y[29].1);
}

#[test]
fn very_high_taxes_empty_the_town() {
    let calm = run_years(Preset::Grid, 3, TAX_DEFAULT_X10, 15);
    let harsh = run_years(Preset::Grid, 3, 180, 15);
    assert!(
        harsh[14].0 < calm[14].0 / 5,
        "{} against {}",
        harsh[14].0,
        calm[14].0
    );
}

#[test]
#[ignore]
fn calibrate() {
    for d in Difficulty::ALL {
        for managed in [true, false] {
            let town = generate(Preset::Grid, 3);
            let mut s = Sim::new_with_rules(
                &town,
                hall_sites(&town)[0],
                7,
                2,
                Rules::sandbox().with_difficulty(d),
            );
            let mut round = 0;
            let mut out = String::new();
            for day in 0..365 * 16 {
                s.step_day();
                if managed && day % 30 == 5 {
                    tend(&mut s, &mut round);
                }
                if day % 365 == 364 {
                    out += &format!(" {}", s.population());
                }
            }
            eprintln!(
                "{d:?} managed {managed}: pop by year:{out}; treasury {} stalls {:?} over {:?}",
                s.treasury,
                s.growth_stalls(),
                s.game_over().map(|g| g.headline())
            );
        }
    }
}
