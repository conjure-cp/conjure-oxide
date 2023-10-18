
fn main() {

    // Builder Pattern
    let builder = ModelBuilder::new();
    let domain = Domain::IntDomain(vec![Range::Bounded(1, 3)]);
    let m1 = builder
        .find(String::from("a"), domain.clone())
        .find(String::from("b"), domain.clone())
        .find(String::from("c"), domain.clone())
        .such_that(Expression::Eq(
            Box::from(Expression::Sum(vec![
                Expression::Reference(DecisionVariable {
                    name: Name::UserName(String::from("a")),
                    domain: domain.clone(),
                }),
                Expression::Reference(DecisionVariable {
                    name: Name::UserName(String::from("b")),
                    domain: domain.clone(),
                }),
                Expression::Reference(DecisionVariable {
                    name: Name::UserName(String::from("c")),
                    domain: domain.clone(),
                }),
            ])),
            Box::from(Expression::ConstantInt(4)),
        ))
        .build();

    // Manually
    let a = Name::UserName(String::from("a"));
    let b = Name::UserName(String::from("b"));
    let c = Name::UserName(String::from("c"));

    let a_decision_variable = DecisionVariable {
        name: a,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    };
    let a_reference = Expression::Reference(a_decision_variable.clone());

    let b_decision_variable = DecisionVariable {
        name: b,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    };
    let b_reference = Expression::Reference(b_decision_variable.clone());

    let c_decision_variable = DecisionVariable {
        name: c,
        domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    };
    let c_reference = Expression::Reference(c_decision_variable.clone());

    let m2 = Model {
        statements: vec![
            Statement::Declaration(a_decision_variable),
            Statement::Declaration(b_decision_variable),
            Statement::Declaration(c_decision_variable),
            Statement::Constraint(Expression::Eq(
                Box::from(Expression::Sum(vec![a_reference, b_reference, c_reference])),
                Box::from(Expression::ConstantInt(4))),
            ),
        ],
    };

    assert!(m1 == m2)
}

// Builder Pattern

struct ModelBuilder {
    statements: Vec<Statement>,
}

impl ModelBuilder {
    fn new() -> Self {
        ModelBuilder {
            statements: Vec::new(),
        }
    }

    fn add_statement(mut self, statement: Statement) -> Self {
        self.statements.push(statement);
        self
    }

    fn find(self, name: String, domain: Domain) -> Self {
        let decision_variable = DecisionVariable {
            name: Name::UserName(name),
            domain,
        };
        let statement: Statement = Statement::Declaration(decision_variable);
        self.add_statement(statement)
    }

    fn such_that(self, expression: Expression) -> Self {
        self.add_statement(Statement::Constraint(expression))
    }

    fn build(self) -> Model {
        Model {
            statements: self.statements,
        }
    }
}

// Language Definitions

#[derive(Debug, PartialEq)]
struct Model {
    statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq)]
struct DecisionVariable {
    name: Name,
    domain: Domain,
}

#[derive(Debug, Clone, PartialEq)]
enum Name {
    UserName(String),
    MachineName(i32),
}

#[derive(Debug, Clone, PartialEq)]
enum Statement {
    Declaration(DecisionVariable),
    Constraint(Expression),
}

#[derive(Debug, Clone, PartialEq)]
enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

#[derive(Debug, Clone, PartialEq)]
enum Range<A> {
    Single(A),
    Bounded(A, A),
}

#[derive(Debug, Clone, PartialEq)]
enum Expression {
    ConstantInt(i32),
    Reference(DecisionVariable),
    Sum(Vec<Expression>),
    Eq(Box<Expression>, Box<Expression>),
}
