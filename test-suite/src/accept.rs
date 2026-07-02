use std::env;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptMode {
    /// Normal test mode: compare generated files with expected files and do not rewrite fixtures.
    Disabled,
    /// Rewrite expected output fixtures, but leave expected runtime budgets untouched.
    Accept,
    /// Rewrite output fixtures and runtime budgets exactly as observed.
    AcceptWithTimes,
    /// Rewrite output fixtures, but only raise runtime budgets.
    ///
    /// Catching slowdowns is more important than automatically accepting speedups. Runtimes
    /// are non-deterministic and machine/load dependent, so a significant slowdown may be
    /// worth noticing while a one-off faster run should not lower the recorded budget.
    AcceptWithSlowerTimes,
}

impl AcceptMode {
    pub fn from_env() -> Self {
        match env::var("ACCEPT").as_deref() {
            Ok("false") => Self::Disabled,
            Ok("true") => Self::Accept,
            Ok("with-times") => Self::AcceptWithTimes,
            Ok("with-exact-times") => Self::AcceptWithTimes,
            Ok("with-slower-times") => Self::AcceptWithSlowerTimes,
            _ => Self::Disabled,
        }
    }

    pub fn accepts_outputs(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub fn records_expected_time(self) -> bool {
        matches!(self, Self::AcceptWithTimes | Self::AcceptWithSlowerTimes)
    }

    pub fn expected_time_to_record(self, current: Option<u64>, observed: u64) -> Option<u64> {
        match self {
            Self::AcceptWithTimes => Some(observed),
            Self::AcceptWithSlowerTimes if current.is_none_or(|current| observed > current) => {
                Some(observed)
            }
            _ => None,
        }
    }

    pub fn refresh_hint() -> &'static str {
        "Run with ACCEPT=true, ACCEPT=with-slower-times, or ACCEPT=with-exact-times"
    }
}
