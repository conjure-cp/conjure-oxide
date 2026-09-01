//! Solver adaptors.

use std::time::{Duration, Instant};

/// One wall-clock budget shared by every backend call made during a single solve.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SolveTimeBudget {
    started: Instant,
    limit: Option<Duration>,
}

impl SolveTimeBudget {
    pub(crate) fn new(limit: Option<Duration>) -> Self {
        Self {
            started: Instant::now(),
            limit,
        }
    }

    pub(crate) fn remaining(self) -> Option<Duration> {
        self.limit
            .map(|limit| limit.saturating_sub(self.started.elapsed()))
    }

    pub(crate) fn expired(self) -> bool {
        self.remaining()
            .is_some_and(|remaining| remaining.is_zero())
    }
}

pub mod minion;
pub mod rustsat;

#[doc(inline)]
pub use minion::{Minion, MinionValueOrder, MinionVariableOrder};

#[doc(inline)]
pub use rustsat::Sat;

pub mod smt;

#[doc(inline)]
pub use smt::Smt;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_solve_time_budget_is_immediately_expired() {
        assert!(SolveTimeBudget::new(Some(Duration::ZERO)).expired());
    }

    #[test]
    fn absent_solve_time_budget_never_expires() {
        let budget = SolveTimeBudget::new(None);
        assert_eq!(budget.remaining(), None);
        assert!(!budget.expired());
    }
}
