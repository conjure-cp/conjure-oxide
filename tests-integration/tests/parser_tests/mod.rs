use conjure_cp::ast::Metadata;
use conjure_cp::parse::tree_sitter::parse_essence_file_native;
use conjure_cp::{
    ast::{Atom, Expression, Moo, Name, declaration},
    context::Context,
    matrix_expr,
};
use pretty_assertions::assert_eq;
use project_root::get_project_root;
use std::sync::{Arc, RwLock};

#[test]
fn test_parse_dominance() {
    let root = get_project_root().unwrap().canonicalize().unwrap();
    let path = root.join("tests-integration/tests/parser_tests");
    let file = "dominance_simple";

    let ctx = Arc::new(RwLock::new(Context::default()));
    let pth = path.to_str().unwrap();
    let filepath = format!("{pth}/{file}.essence");

    let res = parse_essence_file_native(&filepath, ctx);

    assert!(res.is_ok());

    let model = res.unwrap();

    let symbols = model.as_submodel().symbols().clone();
    let cost_decl = symbols
        .lookup(&Name::User("cost".into()))
        .expect("Declaration for 'cost' not found in parsed model");
    let carbon_decl = symbols
        .lookup(&Name::User("carbon".into()))
        .expect("Declaration for 'carbon' not found in parsed model");

    let expected_dominance = Some(Expression::DominanceRelation(
        Metadata::new(),
        Moo::new(Expression::And(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expression::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![
                        Expression::Leq(
                            Metadata::new(),
                            Moo::new(Expression::Atomic(
                                Metadata::new(),
                                Atom::new_ref(cost_decl.clone())
                            )),
                            Moo::new(Expression::FromSolution(
                                Metadata::new(),
                                Moo::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_ref(cost_decl.clone())
                                )),
                            )),
                        ),
                        Expression::Leq(
                            Metadata::new(),
                            Moo::new(Expression::Atomic(
                                Metadata::new(),
                                Atom::new_ref(carbon_decl.clone())
                            )),
                            Moo::new(Expression::FromSolution(
                                Metadata::new(),
                                Moo::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_ref(carbon_decl.clone())
                                )),
                            )),
                        ),
                    ]),
                ),
                Expression::Or(
                    Metadata::new(),
                    Moo::new(matrix_expr![
                        Expression::Lt(
                            Metadata::new(),
                            Moo::new(Expression::Atomic(
                                Metadata::new(),
                                Atom::new_ref(cost_decl.clone())
                            )),
                            Moo::new(Expression::FromSolution(
                                Metadata::new(),
                                Moo::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_ref(cost_decl)
                                )),
                            )),
                        ),
                        Expression::Lt(
                            Metadata::new(),
                            Moo::new(Expression::Atomic(
                                Metadata::new(),
                                Atom::new_ref(carbon_decl.clone())
                            )),
                            Moo::new(Expression::FromSolution(
                                Metadata::new(),
                                Moo::new(Expression::Atomic(
                                    Metadata::new(),
                                    Atom::new_ref(carbon_decl)
                                )),
                            )),
                        ),
                    ]),
                ),
            ]),
        )),
    ));

    assert_eq!(model.dominance, expected_dominance);
}

#[test]
fn test_dominance_twice() {
    let root = get_project_root().unwrap().canonicalize().unwrap();
    let path = root.join("tests-integration/tests/parser_tests");
    let file = "dominance_twice";

    let ctx = Arc::new(RwLock::new(Context::default()));
    let pth = path.to_str().unwrap();
    let filepath = format!("{pth}/{file}.essence");

    let res = parse_essence_file_native(&filepath, ctx);
    assert!(res.is_err());
}

#[test]
fn test_no_dominance() {
    let root = get_project_root().unwrap().canonicalize().unwrap();
    let path = root.join("tests-integration/tests/parser_tests");

    let pth = path.to_str().unwrap();
    let filepath = format!("{pth}/no_dominance.essence");
    let res_nodom = parse_essence_file_native(&filepath, Arc::new(RwLock::new(Context::default())));

    // HACK: reset id so that the declarations in the two models can be compared...
    // this is a bad idea, but should be fine here...
    declaration::reset_declaration_id_unchecked();

    let filepath = format!("{pth}/dominance_simple.essence");
    let res_dom = parse_essence_file_native(&filepath, Arc::new(RwLock::new(Context::default())));

    assert!(res_nodom.is_ok());
    assert!(res_dom.is_ok());

    let mod_nodom = res_nodom.unwrap();
    let mod_dom = res_dom.unwrap();

    assert_eq!(mod_nodom.as_submodel(), mod_dom.as_submodel());
    assert!(mod_nodom.dominance.is_none());
    assert!(mod_dom.dominance.is_some());
}
