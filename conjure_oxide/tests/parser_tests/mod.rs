use conjure_core::{
    ast::{Atom, Expression},
    context::Context,
};
use conjure_oxide::{
    utils::essence_parser::parse_essence_file_native, Metadata,
};
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
