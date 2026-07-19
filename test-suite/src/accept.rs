use std::env;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptMode {
    /// Normal test mode: compare generated files with expected files and do not rewrite fixtures.
    Disabled,
    /// Rewrite expected output fixtures and runtime budgets exactly as observed.
    Accept,
    /// Rewrite output fixtures, but only raise runtime budgets (max of current and observed).
    ///
    /// Useful when recording budgets over several runs: each run keeps the slowest time seen so
    /// far, so a one-off fast fluke does not lower the recorded value.
    AcceptWithMaxTimes,
}

impl AcceptMode {
    pub fn from_env() -> Self {
        match env::var("ACCEPT").as_deref() {
            Ok("false") => Self::Disabled,
            Ok("true") => Self::Accept,
            Ok("with-max-times") => Self::AcceptWithMaxTimes,
            _ => Self::Disabled,
        }
    }

    pub fn accepts_outputs(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub fn records_expected_time(self) -> bool {
        matches!(self, Self::Accept | Self::AcceptWithMaxTimes)
    }

    pub fn expected_time_to_record(self, current: Option<u64>, observed: u64) -> Option<u64> {
        match self {
            Self::Accept => Some(observed),
            Self::AcceptWithMaxTimes if current.is_none_or(|current| observed > current) => {
                Some(observed)
            }
            _ => None,
        }
    }

    pub fn refresh_hint() -> &'static str {
        "Run with ACCEPT=true or ACCEPT=with-max-times"
    }
}
