use std::collections::HashMap;

fn main() {
    let a = Name::UserName(String::from("a"));
    let b = Name::UserName(String::from("b"));
    let c = Name::UserName(String::from("c"));

    let mut variables = HashMap::new();
    variables.insert(
        a.clone(),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );
    variables.insert(
        b.clone(),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );
    variables.insert(
        c.clone(),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );

    // find a,b,c : int(1..3)
    // such that a + b + c = 4
    // such that a >= b
    let mut m = Model {
        variables,
        constraints: vec![
            Expression::Eq(
                Box::new(Expression::Sum(vec![
                    Expression::Reference(a.clone()),
                    Expression::Reference(b.clone()),
                    Expression::Reference(c.clone()),
                ])),
                Box::new(Expression::ConstantInt(4)),
            ),
            Expression::Geq(
                Box::new(Expression::Reference(a.clone())),
                Box::new(Expression::Reference(b.clone())),
            ),
        ],
    };

    println!("{:#?}", m);

    // Updating the domain for variable 'a'
    m.update_domain(&a, Domain::IntDomain(vec![Range::Bounded(1, 2)]));

    println!("{:#?}", m);
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
enum Name {
    UserName(String),
    MachineName(i32),
}

#[derive(Debug)]
struct Model {
    variables: HashMap<Name, DecisionVariable>,
    constraints: Vec<Expression>,
}

impl Model {
    // Function to update a DecisionVariable based on its Name
    fn update_domain(&mut self, name: &Name, new_domain: Domain) {
        if let Some(decision_var) = self.variables.get_mut(name) {
            decision_var.domain = new_domain;
        }
    }
}

#[derive(Debug)]
struct DecisionVariable {
    domain: Domain,
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
    Reference(Name),
    Sum(Vec<Expression>),
    Eq(Box<Expression>, Box<Expression>),
    Geq(Box<Expression>, Box<Expression>),
}
