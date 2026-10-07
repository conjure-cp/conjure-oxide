//! Structural constraints for a binary relation's attributes (reflexive, symmetric, transitive,
//! etc), shared by every relation representation.
//!
//! Mirrors Conjure's `mkBinRelCons` (`Representations/Common.hs`). Each representation supplies
//! its own `member` closure testing "is `(x, y)` in the relation", since the cheapest way to test
//! that differs per representation (dense: `SafeIndex` into the bool matrix; sparse: `in` on the
//! channelled set of tuples) -- this module only owns the formula shapes and the quantifier
//! plumbing, which are identical either way.

use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::comprehension::{Comprehension, ComprehensionBuilder};
use conjure_cp::ast::{
    AbstractLiteral, BinaryAttr, DeclarationPtr, DomainPtr, Expression, Metadata, Moo, Name,
    SymbolTablePtr,
};

/// Builds `(x, y)` as an expression, for use as a channelled set's element or similar.
pub(crate) fn tuple2(x: &Expression, y: &Expression) -> Expression {
    Expression::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Tuple(vec![x.clone(), y.clone()]),
    )
}

/// Builds the structural constraints for `attrs`, quantifying over `inner_domain` (the relation's
/// single, shared inner domain -- only meaningful for binary relations whose two columns have the
/// same domain, which is the only case Conjure's validator and this campaign both allow binary
/// attributes on). `member(x, y)` must build a representation-appropriate expression testing
/// whether `(x, y)` is in the relation.
pub(crate) fn binary_relation_constraints(
    inner_domain: &DomainPtr,
    attrs: &[BinaryAttr],
    member: &impl Fn(&Expression, &Expression) -> Expression,
) -> Vec<Expression> {
    attrs
        .iter()
        .flat_map(|attr| binary_attr_constraint(inner_domain, attr, member))
        .collect()
}

