fn main() {
    let a = Name::UserName(String::from("a"));
    let b = Name::UserName(String::from("b"));
    let c = Name::UserName(String::from("c"));

    let a_decision_variable = DecisionVariable {
        name: a,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    };
    let a_reference = Expression::Reference(&a_decision_variable);

    let b_decision_variable = DecisionVariable {
        name: b,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    };
    let b_reference = Expression::Reference(&b_decision_variable);

    let c_decision_variable = DecisionVariable {
        name: c,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    };
    let c_reference = Expression::Reference(&c_decision_variable);

    let m = Model {
        statements: vec![
            Statement::Declaration(&a_decision_variable),
            Statement::Declaration(&b_decision_variable),
            Statement::Declaration(&c_decision_variable),
            Statement::Constraint(Expression::Eq(
                Box::from(Expression::Sum(vec![a_reference, b_reference, c_reference])),
                Box::from(Expression::ConstantInt(4)),
            )),
        ],
    };

    println!("{:#?}", m);
}

#[derive(Debug)]
enum Name {
    UserName(String),
    MachineName(i32),
}

#[derive(Debug)]
struct Model<'a> {
    statements: Vec<Statement<'a>>,
}

#[derive(Debug)]
struct DecisionVariable {
    name: Name,
    domain: Domain,
}

#[derive(Debug)]
enum Statement<'a> {
    Declaration(&'a DecisionVariable),
    Constraint(Expression<'a>),
}

#[derive(Debug)]
enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

#[derive(Debug)]
enum Range<A> {
    Single(A),
    Bounded(A, A),
}

#[derive(Debug)]
enum Expression<'a> {
    ConstantInt(i32),
    Reference(&'a DecisionVariable),
    Sum(Vec<Expression<'a>>),
    Eq(Box<Expression<'a>>, Box<Expression<'a>>),
}
