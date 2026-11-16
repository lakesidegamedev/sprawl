//! A sensible mayor, for the tests and for the smoke runs that grow a town to look at it.

use super::*;

/// Put a building as near `at` as the rules allow.
fn place_near(s: &mut Sim, kind: CivicKind, at: Pt) -> bool {
    for r in (20..300).step_by(20) {
        for a in 0..16 {
            let ang = a as f64 * std::f64::consts::TAU / 16.0;
            let p = pt(
                at.x + r as f64 * libm::cos(ang),
                at.y + r as f64 * libm::sin(ang),
            );
            if s.place_civic(kind, p).is_ok() {
                return true;
            }
        }
    }
    false
}

/// A sensible mayor: keeps every service built as the town grows, near where people live, builds
/// water, power, schools and clinics before they run out, and moves the tax rate to keep the
/// books balanced.
pub fn tend(s: &mut Sim, round: &mut u32) {
    let (i, e) = s.monthly_flow();
    let t = s.tax_x10();
    tune_taxes(s, i, e, t);
    build_ahead(s, round);
}

fn tune_taxes(s: &mut Sim, i: i64, e: i64, t: u32) {
    if i > 0 && e > i && t < 100 {
        s.set_tax_x10(t + 5);
    } else if i > 0
        && ((i - e) * 100 > 8 * i || (s.treasury > 3_000_000 && (i - e) * 100 > i))
        && t > 60
    {
        s.set_tax_x10(t - 5);
    }
}

/// Everything but the tax rate: capacities and services built ahead of need.
pub fn build_ahead(s: &mut Sim, round: &mut u32) {
    let (i, e) = s.monthly_flow();
    *round += 1;
    // capacities: build ahead of need, one at a time
    for (cap, kind) in [
        (Capacity::Water, CivicKind::Water),
        (Capacity::Power, CivicKind::Power),
        (Capacity::Schools, CivicKind::School),
        (Capacity::Health, CivicKind::Clinic),
    ] {
        let building = s
            .lots()
            .iter()
            .any(|l| l.civic == Some(kind) && matches!(l.phase, Phase::Building { .. }));
        if building || s.capacity(cap).ratio() < 0.7 {
            continue;
        }
        // a mayor who cannot pay borrows for what the town cannot grow without
        while s.treasury < kind.cost() + 100_000 && s.take_loan().is_ok() {}
        if s.treasury < kind.cost() + 50_000 {
            continue;
        }
        let homes: Vec<usize> = (0..s.lots().len())
            .filter(|&i| {
                matches!(s.lots()[i].phase, Phase::Built { .. })
                    && !s.lots()[i].exempt
                    && class_of(s.lots()[i].kind) == Some(Class::Homes)
            })
            .collect();
        let at = if homes.is_empty() {
            s.hall
        } else {
            s.lots()[homes[(*round as usize * 7919) % homes.len()]].pos
        };
        place_near(s, kind, at);
    }
    let want = 1 + (s.population() / 4000) as usize;
    for kind in [CivicKind::Fire, CivicKind::Police, CivicKind::Park] {
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
        place_near(s, kind, at);
    }
}

