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
