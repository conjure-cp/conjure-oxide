use conjure_cp::ast::AbstractLiteral;
use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::Moo;
use conjure_cp::ast::SymbolTable;
use conjure_cp::into_matrix_expr;
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use conjure_cp::ast::Atom;
use conjure_cp::ast::Domain;
use conjure_cp::ast::Expression;
use conjure_cp::ast::Literal;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Name;
use conjure_cp::rule_engine::ApplicationError;
use itertools::izip;

//takes a safe index expression and converts it to an atom via the representation rules
#[register_rule(("Base", 2000))]
fn index_record_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // annoyingly, let chaining only works in if-lets, not let-elses,otherwise I could avoid the
    // indentation here!
    if let Expr::SafeIndex(_, subject, indices) = expr
        && let Expr::Atomic(_, Atom::Reference(decl)) = &**subject
        && let Name::WithRepresentation(name, reprs) = &decl.name() as &Name
    {
        if reprs.first().is_none_or(|x| x.as_str() != "record_to_atom") {
            return Err(RuleNotApplicable);
        }

        // tuples are always one dimensional
        if indices.len() != 1 {
            return Err(RuleNotApplicable);
        }

        let repr = symbols
            .get_representation(name, &["record_to_atom"])
            .unwrap()[0]
            .clone();

        // let decl = symbols.lookup(name).unwrap();

        let Some(Domain::Record(_)) = decl.domain().map(|x| x.resolve(symbols)) else {
            return Err(RuleNotApplicable);
        };

        assert_eq!(
            indices.len(),
            1,
            "record indexing is always one dimensional"
        );

        let index = indices[0].clone();

        // during the conversion from unsafe index to safe index in bubbling
        // we convert the field name to a literal integer for direct access
        let Some(index) = index.into_literal() else {
            return Err(RuleNotApplicable); // we don't support non-literal indices
        };

        let indices_as_name = Name::Represented(Box::new((
            name.as_ref().clone(),
            "record_to_atom".into(),
            index.into(),
        )));

        let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

        Ok(Reduction::pure(subject))
    } else {
        Err(RuleNotApplicable)
    }
}

#[register_rule(("Bubble", 8000))]
fn record_index_to_bubble(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    // annoyingly, let chaining only works in if-lets, not let-elses,otherwise I could avoid the
    // indentation here!
    if let Expr::UnsafeIndex(_, subject, indices) = expr
        && let Expr::Atomic(_, Atom::Reference(decl)) = &**subject
        && let Name::WithRepresentation(_, reprs) = &decl.name() as &Name
    {
        if reprs.first().is_none_or(|x| x.as_str() != "record_to_atom") {
            return Err(RuleNotApplicable);
        }

        let domain = subject
            .domain_of()
            .ok_or(ApplicationError::DomainError)?
            .resolve(symtab);

        let Domain::Record(elems) = domain else {
            return Err(RuleNotApplicable);
        };

        assert_eq!(
            indices.len(),
            1,
            "record indexing is always one dimensional"
        );

        let index = indices[0].clone();

        let Expr::Atomic(_, Atom::Reference(decl)) = index else {
            return Err(RuleNotApplicable);
        };

        let name: &Name = &decl.name();

        // find what numerical index in elems matches the entry name
        let Some(idx) = elems.iter().position(|x| &x.name == name) else {
            return Err(RuleNotApplicable);
        };

        // converting to an integer for direct access
        let idx = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(idx as i32 + 1)));

        let bubble_constraint = Moo::new(Expression::And(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expression::Leq(
                    Metadata::new(),
                    Moo::new(idx.clone()),
                    Moo::new(Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Int(elems.len() as i32))
                    ))
                ),
                Expression::Geq(
                    Metadata::new(),
                    Moo::new(idx.clone()),
                    Moo::new(Expression::Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Int(1))
                    ))
                )
            ]),
        ));

        let new_expr = Moo::new(Expression::SafeIndex(
            Metadata::new(),
            subject.clone(),
            Vec::from([idx]),
        ));

        Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            new_expr,
            bubble_constraint,
        )))
    } else {
        Err(RuleNotApplicable)
    }
}

