//! Money: tax rates per class, department budgets, bonds, the monthly ledger, history and the
//! event log.

use crate::*;

/// What one month costs and brings in at today's state.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Ledger {
    pub income: ByClass<i64>,
    pub spending: [i64; 6],
    pub admin: i64,
    pub interest: i64,
    pub principal: i64,
}

impl Ledger {
    pub fn income_total(&self) -> i64 {
        self.income.homes + self.income.shops + self.income.industry
    }

    pub fn expense_total(&self) -> i64 {
        self.spending.iter().sum::<i64>() + self.admin + self.interest + self.principal
    }
}

/// How much more a building of level 1 to 5 is worth, in percent.
pub(crate) fn level_value_pct(level: u8) -> i64 {
    match level {
        0 | 1 => 100,
        2 => 160,
        3 => 240,
        4 => 340,
        _ => 460,
    }
}

/// General upkeep per taxed building per month in a small town.
const ADMIN_BASE: i64 = 540;
/// Every this many taxed buildings add one percent to that.
const ADMIN_SIZE_STEP: i64 = 2000;
/// Utilities and wear, dollars per month per $10,000 of building value.
const UTILITY_UPKEEP_PER_10K: i64 = 14;
/// Staffing of a department costs this percent plus the slope times coverage (0 to 1).
const COVER_COST_BASE_PCT: i64 = 60;
const COVER_COST_SLOPE_PCT: i64 = 50;

const MILESTONES: [i64; 9] = [100, 250, 500, 1000, 2000, 5000, 10_000, 20_000, 50_000];

fn kind_jobs(k: LotKind) -> i64 {
    match k {
        LotKind::Shop => 6,
        LotKind::Gas => 3,
        LotKind::Strip => 25,
        LotKind::BigBox => 60,
        LotKind::FastFood => 8,
        LotKind::Office => 40,
        LotKind::Workshop => 20,
        LotKind::Warehouse => 15,
        _ => 0,
    }
}

impl Sim {
    // ----- taxes -----

    /// The three rates averaged, in tenths of a percent (for a single "all" slider).
    pub fn tax_x10(&self) -> u32 {
        (self.tax.homes + self.tax.shops + self.tax.industry + 1) / 3
    }

    pub fn tax_pct(&self) -> f64 {
        self.tax_x10() as f64 / 10.0
    }

    /// Set all three rates at once.
    pub fn set_tax_x10(&mut self, v: u32) {
        self.tax = ByClass::all(v.clamp(TAX_MIN_X10, TAX_MAX_X10));
    }

    pub fn tax_rates(&self) -> ByClass<u32> {
        self.tax
    }

    pub fn set_tax_class(&mut self, c: Class, v: u32) {
        self.tax.set(c, v.clamp(TAX_MIN_X10, TAX_MAX_X10));
    }

    /// The rate that applies to a lot (0 for lots that pay no tax).
    pub fn lot_rate_x10(&self, lot: usize) -> u32 {
        class_of(self.lots[lot].kind).map_or(0, |c| self.tax.get(c))
    }

    // ----- budgets -----

    pub fn budget_pct(&self, d: Dept) -> u32 {
        self.budget[d.index()]
    }

    pub fn set_budget(&mut self, d: Dept, pct: u32) {
        self.budget[d.index()] = pct.clamp(BUDGET_MIN_PCT, BUDGET_MAX_PCT);
    }

    // ----- bonds -----

    pub fn loans(&self) -> &[Loan] {
        &self.loans
    }

    pub fn debt(&self) -> i64 {
        self.loans.iter().map(|l| l.principal_left).sum()
    }