fn binary_attr_constraint(
    dom: &DomainPtr,
    attr: &BinaryAttr,
    member: &impl Fn(&Expression, &Expression) -> Expression,
) -> Vec<Expression> {
    match attr {
        BinaryAttr::Reflexive => vec![quantify_same_domain(
            dom,
            &["x"],
            ACOperatorKind::And,
            |v| member(&v[0], &v[0]),
        )],
        BinaryAttr::Irreflexive => vec![quantify_same_domain(
            dom,
            &["x"],
            ACOperatorKind::And,
            |v| not(member(&v[0], &v[0])),
        )],
        BinaryAttr::Coreflexive => vec![quantify_same_domain(
            dom,
            &["x", "y"],
            ACOperatorKind::And,
            |v| implies(member(&v[0], &v[1]), eq(&v[0], &v[1])),
        )],
        BinaryAttr::Symmetric => vec![quantify_same_domain(
            dom,
            &["x", "y"],
            ACOperatorKind::And,
            |v| implies(member(&v[0], &v[1]), member(&v[1], &v[0])),
        )],
        BinaryAttr::AntiSymmetric => vec![quantify_same_domain(
            dom,
            &["x", "y"],
            ACOperatorKind::And,
            |v| {
                implies(
                    and2(member(&v[0], &v[1]), member(&v[1], &v[0])),
                    eq(&v[0], &v[1]),
                )
            },
        )],
        BinaryAttr::ASymmetric => vec![quantify_same_domain(
            dom,
            &["x", "y"],
            ACOperatorKind::And,
            |v| implies(member(&v[0], &v[1]), not(member(&v[1], &v[0]))),
        )],
        BinaryAttr::Transitive => vec![quantify_same_domain(
            dom,
            &["x", "y", "z"],
            ACOperatorKind::And,
            |v| {
                implies(
                    and2(member(&v[0], &v[1]), member(&v[1], &v[2])),
                    member(&v[0], &v[2]),
                )
            },
        )],
        BinaryAttr::Total => vec![quantify_same_domain(
            dom,
            &["x", "y"],
            ACOperatorKind::And,
            |v| or2(member(&v[0], &v[1]), member(&v[1], &v[0])),
        )],
        BinaryAttr::Connex => vec![quantify_same_domain(
            dom,
            &["x", "y"],
            ACOperatorKind::And,
            |v| or3(member(&v[0], &v[1]), member(&v[1], &v[0]), eq(&v[0], &v[1])),
        )],
        BinaryAttr::Euclidean => vec![quantify_same_domain(
            dom,
            &["x", "y", "z"],
            ACOperatorKind::And,
            |v| {
                implies(
                    and2(member(&v[0], &v[1]), member(&v[0], &v[2])),
                    member(&v[1], &v[2]),
                )
            },
        )],
        // Serial is a direct alias for LeftTotal in Conjure (mkBinRelCons: `one BinRelAttr_Serial =
        // one BinRelAttr_LeftTotal`); Oxide keeps both names as separate `BinaryAttr` variants
        // (matching the grammar's own two keywords) but they share this one implementation.
        BinaryAttr::Serial | BinaryAttr::LeftTotal => {
            vec![forall_exists(dom, |x, y| member(x, y))]
        }
        // `forAll y . exists x . member(x, y)`: the same shape as LeftTotal with the two
        // quantified variables' roles swapped, so `forall_exists`'s outer/inner variables just get
        // fed to `member` in the opposite order.
        BinaryAttr::RightTotal => vec![forall_exists(dom, |y, x| member(x, y))],
        // Derived attributes expand to a conjunction of the base formulas above, matching
        // Conjure's own definitions.
        BinaryAttr::Equivalence => [
            BinaryAttr::Reflexive,
            BinaryAttr::Symmetric,
            BinaryAttr::Transitive,
        ]
        .iter()
        .flat_map(|a| binary_attr_constraint(dom, a, member))
        .collect(),
        BinaryAttr::PartialOrder => [
            BinaryAttr::Reflexive,
            BinaryAttr::AntiSymmetric,
            BinaryAttr::Transitive,
        ]
        .iter()
        .flat_map(|a| binary_attr_constraint(dom, a, member))
        .collect(),
        BinaryAttr::LinearOrder => [
            BinaryAttr::Total,
            BinaryAttr::AntiSymmetric,
            BinaryAttr::Transitive,
        ]
        .iter()
        .flat_map(|a| binary_attr_constraint(dom, a, member))
        .collect(),
        BinaryAttr::WeakOrder => [BinaryAttr::Total, BinaryAttr::Transitive]
            .iter()
            .flat_map(|a| binary_attr_constraint(dom, a, member))
            .collect(),
        BinaryAttr::PreOrder => [BinaryAttr::Reflexive, BinaryAttr::Transitive]
            .iter()
            .flat_map(|a| binary_attr_constraint(dom, a, member))
            .collect(),
        BinaryAttr::StrictPartialOrder => [
            BinaryAttr::Irreflexive,
            BinaryAttr::ASymmetric,
            BinaryAttr::Transitive,
        ]
        .iter()
        .flat_map(|a| binary_attr_constraint(dom, a, member))
        .collect(),
    }
}

fn eq(a: &Expression, b: &Expression) -> Expression {
    Expression::Eq(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone()))
}

fn not(a: Expression) -> Expression {
    Expression::Not(Metadata::new(), Moo::new(a))
}

fn implies(a: Expression, b: Expression) -> Expression {
    Expression::Imply(Metadata::new(), Moo::new(a), Moo::new(b))
}

fn and2(a: Expression, b: Expression) -> Expression {
    Expression::And(Metadata::new(), Moo::new(conjure_cp::matrix_expr![a, b]))
}

fn or2(a: Expression, b: Expression) -> Expression {
    Expression::Or(Metadata::new(), Moo::new(conjure_cp::matrix_expr![a, b]))
}

fn or3(a: Expression, b: Expression, c: Expression) -> Expression {
    Expression::Or(Metadata::new(), Moo::new(conjure_cp::matrix_expr![a, b, c]))
}

/// Looks up a just-inserted quantified variable's declaration by name.
fn quantified_ref(symbols: &SymbolTablePtr, name: &Name) -> Expression {
    let decl = symbols.read().lookup(name).unwrap_or_else(|| {
        conjure_cp::bug!("expected a quantified variable {name} to be in scope")
    });
    conjure_cp::ast::Reference::new(decl).into()
}

