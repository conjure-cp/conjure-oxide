/// Emulates let chaining; Syntax:
///
/// ```ignore
/// guard!(
///     let My(a, Pattern { b, .. }) = x && // let bindings
///     let Foo(c) = a.bar(b)            && // using a previously bound variable
///     c >= 2 && c <= 10                   // boolean conditions
///     else {
///         if_any_of_the_above_fails();
///     }
/// );
/// if_all_succeed(c);
/// ```
///
/// # Example
///
/// ```
/// use conjure_cp_rules::guard;
///
/// struct S {
///     x: Option<i32>,
///     y: bool
/// }
///
/// enum E {
///     A(&'static str, S),
///     B(&'static str, Box<E>),
///     C
/// }
///
/// fn maybe_get_the_answer(val: E) -> Option<i32> {
///     guard!(
///         let E::A(_, S { x, ..}) = val &&
///         let Some(x) = x &&
///         x == 42
///         else {
///             return None;
///         }
///     );
///     Some(x)
/// }
///
/// assert_eq!(maybe_get_the_answer(E::A("the answer", S { x: Some(42), y: true })), Some(42));
/// assert_eq!(maybe_get_the_answer(E::A("not the answer", S { x: Some(69), y: true })), None);
/// assert_eq!(maybe_get_the_answer(E::B("also not the answer", Box::new(E::C))), None);
/// ```
#[macro_export]
macro_rules! guard {
    // ---- Accumulating Let Expressions ----
    // If our current token buffer is empty and we see a `let`, start a let-binding
    (@parse [$($accum:tt)*] [] -> let $p:pat = $($tail:tt)*) => {
        $crate::guard!(@parse_let [$($accum)*] let $p = [] -> $($tail)*);
    };
    // Hit `&&`: save the let binding and start looking for the next condition
    (@parse_let [$($accum:tt)*] let $p:pat = [$($expr:tt)+] -> && $($tail:tt)*) => {
        $crate::guard!(@parse [$($accum)* @let($p, $($expr)+)] [] -> $($tail)*);
    };
    // Hit `else`: save the let binding and begin code generation
    (@parse_let [$($accum:tt)*] let $p:pat = [$($expr:tt)+] -> else $fallback:block) => {
        $crate::guard!(@generate [else $fallback] $($accum)* @let($p, $($expr)+));
    };
    // Otherwise: accumulate tokens into the expression
    (@parse_let [$($accum:tt)*] let $p:pat = [$($expr:tt)*] -> $tok:tt $($tail:tt)*) => {
        $crate::guard!(@parse_let [$($accum)*] let $p = [$($expr)* $tok] -> $($tail)*);
    };

    // ---- Accumulating Boolean Conditions ----
    // Hit `&&`: save the boolean condition and start looking for the next condition
    (@parse [$($accum:tt)*] [$($cond:tt)+] -> && $($tail:tt)*) => {
        $crate::guard!(@parse [$($accum)* @cond($($cond)+)] [] -> $($tail)*);
    };
    // Hit `else`: save the boolean condition and begin code generation
    (@parse [$($accum:tt)*] [$($cond:tt)+] -> else $fallback:block) => {
        $crate::guard!(@generate [else $fallback] $($accum)* @cond($($cond)+));
    };
    // Otherwise: accumulate tokens into the boolean condition
    (@parse [$($accum:tt)*] [$($cond:tt)*] -> $tok:tt $($tail:tt)*) => {
        $crate::guard!(@parse [$($accum)*] [$($cond)* $tok] -> $($tail)*);
    };

    // ---- Code Generation ----
    // Base case: nothing left to generate
    (@generate [else $fallback:block]) => {};

    // Generate a let-else statement
    (@generate [else $fallback:block] @let($p:pat, $($expr:tt)+) $($rest:tt)*) => {
        let $p = $crate::guard_as_expr!($($expr)+) else $fallback;
        $crate::guard!(@generate [else $fallback] $($rest)*);
    };

    // Generate an if-statement for a boolean condition
    (@generate [else $fallback:block] @cond($($cond:tt)+) $($rest:tt)*) => {
        if !($crate::guard_as_expr!($($cond)+)) $fallback
        $crate::guard!(@generate [else $fallback] $($rest)*);
    };

    // Entry point; MUST be at the bottom, otherwise the expansion will loop forever
    ($($tail:tt)*) => {
        // Start parsing: @parse [Saved Conditions] [Current Tokens] -> Remaining Tokens
        $crate::guard!(@parse [] [] -> $($tail)*);
    };

}

// Coerces a tt fragment back into an expr fragment.
#[macro_export]
#[doc(hidden)]
macro_rules! guard_as_expr {
    ($e:expr) => {
        $e
    };
}
