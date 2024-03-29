#[test]
fn test_single_var() {
    // x -> [[1]]

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_constraint(Reference(Metadata::new(), x.clone()));

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

    let x: Name = Name::UserName(String::from('x'));
    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_constraint(Not(
        Metadata::new(),
        Box::from(Reference(Metadata::new(), x.clone())),
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
                    Box::from(Reference(Metadata::new(), x.clone()))
                )]
            )]
        )
    )
}

#[test]
fn test_single_or() {
    // Or(x, y) -> [[1, 2]]

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

    model.add_constraint(Or(
        Metadata::new(),
        vec![
            Reference(Metadata::new(), x.clone()),
            Reference(Metadata::new(), y.clone()),
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
                    Reference(Metadata::new(), x.clone()),
                    Reference(Metadata::new(), y.clone())
                ]
            )]
        )
    )
}

#[test]
fn test_or_not() {
    // Or(x, Not(y)) -> [[1, -2]]

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

    model.add_constraint(Or(
        Metadata::new(),
        vec![
            Reference(Metadata::new(), x.clone()),
            Not(
                Metadata::new(),
                Box::from(Reference(Metadata::new(), y.clone())),
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
                    Reference(Metadata::new(), x.clone()),
                    Not(
                        Metadata::new(),
                        Box::from(Reference(Metadata::new(), y.clone()))
                    )
                ]
            )]
        )
    )
}

#[test]
fn test_multiple() {
    // [x, y] - equivalent to And(x, y) -> [[1], [2]]

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

    model.add_constraint(Reference(Metadata::new(), x.clone()));
    model.add_constraint(Reference(Metadata::new(), y.clone()));

    let cnf: CNFModel = CNFModel::from_conjure(model).unwrap();

    let xi = cnf.get_index(&x).unwrap();
    let yi = cnf.get_index(&y).unwrap();
    assert_eq_any_order(&cnf.clauses, &vec![vec![xi], vec![yi]]);

    assert_eq!(
        cnf.as_expression().unwrap(),
        And(
            Metadata::new(),
            vec![
                Or(Metadata::new(), vec![Reference(Metadata::new(), x.clone())]),
                Or(Metadata::new(), vec![Reference(Metadata::new(), y.clone())])
            ]
        )
    )
}

#[test]
fn test_and() {
    // And(x, y) -> [[1], [2]]

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

    model.add_constraint(And(
        Metadata::new(),
        vec![
            Reference(Metadata::new(), x.clone()),
            Reference(Metadata::new(), y.clone()),
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
                Or(Metadata::new(), vec![Reference(Metadata::new(), x.clone())]),
                Or(Metadata::new(), vec![Reference(Metadata::new(), y.clone())])
            ]
        )
    )
}

#[test]
fn test_nested_ors() {
    // Or(x, Or(y, z)) -> [[1, 2, 3]]

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));
    let z: Name = Name::UserName(String::from('z'));

    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(z.clone(), DecisionVariable { domain: BoolDomain });

    model.add_constraint(Or(
        Metadata::new(),
        vec![
            Reference(Metadata::new(), x.clone()),
            Or(
                Metadata::new(),
                vec![
                    Reference(Metadata::new(), y.clone()),
                    Reference(Metadata::new(), z.clone()),
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
                    Reference(Metadata::new(), x.clone()),
                    Reference(Metadata::new(), y.clone()),
                    Reference(Metadata::new(), z.clone())
                ]
            )]
        )
    )
}

#[test]
fn test_int() {
    // y is an IntDomain - only booleans should be allowed

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(
        y.clone(),
        DecisionVariable {
            domain: IntDomain(vec![]),
        },
    );

    model.add_constraint(Reference(Metadata::new(), x.clone()));
    model.add_constraint(Reference(Metadata::new(), y.clone()));

    let cnf: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
    assert!(cnf.is_err());
}

#[test]
fn test_eq() {
    // Eq(x, y) - this operation is not allowed

    let mut model: ConjureModel = ConjureModel::default();

    let x: Name = Name::UserName(String::from('x'));
    let y: Name = Name::UserName(String::from('y'));

    model.add_variable(x.clone(), DecisionVariable { domain: BoolDomain });
    model.add_variable(y.clone(), DecisionVariable { domain: BoolDomain });

    model.add_constraint(Expression::Eq(
        Metadata::new(),
        Box::from(Reference(Metadata::new(), x.clone())),
        Box::from(Reference(Metadata::new(), y.clone())),
    ));

    let cnf: Result<CNFModel, SolverError> = CNFModel::from_conjure(model);
    assert!(cnf.is_err());
}
