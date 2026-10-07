use super::FunctionPacked;
use crate::guard;
use crate::shared::utils::{as_eq_or_neq, collect_eq_or_neq};
use crate::types::function::FunctionExplicit;
use conjure_cp::ast::{
    Atom, Domain, Expression as Expr, GroundDomain, Metadata, Moo, Reference, SymbolTable,
};
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{
    ApplicationResult, RuleEffect as Reduction, register_rule, register_rule_set,
};
use conjure_cp::{into_matrix_expr, matrix_expr};
use parking_lot::MappedRwLockReadGuard;

// Packed functions are backend-neutral, so make their lowering rules available for every solver
// family.
register_rule_set!("ReprFunctionPacked", ("Base"), |_| true);

/// Channelling constraint between FunctionExplicit and FunctionPacked for the same variable.
/// ```plain
/// x#FunctionExplicit = x#FunctionPacked
/// ~>
/// decoded packed values = explicit values, decoded definedness = explicit definedness
/// ```
#[register_rule("ReprFunctionPacked", 9700, [Eq, Neq])]
fn function_channel_explicit_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re_a)) = lhs &&
        let Expr::Atomic(_, Atom::Reference(re_b)) = rhs &&
        let Some((packed, explicit)) = as_channeling_pair(re_a, re_b)
        else {
            return Err(RuleNotApplicable);
        }
    );

    let n = explicit.domain_values.len() as i32;
    let values = (1..=n).map(|index| (packed.value_expr(index), explicit.value_expr(index)));
    let definedness =
        (1..=n).map(|index| (packed.defined_expr(index), explicit.defined_expr(index)));
    Ok(Reduction::pure(collect_eq_or_neq(
        neq,
        values.chain(definedness),
    )))
}

/// Application of a packed function variable
/// ```plain
/// f(arg)
/// ~>
/// { value @ defined }
///   where, at the top level,
///     defined <-> or([ arg = domainValues[i] /\ <defined at i> | i ])
///     defined -> or([ arg = domainValues[i] /\ value = <value at i> | i ])
/// ```
///
/// Each domain value is a digit of the packed integer at its own place value, so a position that is
/// not known until solving cannot be read with a digit formula. Rather than index a matrix of those
/// formulas -- which nothing can turn into a Minion `element`, as its entries are arithmetic rather
/// than atoms -- the value is an auxiliary pinned by a disjunction over the positions.
///
/// Both auxiliaries are pinned at the *top level*, not by the bubble condition: a bubble condition
/// can be satisfied by being false, which in a context where false is acceptable would let the
/// solver pick a `value` unrelated to the function. `defined` covers a partial function with no
/// entry for `arg` and an `arg` outside the function's domain alike.
#[register_rule("ReprFunctionPacked", 9700, [Image])]
fn image_function_packed(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Image(_, function, arg) = expr &&
        let Expr::Atomic(_, Atom::Reference(re)) = function.as_ref() &&
        let Some(repr) = re.get_repr_as::<FunctionPacked>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let domain = function
        .domain_of()
        .ok_or(RuleNotApplicable)?
        .resolve()
        .map_err(|_| RuleNotApplicable)?;
    let GroundDomain::Function(_, _, codomain) = domain.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();
    let value = Expr::from(Reference::new(
        symbols.gen_find_auxiliary(&codomain.clone().into()),
    ));
    let defined = Expr::from(Reference::new(symbols.gen_find_auxiliary(&Domain::bool())));

    let arg = arg.as_ref().clone();
    let at_position = |index: i32| {
        Expr::Eq(
            Metadata::new(),
            Moo::new(arg.clone()),
            Moo::new(Expr::from(repr.domain_values[(index - 1) as usize].clone())),
        )
    };
    let n = repr.domain_values.len() as i32;

    let defined_here: Vec<Expr> = (1..=n)
        .map(|index| {
            Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![at_position(index), repr.defined_expr(index)]),
            )
        })
        .collect();
    let value_here: Vec<Expr> = (1..=n)
        .map(|index| {
            let takes = Expr::Eq(
                Metadata::new(),
                Moo::new(value.clone()),
                Moo::new(repr.value_expr(index)),
            );
            Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![at_position(index), takes]),
            )
        })
        .collect();

    let new_top = vec![
        Expr::Iff(
            Metadata::new(),
            Moo::new(defined.clone()),
            Moo::new(Expr::Or(
                Metadata::new(),
                Moo::new(into_matrix_expr![defined_here]),
            )),
        ),
        Expr::Imply(
            Metadata::new(),
            Moo::new(defined.clone()),
            Moo::new(Expr::Or(
                Metadata::new(),
                Moo::new(into_matrix_expr![value_here]),
            )),
        ),
    ];

    Ok(Reduction::new(
        Expr::Bubble(Metadata::new(), Moo::new(value), Moo::new(defined)),
        new_top,
        symbols,
    ))
}

type PackedState<'a> = MappedRwLockReadGuard<'a, <FunctionPacked as ReprRule>::DeclLevel>;
type ExplicitState<'a> = MappedRwLockReadGuard<'a, <FunctionExplicit as ReprRule>::DeclLevel>;
fn as_channeling_pair<'a>(
    lhs: &'a Reference,
    rhs: &'a Reference,
) -> Option<(PackedState<'a>, ExplicitState<'a>)> {
    if lhs.ptr != rhs.ptr {
        return None;
    }
    let packed = match (
        lhs.get_repr_as::<FunctionPacked>(),
        rhs.get_repr_as::<FunctionPacked>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    let explicit = match (
        lhs.get_repr_as::<FunctionExplicit>(),
        rhs.get_repr_as::<FunctionExplicit>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    Some((packed, explicit))
}
