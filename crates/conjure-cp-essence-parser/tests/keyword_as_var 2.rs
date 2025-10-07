use std::sync::{Arc, RwLock};

use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::parser::parse_essence_with_context;

fn keyword_as_var_tests(src: &str, key: &str) {
    let ctx = Arc::new(RwLock::new(Context::default()));
    let res = parse_essence_with_context(src, ctx);
    assert!(res.is_err(), "expected parse to return Err; got {:?}", res);
    let err = format!("{:?}", res.unwrap_err());
    assert!(
        err.to_lowercase().contains("keyword") || err.to_lowercase().contains(key),
        "error did not mention keyword: {}",
        err
    );

    // print the thing
    // eprintln!("error: {}", err);
}

#[test]
fn keyword_as_var_2x_find() {
    let src = "\
find find,b,c : int(1..3)
such that a + b + c = 4
such that a >= b
";
    let key = "find";
    keyword_as_var_tests(src, key);
}

#[test]
fn keyword_as_var_3x_find() {
    let src = "\
find find find,b,c : int(1..3)
such that a + b + c = 4
such that a >= b
";
    keyword_as_var_tests(src, "find");
}

#[test]
fn keyword_as_var_letting() {
    let src = "\
find letting,b,c : int(1..3)
such that a + b + c = 4
such that a >= b
";
    keyword_as_var_tests(src, "letting");
}

#[test]
fn keyword_as_var_true() {
    let src = "\
find true,b,c : int(1..3)
such that a + b + c = 4
such that a >= b
";
    keyword_as_var_tests(src, "true");
}