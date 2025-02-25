use conjure_core::{
    ast::{Atom, Expression},
    context::Context,
};
use conjure_oxide::{utils::essence_parser::parse_essence_file_native, Metadata};
use pretty_assertions::assert_eq;
use project_root::get_project_root;
use std::sync::{Arc, RwLock};

#[test]
fn test_parse_dominance() {
    let root = get_project_root().unwrap().canonicalize().unwrap();
    let path = root.join("conjure_oxide/tests/parser_tests");
    let file = "dominance_simple";

    let expected_dominance = Some(Expression::DominanceRelation(
        Metadata::new(),
        Box::new(Expression::And(
            Metadata::new(),
            vec![
                Expression::And(
                    Metadata::new(),
                    vec![
                        Expression::Leq(
                            Metadata::new(),
                            Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("cost"))),
                            Box::new(Expression::FromSolution(
                                Metadata::new(),
                                Box::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_uref("cost"),
                                )),
                            )),
                        ),
                        Expression::Leq(
                            Metadata::new(),
                            Box::new(Expression::Atomic(
                                Metadata::new(),
                                Atom::new_uref("carbon"),
                            )),
                            Box::new(Expression::FromSolution(
                                Metadata::new(),
                                Box::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_uref("carbon"),
                                )),
                            )),
                        ),
                    ],
                ),
                Expression::Or(
                    Metadata::new(),
                    vec![
                        Expression::Lt(
                            Metadata::new(),
                            Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("cost"))),
                            Box::new(Expression::FromSolution(
                                Metadata::new(),
                                Box::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_uref("cost"),
                                )),
                            )),
                        ),
                        Expression::Lt(
                            Metadata::new(),
                            Box::new(Expression::Atomic(
                                Metadata::new(),
                                Atom::new_uref("carbon"),
                            )),
                            Box::new(Expression::FromSolution(
                                Metadata::new(),
                                Box::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_uref("carbon"),
                                )),
                            )),
                        ),
                    ],
                ),
            ],
        )),
    ));

    let ctx = Arc::new(RwLock::new(Context::default()));
    let pth = path.to_str().unwrap();

    let res = parse_essence_file_native(pth, file, "essence", ctx);
    assert!(res.is_ok());

    let model = res.unwrap();
    assert_eq!(model.dominance, expected_dominance);
}

#[test]
fn test_dominance_twice() {
    let root = get_project_root().unwrap().canonicalize().unwrap();
    let path = root.join("conjure_oxide/tests/parser_tests");
    let file = "dominance_twice";

    let ctx = Arc::new(RwLock::new(Context::default()));
    let pth = path.to_str().unwrap();

    let res = parse_essence_file_native(pth, file, "essence", ctx);
    assert!(res.is_err());
}

#[test]
fn test_no_dominance() {
    let root = get_project_root().unwrap().canonicalize().unwrap();
    let path = root.join("conjure_oxide/tests/parser_tests");

    let pth = path.to_str().unwrap();
    let res_nodom = parse_essence_file_native(
        pth,
        "no_dominance",
        "essence",
        Arc::new(RwLock::new(Context::default())),
    );
    let res_dom = parse_essence_file_native(
        pth,
        "dominance_simple",
        "essence",
        Arc::new(RwLock::new(Context::default())),
    );

    assert!(res_nodom.is_ok());
    assert!(res_dom.is_ok());

    let mod_nodom = res_nodom.unwrap();
    let mod_dom = res_dom.unwrap();

    assert_eq!(mod_nodom.as_submodel(), mod_dom.as_submodel());
    assert!(mod_nodom.dominance.is_none());
    assert!(mod_dom.dominance.is_some());
}
