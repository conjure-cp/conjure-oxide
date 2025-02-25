use conjure_oxide::ast::{
    Atom::Reference, Declaration, Domain::BoolDomain, Domain::IntDomain, Expression::*, Name, Range,
};
use conjure_oxide::solver::adaptors::sat_common::CNFModel;
use conjure_oxide::solver::SolverError;
use conjure_oxide::utils::testing::assert_eq_any_order;
use conjure_oxide::{Metadata, Model as ConjureModel};

#[test]
fn test_single_var() {
    // x -> [[1]]

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_constraint(Atomic(Metadata::new(), Reference(x.clone())));

    let res: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
    assert!(res.is_ok());

    let cnf = res.unwrap();

    assert_eq!(cnf.get_index(&x), Some(1));
    assert!(cnf.get_name(1).is_some());
    assert_eq!(cnf.get_name(1).unwrap(), &x);

    assert_eq!(cnf.clauses, vec![vec![1]]);
}

#[test]
fn test_single_not() {
    // Not(x) -> [[-1]]

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_constraint(Not(
        Metadata::new(),
        Box::from(Atomic(Metadata::new(), Reference(x.clone()))),
    ));

    let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();
    assert_eq!(cnf.get_index(&x), Some(1));
    assert_eq!(cnf.clauses, vec![vec![-1]]);

    assert_eq!(
        cnf.as_expression().unwrap(),
        And(
            Metadata::new(),
            vec![Or(
                Metadata::new(),
                vec![Not(
                    Metadata::new(),
                    Box::from(Atomic(Metadata::new(), Reference(x.clone())))
                )]
            )]
        )
    )
}

#[test]
fn test_single_or() {
    // Or(x, y) -> [[1, 2]]

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(y.clone(), BoolDomain));
    submodel.add_constraint(Or(
        Metadata::new(),
        vec![
            Atomic(Metadata::new(), Reference(x.clone())),
            Atomic(Metadata::new(), Reference(y.clone())),
        ],
    ));

    let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

    let xi = cnf.get_index(&x).unwrap();
    let yi = cnf.get_index(&y).unwrap();
    assert_eq_any_order(&cnf.clauses, &vec![vec![xi, yi]]);

    assert_eq!(
        cnf.as_expression().unwrap(),
        And(
            Metadata::new(),
            vec![Or(
                Metadata::new(),
                vec![
                    Atomic(Metadata::new(), Reference(x.clone())),
                    Atomic(Metadata::new(), Reference(y.clone())),
                ],
            )]
        )
    )
}

#[test]
fn test_or_not() {
    // Or(x, Not(y)) -> [[1, -2]]

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(y.clone(), BoolDomain));
    submodel.add_constraint(Or(
        Metadata::new(),
        vec![
            Atomic(Metadata::new(), Reference(x.clone())),
            Not(
                Metadata::new(),
                Box::from(Atomic(Metadata::new(), Reference(y.clone()))),
            ),
        ],
    ));

    let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

    let xi = cnf.get_index(&x).unwrap();
    let yi = cnf.get_index(&y).unwrap();
    assert_eq_any_order(&cnf.clauses, &vec![vec![xi, -yi]]);

    assert_eq!(
        cnf.as_expression().unwrap(),
        And(
            Metadata::new(),
            vec![Or(
                Metadata::new(),
                vec![
                    Atomic(Metadata::new(), Reference(x.clone())),
                    Not(
                        Metadata::new(),
                        Box::from(Atomic(Metadata::new(), Reference(y.clone())))
                    ),
                ]
            )]
        )
    )
}

