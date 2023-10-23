use std::rc::Rc;

fn main() {
    let a = Name::UserName(String::from("a"));
    let b = Name::UserName(String::from("b"));
    let c = Name::UserName(String::from("c"));

    let a_decision_variable = Rc::new(DecisionVariable {
        name: a,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    });
    let b_decision_variable = Rc::new(DecisionVariable {
        name: b,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    });
    let c_decision_variable = Rc::new(DecisionVariable {
        name: c,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    });

    // find a,b,c : int(1..3)
    // such that a + b + c = 4
    // such that a >= b
    let m = Model {
        statements: vec![
            Statement::Declaration(Rc::clone(&a_decision_variable)),
            Statement::Declaration(Rc::clone(&b_decision_variable)),
            Statement::Declaration(Rc::clone(&c_decision_variable)),
            Statement::Constraint(Expression::Eq(
                Box::from(Expression::Sum(vec![
                    Expression::Reference(Rc::clone(&a_decision_variable)),
                    Expression::Reference(Rc::clone(&b_decision_variable)),
                    Expression::Reference(Rc::clone(&c_decision_variable)),
                ])),
                Box::from(Expression::ConstantInt(4)),
            )),
            Statement::Constraint(Expression::Geq(
                Box::from(Expression::Reference(Rc::clone(&a_decision_variable))),
                Box::from(Expression::Reference(Rc::clone(&b_decision_variable))),
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
struct Model {
    statements: Vec<Statement>,
}

#[derive(Debug)]
struct DecisionVariable {
    name: Name,
    domain: Domain,
}

#[derive(Debug)]
enum Statement {
    Declaration(Rc<DecisionVariable>),
    Constraint(Expression),
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
enum Expression {
    ConstantInt(i32),
    Reference(Rc<DecisionVariable>),
    Sum(Vec<Expression>),
    Eq(Box<Expression>, Box<Expression>),
    Geq(Box<Expression>, Box<Expression>),
}