    /// Total value of taxed buildings standing now.
    pub fn tax_base(&self) -> i64 {
        self.lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Built { .. }) && !l.exempt)
            .map(|l| kind_value(l.kind) * level_value_pct(l.level) / 100)
            .sum()
    }

    /// The most the town may owe: a fifth of its tax base, never under $150,000.
    pub fn credit_limit(&self) -> i64 {
        (self.tax_base() / 5).max(MIN_CREDIT_LIMIT)
    }

    /// Yearly bond interest in tenths of a percent. It climbs as the town borrows against more of
    /// its credit limit.
    pub fn bond_rate_x10(&self) -> i64 {
        BOND_RATE_X10
            + BOND_RATE_RISK_X10 * self.debt().min(self.credit_limit()) / self.credit_limit()
    }

    /// Buildings that pay tax now.
    pub(crate) fn taxed_building_count(&self) -> usize {
        self.lots
            .iter()
            .filter(|l| {
                matches!(l.phase, Phase::Built { .. }) && !l.exempt && class_of(l.kind).is_some()
            })
            .count()
    }

    /// Take one bond of $100,000: cash now, interest and equal principal each month for 48
    /// months.
    pub fn take_loan(&mut self) -> Result<(), LoanError> {
        if self.debt() + BOND_SIZE > self.credit_limit() {
            return Err(LoanError::LimitReached);
        }
        self.loans.push(Loan {
            principal_left: BOND_SIZE,
            monthly_principal: BOND_SIZE / BOND_MONTHS,
        });
        self.treasury += BOND_SIZE;
        self.log(
            EventKind::Loan,
            format!("The town sold a ${BOND_SIZE} bond."),
            None,
        );
        Ok(())
    }

    /// Pay off the oldest bond early. Returns the amount paid.
    pub fn repay_loan(&mut self) -> Result<i64, LoanError> {
        let Some(l) = self.loans.first().copied() else {
            return Err(LoanError::NothingToRepay);
        };
        if self.treasury < l.principal_left {
            return Err(LoanError::NotEnoughCash);
        }
        self.treasury -= l.principal_left;
        self.loans.remove(0);
        self.log(
            EventKind::Loan,
            format!("A bond was paid off early (${}).", l.principal_left),
            None,
        );
        Ok(l.principal_left)
    }

    // ----- ledger -----

    pub(crate) fn compute_ledger(&self) -> econ::Ledger {
        let mut lg = Ledger::default();
        let f = |d: Dept| self.budget[d.index()] as i64;
        // a bigger town costs more to run per building: more offices, more records, more
        // everything
        let taxed = self.taxed_building_count() as i64;
        let admin_each = ADMIN_BASE * (100 + taxed / ADMIN_SIZE_STEP) / 100;
        for (i, l) in self.lots.iter().enumerate() {
            if !matches!(l.phase, Phase::Built { .. }) {
                continue;
            }
            if let Some(c) = class_of(l.kind) {
                let value = kind_value(l.kind) * level_value_pct(l.level) / 100;
                let add = value * self.tax.get(c) as i64 / 1000 / 12;
                match c {
                    Class::Homes => lg.income.homes += add,
                    Class::Shops => lg.income.shops += add,
                    Class::Industry => lg.income.industry += add,
                }
                lg.admin += admin_each + value * UTILITY_UPKEEP_PER_10K / 10_000;
                for d in Dept::ALL {
                    // the better a building is covered, the more the service costs there
                    let cov = d
                        .service()
                        .and_then(|s| self.coverage[s.index()].get(i).copied())
                        .unwrap_or(0.0);
                    let factor = COVER_COST_BASE_PCT + (COVER_COST_SLOPE_PCT as f32 * cov) as i64;
                    lg.spending[d.index()] += d.staff_cost() * f(d) / 100 * factor / 100;
                }
                continue;
            }
            let (service, upkeep) = match l.civic {
                Some(c) => (Some(c.service()), c.upkeep()),
                None => (service_of_kind(l.kind), kind_upkeep(l.kind)),
            };
            match service.and_then(Service::dept) {
                Some(d) => lg.spending[d.index()] += upkeep * f(d) / 100,
                None => lg.admin += upkeep,
            }
        }
        let mut roads = 0;
        for (i, &r) in self.revealed.iter().enumerate() {
            if r {
                roads += self.road_cost_per_month[i];
            }
        }
        lg.spending[Dept::Roads.index()] = roads * f(Dept::Roads) / 100;
        let rate_x10 = self.bond_rate_x10();
        for l in &self.loans {
            lg.interest += l.principal_left * rate_x10 / 1000 / 12;
            lg.principal += l.monthly_principal.min(l.principal_left);
        }
        lg
    }

    /// Dollars per month at today's state: (tax income, expenses including bond payments).
    pub fn monthly_flow(&self) -> (i64, i64) {
        let lg = self.compute_ledger();
        (lg.income_total(), lg.expense_total())
    }

    pub fn monthly_net(&self) -> i64 {
        let (i, e) = self.monthly_flow();
        i - e
    }

    /// Jobs in buildings standing now.
    pub fn jobs(&self) -> i64 {
        self.lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Built { .. }) && !l.exempt)
            .map(|l| kind_jobs(l.kind) * level_value_pct(l.level).min(300) / 100)
            .sum()
    }

    pub(crate) fn jobs_of(&self, class: Class) -> i64 {
        self.lots
            .iter()
            .filter(|l| matches!(l.phase, Phase::Built { .. }) && !l.exempt)
            .filter(|l| class_of(l.kind) == Some(class))
            .map(|l| kind_jobs(l.kind) * level_value_pct(l.level).min(300) / 100)
            .sum()
    }

    /// Open the books for the month that just ended.
    pub(crate) fn run_month(&mut self) {
        self.step_rival();
        self.recompute_values();
        self.evolve_buildings();
        let lg = self.compute_ledger();
        let (income, expense) = (lg.income_total(), lg.expense_total());
        self.treasury += income - expense;
        self.last_income = income;
        self.last_expense = expense;
        for l in self.loans.iter_mut() {
            l.principal_left -= l.monthly_principal.min(l.principal_left);
        }
        self.loans.retain(|l| l.principal_left > 0);
        self.last_ledger = lg;
        let d = self.date();
        let sample = Sample {
            day: self.day,
            year: d.year,
            month: d.month,
            population: self.population(),
            jobs: self.jobs(),
            treasury: self.treasury,
            income: lg.income,
            spending: lg.spending,
            admin: lg.admin,
            debt_service: lg.interest + lg.principal,
            debt: self.debt(),
            tax_x10: self.tax,
            demand: self.class_demand,
            built: self.built_count() as u32,
            building: self.building_count() as u32,
            empty: self.empty_count() as u32,
            avg_land_value: self.avg_land_value(),
            congestion: self.avg_congestion(),
            approval: self.compute_approval().total,
            days_to_election: if self.rules.elections {
                (self.next_election - self.day).max(0)
            } else {
                0
            },
        };
        self.history.push(sample);
        // budget warnings, once on entering each state
        let net = income - expense;
        let state = if self.treasury < 0 {
            2
        } else if net < 0 && self.treasury < -net * 6 {
            1
        } else {
            0
        };
        if state > self.warn_level {
            let text = if state == 2 {
                "Budget warning: the treasury is overdrawn."
            } else {
                "Budget warning: at this rate the treasury runs out within six months."
            };
            self.log(EventKind::Budget, text.to_string(), None);
        }
        self.warn_level = state;
        // population milestones
        let pop = self.population();
        while self.next_milestone < MILESTONES.len() && pop >= MILESTONES[self.next_milestone] {
            let m = MILESTONES[self.next_milestone];
            self.next_milestone += 1;
            self.log(
                EventKind::Milestone,
                format!("The town reached {m} residents."),
                None,
            );
        }
        self.pressure_month();
    }

    // ----- history and events -----

    /// One sample per game month since the game began, oldest first.
    pub fn history(&self) -> &[Sample] {
        &self.history
    }

    pub(crate) fn log(&mut self, kind: EventKind, text: String, place: Option<Pt>) {
        self.events.push(Event {
            day: self.day,
            kind,
            text,
            place,
        });
    }

    /// Events since the last call, oldest first.
    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub(crate) fn addition_centre(&self, a: usize) -> Pt {
        let ring = &self.addition_poly[a];
        let n = ring.len().max(1) as f64;
        pt(
            ring.iter().map(|p| p.x).sum::<f64>() / n,
            ring.iter().map(|p| p.y).sum::<f64>() / n,
        )
    }
}
