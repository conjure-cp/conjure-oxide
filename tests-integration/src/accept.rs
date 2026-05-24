use std::env;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptMode {
    Disabled,
    Accept,
    AcceptWithTimes,
}

impl AcceptMode {
    pub fn from_env() -> Self {
        match env::var("ACCEPT").as_deref() {
            Ok("false") => Self::Disabled,
            Ok("true") => Self::Accept,
            Ok("with-times") => Self::AcceptWithTimes,
            _ => Self::Disabled,
        }
    }

    pub fn accepts_outputs(self) -> bool {
        !matches!(self, Self::Disabled)
    }

    pub fn records_expected_time(self) -> bool {
        matches!(self, Self::AcceptWithTimes)
    }

    pub fn refresh_hint() -> &'static str {
        "Run with ACCEPT=true or ACCEPT=with-times"
    }
}