// dealing with equality over 2 record variables
#[register_rule(("Base", 2000))]
fn record_equality(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // annoyingly, let chaining only works in if-lets, not let-elses, otherwise I could avoid the
    // indentation here!

    // check if both sides are record variables
    if let Expr::Eq(_, left, right) = expr
        && let Expr::Atomic(_, Atom::Reference(decl)) = &**left
        && let Name::WithRepresentation(_, reprs) = &decl.name() as &Name
        && let Expr::Atomic(_, Atom::Reference(decl2)) = &**right
        && let Name::WithRepresentation(_, reprs2) = &decl2.name() as &Name

        // .. that have been represented with record_to_atom
        && reprs.first().is_none_or(|x| x.as_str() == "record_to_atom")
        && reprs2.first().is_none_or(|x| x.as_str() == "record_to_atom")
        && let Some(domain) = decl.domain().map(|x| x.resolve(symbols))
        && let Some(domain2) = decl2.domain().map(|x| x.resolve(symbols))

        // .. and have record variable domains
        && let Domain::Record(entries) = domain
        && let Domain::Record(entries2) = domain2

        // we only support equality over records of the same size
        && entries.len() == entries2.len()

        // assuming all record entry names must match for equality
        && izip!(&entries,&entries2).all(|(entry1,entry2)| entry1.name == entry2.name)
    {
        let mut equality_constraints = vec![];
        // unroll the equality into equality constraints for each field
        for i in 0..entries.len() {
            let left_elem = Expression::SafeIndex(
                Metadata::new(),
                Moo::clone(left),
                vec![Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int((i + 1) as i32)),
                )],
            );
            let right_elem = Expression::SafeIndex(
                Metadata::new(),
                Moo::clone(right),
                vec![Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int((i + 1) as i32)),
                )],
            );

            equality_constraints.push(Expression::Eq(
                Metadata::new(),
                Moo::new(left_elem),
                Moo::new(right_elem),
            ));
        }

        let new_expr = Expression::And(
            Metadata::new(),
            Moo::new(into_matrix_expr!(equality_constraints)),
        );

        Ok(Reduction::pure(new_expr))
    } else {
        Err(RuleNotApplicable)
    }
}

// dealing with equality where the left is a record variable, and the right is a constant record
#[register_rule(("Base", 2000))]
fn record_to_const(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if let Expr::Eq(_, left, right) = expr
        && let Expr::Atomic(_, Atom::Reference(decl)) = &**left
        && let Name::WithRepresentation(_, reprs) = &decl.name() as &Name
        && reprs.first().is_none_or(|x| x.as_str() == "record_to_atom")
    {
        let domain = decl
            .domain()
            .map(|x| x.resolve(symbols))
            .ok_or(ApplicationError::DomainError)?;

        let Domain::Record(entries) = domain else {
            return Err(RuleNotApplicable);
        };

        let Expr::AbstractLiteral(_, AbstractLiteral::Record(entries2)) = &**right else {
            return Err(RuleNotApplicable);
        };

        for i in 0..entries.len() {
            if entries[i].name != entries2[i].name {
                return Err(RuleNotApplicable);
            }
        }
        let mut equality_constraints = vec![];
        for i in 0..entries.len() {
            let left_elem = Expression::SafeIndex(
                Metadata::new(),
                Moo::clone(left),
                vec![Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int((i + 1) as i32)),
                )],
            );
            let right_elem = Expression::SafeIndex(
                Metadata::new(),
                Moo::clone(right),
                vec![Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int((i + 1) as i32)),
                )],
            );

            equality_constraints.push(Expression::Eq(
                Metadata::new(),
                Moo::new(left_elem),
                Moo::new(right_elem),
            ));
        }
        let new_expr = Expression::And(
            Metadata::new(),
            Moo::new(into_matrix_expr!(equality_constraints)),
        );
        Ok(Reduction::pure(new_expr))
    } else {
        Err(RuleNotApplicable)
    }
}