#[test]
fn test_multiple() {
    // [x, y] - equivalent to And(x, y) -> [[1], [2]]

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(y.clone(), BoolDomain));
    submodel.add_constraint(Atomic(Metadata::new(), Reference(x.clone())));
    submodel.add_constraint(Atomic(Metadata::new(), Reference(y.clone())));

    let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

    let xi = cnf.get_index(&x).unwrap();
    let yi = cnf.get_index(&y).unwrap();
    assert_eq_any_order(&cnf.clauses, &vec![vec![xi], vec![yi]]);

    assert_eq!(
        cnf.as_expression().unwrap(),
        And(
            Metadata::new(),
            vec![
                Or(
                    Metadata::new(),
                    vec![Atomic(Metadata::new(), Reference(x.clone()))]
                ),
                Or(
                    Metadata::new(),
                    vec![Atomic(Metadata::new(), Reference(y.clone()))]
                )
            ]
        )
    )
}

#[test]
fn test_and() {
    // And(x, y) -> [[1], [2]]

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(y.clone(), BoolDomain));
    submodel.add_constraint(And(
        Metadata::new(),
        vec![
            Atomic(Metadata::new(), Reference(x.clone())),
            Atomic(Metadata::new(), Reference(y.clone())),
        ],
    ));

    let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

    let xi = cnf.get_index(&x).unwrap();
    let yi = cnf.get_index(&y).unwrap();
    assert_eq_any_order(&cnf.clauses, &vec![vec![xi], vec![yi]]);

    assert_eq!(
        cnf.as_expression().unwrap(),
        And(
            Metadata::new(),
            vec![
                Or(
                    Metadata::new(),
                    vec![Atomic(Metadata::new(), Reference(x.clone())),]
                ),
                Or(
                    Metadata::new(),
                    vec![Atomic(Metadata::new(), Reference(y.clone())),]
                )
            ]
        )
    )
}

#[test]
fn test_nested_ors() {
    // Or(x, Or(y, z)) -> [[1, 2, 3]]

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));
    let z: Name = Name::UserName(String::from('z'));

    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(y.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(z.clone(), BoolDomain));

    submodel.add_constraint(Or(
        Metadata::new(),
        vec![
            Atomic(Metadata::new(), Reference(x.clone())),
            Or(
                Metadata::new(),
                vec![
                    Atomic(Metadata::new(), Reference(y.clone())),
                    Atomic(Metadata::new(), Reference(z.clone())),
                ],
            ),
        ],
    ));

    let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

    let xi = cnf.get_index(&x).unwrap();
    let yi = cnf.get_index(&y).unwrap();
    let zi = cnf.get_index(&z).unwrap();
    assert_eq_any_order(&cnf.clauses, &vec![vec![xi, yi, zi]]);

    assert_eq!(
        cnf.as_expression().unwrap(),
        And(
            Metadata::new(),
            vec![Or(
                Metadata::new(),
                vec![
                    Atomic(Metadata::new(), Reference(x.clone())),
                    Atomic(Metadata::new(), Reference(y.clone())),
                    Atomic(Metadata::new(), Reference(z.clone())),
                ]
            )]
        )
    )
}

#[test]
fn test_int() {
    // y is an IntDomain - only booleans should be allowed

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(
        y.clone(),
        IntDomain(vec![Range::Bounded(1, 2)]),
    ));

    submodel.add_constraint(Atomic(Metadata::new(), Reference(x.clone())));
    submodel.add_constraint(Atomic(Metadata::new(), Reference(y.clone())));

    let cnf: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
    assert!(cnf.is_err());
}

#[test]
fn test_eq() {
    // Eq(x, y) - this operation is not allowed

    let mut model: ConjureModel = ConjureModel::default();
    let submodel = model.as_submodel_mut();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    submodel.add_symbol(Declaration::new_var(x.clone(), BoolDomain));
    submodel.add_symbol(Declaration::new_var(y.clone(), BoolDomain));

    submodel.add_constraint(Eq(
        Metadata::new(),
        Box::from(Atomic(Metadata::new(), Reference(x.clone()))),
        Box::from(Atomic(Metadata::new(), Reference(y.clone()))),
    ));

    let cnf: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
    assert!(cnf.is_err());
}
