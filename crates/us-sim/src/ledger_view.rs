//! A read-only copy of what one month costs and brings in at today's state, for the budget screens.

use crate::*;

/// One month at today's rates, budgets and buildings.
#[derive(Clone, Copy, Debug, Default)]
pub struct LedgerNow {
    /// Tax per class.
    pub income: ByClass<i64>,
    /// Spending per department, indexed by `Dept::index()`.
    pub spending: [i64; 6],
    /// General upkeep not tied to a department.
    pub admin: i64,
    /// Interest on bonds.
    pub interest: i64,
    /// Principal repaid on bonds.
    pub principal: i64,
}

impl Sim {
    /// What the next month would cost and bring in if nothing changed.
    pub fn ledger_now(&self) -> LedgerNow {
        let lg = self.compute_ledger();
        LedgerNow {
            income: lg.income,
            spending: lg.spending,
            admin: lg.admin,
            interest: lg.interest,
            principal: lg.principal,
        }
    }
}
