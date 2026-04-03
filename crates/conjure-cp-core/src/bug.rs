use std::fmt::Display;

/// Triggers a panic with a detailed bug report message, while ensuring the panic is ignored in coverage reports.
///
/// This macro is useful in situations where an unreachable code path is hit or when a bug occurs.
///
/// # Parameters
///
/// - `msg`: A string expression describing the cause of the panic or bug.
///
/// ```
#[macro_export]
macro_rules! bug {
    ($msg:expr)=> {
        $crate::bug!($msg,)
    };

    ($msg:expr, $($arg:tt)*) => {{
        let formatted_msg = format!($msg, $($arg)*);
        let full_message = format!(
            r#"
This should never happen, sorry!

However, it did happen, so it must be a bug. Please report it to us!

Conjure Oxide is actively developed and maintained. We will get back to you as soon as possible.

You can help us by providing a minimal failing example.

Issue tracker: http://github.com/conjure-cp/conjure-oxide/issues

version: {}
location: {}:{}:{}

{}
"#, git_version::git_version!(),file!(),module_path!(),line!(), &formatted_msg);

        panic!("{}", full_message);
    }};
}

/// Like [assert!], but formats the error message using [bug!]
#[macro_export]
macro_rules! bug_assert {
    ($cond:expr, $msg:expr) => {
        if !$cond {
            $crate::bug!("assertion failed: {}\n{}", stringify!($cond), $msg);
        }
    };

    ($cond:expr) => {
        if !$cond {
            $crate::bug!("assertion failed: {}", stringify!($cond));
        }
    };
}

/// Like [assert_eq!], but formats the error message using [bug!]
#[macro_export]
macro_rules! bug_assert_eq {
    ($left:expr, $right:expr, $msg:expr, $($arg:tt)*) => {
        if &$left != &$right {
            let formatted_msg = format!($msg, $($arg)*);
            $crate::bug!(
                "assertion failed: {} != {}\n{}",
                stringify!($left),
                stringify!($right),
                formatted_msg
            );
        }
    };

    ($left:expr, $right:expr, $msg:expr) => {
        $crate::bug_assert_eq!($left, $right, $msg,);
    };

    ($left:expr, $right:expr) => {
        if &$left != &$right {
            $crate::bug!(
                "assertion failed: {} != {}",
                stringify!($left),
                stringify!($right)
            );
        }
    };
}

pub trait UnwrapOrBug {
    type Output;
    fn unwrap_or_bug(self) -> Self::Output;
}

impl<T, E: Display> UnwrapOrBug for Result<T, E> {
    type Output = T;
    fn unwrap_or_bug(self) -> Self::Output {
        self.unwrap_or_else(|e| bug!("error: {}", e))
    }
}

impl<T> UnwrapOrBug for Option<T> {
    type Output = T;
    fn unwrap_or_bug(self) -> Self::Output {
        self.unwrap_or_else(|| bug!("expected a value, but got None"))
    }
}
