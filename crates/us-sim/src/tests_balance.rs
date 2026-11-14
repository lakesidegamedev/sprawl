use super::*;
use us_gen::{Preset, generate};

/// One sample per game year: (population, treasury, monthly income, monthly expenses).
type Year = (i64, i64, i64, i64);

fn run_years(preset: Preset, tseed: u64, tax: u32, years: i32) -> Vec<Year> {
    let town = generate(preset, tseed);
    let mut s = Sim::new_with_rules(&town, hall_sites(&town)[0], 7, 2, Rules::sandbox());
    s.set_tax_x10(tax);
    let mut out = Vec::new();
    for d in 0..365 * years {
        s.step_day();
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
            (-0.10..=0.12).contains(&r),
            "year {}: net is {:.1}% of income",
            n + 1,
            r * 100.0
        );
        // the pile never reaches more than twenty months of income
        assert!(
            y.1.abs() < 20 * y.2,
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

/// A sensible mayor: keeps every service built as the town grows, near where people live, and
/// moves the tax rate to keep the books balanced.
pub(crate) fn tend(s: &mut Sim, round: &mut u32) {
    let (i, e) = s.monthly_flow();
    let t = s.tax_x10();
    if i > 0 && e > i && t < 100 {
        s.set_tax_x10(t + 5);
    } else if i > 0
        && ((i - e) * 100 > 8 * i || (s.treasury > 3_000_000 && (i - e) * 100 > i))
        && t > 60
    {
        s.set_tax_x10(t - 5);
    }
    let want = 1 + (s.population() / 4000) as usize;
    *round += 1;
    for kind in [
        CivicKind::Fire,
        CivicKind::Police,
        CivicKind::School,
        CivicKind::Clinic,
        CivicKind::Park,
    ] {
        let have = s.lots().iter().filter(|l| l.civic == Some(kind)).count();
        let want = if kind == CivicKind::Fire {
            want * 3 / 2
        } else {
            want
        };
        if have >= want || e > i || s.treasury < kind.cost() + 500_000 {
            continue;
        }
        let svc = kind.service();
        let poor: Vec<usize> = (0..s.lots().len())
            .filter(|&i| {
                let l = &s.lots()[i];
                matches!(l.phase, Phase::Built { .. })
                    && !l.exempt
                    && class_of(l.kind) == Some(Class::Homes)
                    && s.coverage[svc.index()].get(i).copied().unwrap_or(0.0) < 0.3
            })
            .collect();
        if poor.is_empty() {
            continue;
        }
        let at = s.lots()[poor[(*round as usize * 7919) % poor.len()]].pos;
        'search: for r in (20..300).step_by(20) {
            for a in 0..16 {
                let ang = a as f64 * std::f64::consts::TAU / 16.0;
                let p = pt(
                    at.x + r as f64 * libm::cos(ang),
                    at.y + r as f64 * libm::sin(ang),
                );
                if s.place_civic(kind, p).is_ok() {
                    break 'search;
                }
            }
        }
    }
}