/// Wraps a skip-operator comprehension in its AC-operator expression node directly (`And`, `Or`,
/// `Sum`, `Product`), rather than via `ACOperatorKind::as_expression`: that helper asserts its
/// argument already has `ReturnType::Matrix`, but `Comprehension::return_type` is simply its
/// return expression's type (e.g. `Boolean` for a `forAll`'s body), not a matrix wrapper. Matches
/// the pattern `FunctionAsRelation`'s `forall_pairs` already established.
fn wrap_ac(kind: ACOperatorKind, comprehension: Comprehension) -> Expression {
    let wrapped = Expression::Comprehension(Metadata::new(), Moo::new(comprehension));
    match kind {
        ACOperatorKind::And => Expression::And(Metadata::new(), Moo::new(wrapped)),
        ACOperatorKind::Or => Expression::Or(Metadata::new(), Moo::new(wrapped)),
        ACOperatorKind::Sum => Expression::Sum(Metadata::new(), Moo::new(wrapped)),
        ACOperatorKind::Product => Expression::Product(Metadata::new(), Moo::new(wrapped)),
        // Every caller in this file passes And/Or only.
        ACOperatorKind::Min | ACOperatorKind::Max => unreachable!(),
    }
}

/// `forAll v1 : &domains[0], v2 : &domains[1], ... . body(v1, v2, ...)` (or `Or`/`Sum`/etc, per
/// `kind`), built directly rather than via `essence_expr!` (which cannot build quantifier
/// expressions: they need a symbol table for the bound variables, which the macro does not
/// provide). `names.len()` must equal `domains.len()`.
pub(crate) fn quantify(
    domains: &[DomainPtr],
    names: &[&str],
    kind: ACOperatorKind,
    body: impl FnOnce(&[Expression]) -> Expression,
) -> Expression {
    let names: Vec<Name> = names.iter().map(|s| Name::user(s)).collect();
    let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new());
    for (name, domain) in names.iter().zip(domains) {
        builder = builder.generator(DeclarationPtr::new_find(name.clone(), domain.clone()));
    }
    let symbols = builder.return_expr_symboltable();
    let refs: Vec<Expression> = names.iter().map(|n| quantified_ref(&symbols, n)).collect();
    let return_expr = body(&refs);
    let mut comprehension = builder.with_return_value(return_expr);
    comprehension.skip_operator = Some(kind);
    wrap_ac(kind, comprehension)
}

/// [`quantify`] over the same domain repeated `names.len()` times, matching every binary-
/// relation-attribute formula's shape (they all quantify over the relation's one shared column
/// domain).
fn quantify_same_domain(
    domain: &DomainPtr,
    names: &[&str],
    kind: ACOperatorKind,
    body: impl FnOnce(&[Expression]) -> Expression,
) -> Expression {
    let domains = vec![domain.clone(); names.len()];
    quantify(&domains, names, kind, body)
}

/// `forAll x : &domain . exists y : &domain . body(x, y)`, for `Serial`/Conjure's `LeftTotal`.
/// Needs two nested comprehensions (an `Or`-quantified one inside an `And`-quantified one) rather
/// than `quantify`'s flat same-kind multi-variable form, since `y` must be existentially
/// quantified independently of `x`. The inner builder shares `x`'s symbol table so `x` stays in
/// scope while building the inner `exists`, matching the nested-generator precedent used
/// elsewhere for e.g. `j : int(1..&i)` depending on an outer `i`.
fn forall_exists(
    domain: &DomainPtr,
    body: impl Fn(&Expression, &Expression) -> Expression,
) -> Expression {
    let x_name = Name::user("x");
    let mut outer_builder = ComprehensionBuilder::new(SymbolTablePtr::new())
        .generator(DeclarationPtr::new_find(x_name.clone(), domain.clone()));
    let outer_symbols = outer_builder.generator_symboltable();
    let x_ref = quantified_ref(&outer_symbols, &x_name);

    let y_name = Name::user("y");
    let mut inner_builder = ComprehensionBuilder::new(outer_symbols)
        .generator(DeclarationPtr::new_find(y_name.clone(), domain.clone()));
    let inner_symbols = inner_builder.return_expr_symboltable();
    let y_ref = quantified_ref(&inner_symbols, &y_name);
    let mut inner_comprehension = inner_builder.with_return_value(body(&x_ref, &y_ref));
    inner_comprehension.skip_operator = Some(ACOperatorKind::Or);
    let exists_expr = wrap_ac(ACOperatorKind::Or, inner_comprehension);

    let mut outer_comprehension = outer_builder.with_return_value(exists_expr);
    outer_comprehension.skip_operator = Some(ACOperatorKind::And);
    wrap_ac(ACOperatorKind::And, outer_comprehension)
}
