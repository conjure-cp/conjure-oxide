use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::GroundDomain;
use conjure_cp::ast::Moo;
use conjure_cp::ast::SymbolTable;
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use conjure_cp::ast::Atom;
use conjure_cp::ast::Expression;
use conjure_cp::ast::Literal;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Name;
use conjure_cp::rule_engine::ApplicationError;
use itertools::izip;

// takes a record field access and converts it to an atom via the representation rules
#[register_rule("Base", 2000, [RecordField])]
fn index_record_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // annoyingly, let chaining only works in if-lets, not let-elses, otherwise I could avoid the
    // indentation here!
    if let Expr::RecordField(_, subject, field_name) = expr
        && let Expr::Atomic(_, Atom::Reference(decl)) = &**subject
        && let Name::WithRepresentation(name, reprs) = &decl.name() as &Name
    {
        if reprs.first().is_none_or(|x| x.as_str() != "record_to_atom") {
            return Err(RuleNotApplicable);
        }

        let repr = symbols
            .get_representation(name, &["record_to_atom"])
            .unwrap()[0]
            .clone();

        // bind the domain to a variable so the borrowed entries outlive this statement
        let domain = decl.resolved_domain();
        let Some(GroundDomain::Record(entries)) = domain.as_deref() else {
            return Err(RuleNotApplicable);
        };

        // find the numerical index of the field name in the record, and convert it to an integer
        // literal for direct access (the representation indexes its variables by integer)
        let Some(idx) = entries.iter().position(|entry| &entry.name == field_name) else {
            return Err(RuleNotApplicable);
        };

        let index = Literal::Int(idx as i32 + 1);

        let indices_as_name = Name::Represented(Box::new((
            name.as_ref().clone(),
            "record_to_atom".into(),
            index.into(),
        )));

        let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

        Ok(RuleEffect::pure(subject))
    } else {
        Err(RuleNotApplicable)
    }
}

// dealing with equality over 2 record variables
#[register_rule("Base", 2000, [Eq])]
fn record_equality(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
        && let Some(domain) = decl.resolved_domain()
        && let Some(domain2) = decl2.resolved_domain()

        // .. and have record variable domains
        && let GroundDomain::Record(entries) = domain.as_ref()
        && let GroundDomain::Record(entries2) = domain2.as_ref()

        // we only support equality over records of the same size
        && entries.len() == entries2.len()

        // assuming all record entry names must match for equality
        && izip!(entries,entries2).all(|(entry1,entry2)| entry1.name == entry2.name)
    {
        let mut equality_constraints = vec![];
        // unroll the equality into equality constraints for each field
        for entry in entries {
            let left_elem =
                Expression::RecordField(Metadata::new(), Moo::clone(left), entry.name.clone());
            let right_elem =
                Expression::RecordField(Metadata::new(), Moo::clone(right), entry.name.clone());

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

        Ok(RuleEffect::pure(new_expr))
    } else {
        Err(RuleNotApplicable)
    }
}

// dealing with equality where the left is a record variable, and the right is a constant record
#[register_rule("Base", 2000, [Eq])]
fn record_to_const(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    if let Expr::Eq(_, left, right) = expr
        && let Expr::Atomic(_, Atom::Reference(decl)) = &**left
        && let Name::WithRepresentation(_, reprs) = &decl.name() as &Name
        && reprs.first().is_none_or(|x| x.as_str() == "record_to_atom")
    {
        let domain = decl
            .resolved_domain()
            .ok_or(ApplicationError::DomainError)?;

        let GroundDomain::Record(entries) = domain.as_ref() else {
            return Err(RuleNotApplicable);
        };

        let Some(rhs_record_names) = crate::utils::constant_record_names(right.as_ref()) else {
            return Err(RuleNotApplicable);
        };

        if entries.len() != rhs_record_names.len() {
            return Err(RuleNotApplicable);
        }

        for i in 0..entries.len() {
            if entries[i].name != rhs_record_names[i] {
                return Err(RuleNotApplicable);
            }
        }
        let mut equality_constraints = vec![];
        for entry in entries {
            let left_elem =
                Expression::RecordField(Metadata::new(), Moo::clone(left), entry.name.clone());
            let right_elem =
                Expression::RecordField(Metadata::new(), Moo::clone(right), entry.name.clone());

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
        Ok(RuleEffect::pure(new_expr))
    } else {
        Err(RuleNotApplicable)
    }
}
