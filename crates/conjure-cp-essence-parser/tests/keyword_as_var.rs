use std::sync::{Arc, RwLock};

use conjure_cp_core::context::Context;
use conjure_cp_essence_parser::parser::parse_essence_with_context;

#[test]
fn keyword_as_var_is_reported() {
    let src = "\
find find,b,c : int(1..3)
such that a + b + c = 4
such that a >= b
";
    let ctx = Arc::new(RwLock::new(Context::default()));
    let res = parse_essence_with_context(src, ctx);
    assert!(res.is_err(), "expected parse to return Err; got {:?}", res);
    let err = format!("{:?}", res.unwrap_err());
    assert!(
        err.to_lowercase().contains("keyword") || err.to_lowercase().contains("find"),
        "error did not mention keyword: {}",
        err
    );

    // print the thing
    eprintln!("error: {}", err);
}