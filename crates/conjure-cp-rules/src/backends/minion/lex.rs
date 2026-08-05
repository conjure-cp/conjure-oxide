use crate::shared::utils::{as_resolved_atom, to_aux_var};
use crate::types::record::RecordComponents;
use crate::types::tuple::{TupleComponents, TuplePacked};
use conjure_cp::ast::{
    Atom, Expression as Expr, GroundDomain, IntVal, Metadata, Moo, Name, Range, SymbolTable, matrix,
};
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect,
    register_rule,
};
use itertools::Itertools as _;

fn lex_operand_elements(expr: &Expr) -> Result<Vec<Expr>, ApplicationError> {
    let expr = match expr {
        Expr::Flatten(_, None, inner) => inner.as_ref(),
        other => other,
    };
    if let Some(elements) = expr.unwrap_list() {
        return Ok(elements);
    }

    let Some((elements, domain)) = expr.clone().unwrap_matrix_unchecked() else {
        return Err(RuleNotApplicable);
    };
    let Some(ranges) = domain.as_int() else {
        return Err(RuleNotApplicable);
    };
    let [Range::Bounded(IntVal::Const(1), _)] = ranges[..] else {
        return Err(RuleNotApplicable);
    };
    Ok(elements)
}

fn lex_operand_to_atoms(
    operand: &Moo<Expr>,
    symbols: &mut SymbolTable,
    tops: &mut Vec<Expr>,
) -> Result<Vec<Atom>, ApplicationError> {
    if let Some(atoms) = lex_represented_matrix_to_atoms(operand.as_ref(), symbols)? {
        return Ok(atoms);
    }

    let mut atoms = vec![];
    for element in lex_operand_elements(operand.as_ref())? {
        atoms.extend(flatten_lex_element(&element, symbols, tops)?);
    }
    Ok(atoms)
}

/// Expand one lex-operand element into the flat scalar atoms Minion's `FlatLexLt`/`FlatLexLeq`
/// need. A list element is not always already scalar -- e.g. a matrix-of-tuples operand (produced
/// by a set's own "compare through representation" ordering rule when its elements are tuples)
/// has tuple-typed elements, and treating one of those as a ready atom would leak its still-
/// abstract declaration name straight into the backend. Splicing a compound element's own fields
/// into the flat list in its place instead preserves lex semantics exactly: comparing
/// `[t1, t2]` against `[u1, u2]` lexicographically is equivalent to comparing
/// `[t1.f1, t1.f2, t2.f1, t2.f2]` against `[u1.f1, u1.f2, u2.f1, u2.f2]`, since a tuple's own
/// ordering is itself lexicographic over its fields. Recurses so a field that is itself a
/// tuple/record keeps unwinding.
fn flatten_lex_element(
    element: &Expr,
    symbols: &mut SymbolTable,
    tops: &mut Vec<Expr>,
) -> Result<Vec<Atom>, ApplicationError> {
    if let Some(atom) = as_resolved_atom(element) {
        return Ok(vec![atom]);
    }

    if let Expr::Atomic(_, Atom::Reference(reference)) = element {
        // Tuple/record fields decompose into several sub-elements; a packed tuple is already a
        // single scalar integer that preserves lex order by construction (see
        // `types::tuple::packed::vertical::packed_cmp`), so it decomposes into just itself.
        let sub_elements = if let Some(repr) = reference.ptr().get_repr::<TupleComponents>() {
            Some(repr.field_exprs())
        } else if let Some(repr) = reference.ptr().get_repr::<RecordComponents>() {
            Some(repr.field_exprs())
        } else {
            reference
                .ptr()
                .get_repr::<TuplePacked>()
                .map(|repr| vec![repr.packed_expr()])
        };
        if let Some(sub_elements) = sub_elements {
            let mut flattened = Vec::with_capacity(sub_elements.len());
            for sub_element in &sub_elements {
                flattened.extend(flatten_lex_element(sub_element, symbols, tops)?);
            }
            return Ok(flattened);
        }
    }

    if let Some(aux) = to_aux_var(element, symbols) {
        *symbols = aux.symbols();
        tops.push(aux.top_level_expr());
        return Ok(vec![aux.as_atom()]);
    }

    Err(RuleNotApplicable)
}

fn lex_represented_matrix_to_atoms(
    operand: &Expr,
    symbols: &SymbolTable,
) -> Result<Option<Vec<Atom>>, ApplicationError> {
    let Expr::Atomic(_, Atom::Reference(declaration)) = operand else {
        return Ok(None);
    };
    let Name::WithRepresentation(name, representations) = &declaration.name() as &Name else {
        return Ok(None);
    };
    if representations
        .first()
        .is_none_or(|name| name.as_str() != "matrix_to_atom")
    {
        return Ok(None);
    }

    let declaration = symbols.lookup(name.as_ref()).ok_or(RuleNotApplicable)?;
    let representation = symbols
        .get_representation(name.as_ref(), &["matrix_to_atom"])
        .ok_or(RuleNotApplicable)?[0]
        .clone();
    let domain = declaration.resolved_domain().ok_or(RuleNotApplicable)?;
    let GroundDomain::Matrix(_, index_domains) = domain.as_ref() else {
        return Ok(None);
    };
    if index_domains.len() != 1 {
        return Ok(None);
    }

    let matrix_values = representation.expression_down(symbols)?;
    matrix::enumerate_indices(index_domains.clone())
        .map(|index| {
            matrix_values
                .get(&Name::Represented(Box::new((
                    name.as_ref().clone(),
                    "matrix_to_atom".into(),
                    index.iter().join("_").into(),
                ))))
                .cloned()
                .ok_or(RuleNotApplicable)?
                .try_into()
                .map_err(|_| RuleNotApplicable)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

#[register_rule("Minion", 2000, [LexLt, LexLeq])]
fn flatten_lex_lt_leq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };

    let mut symbols = symbols.clone();
    let mut tops = vec![];
    let mut lhs = lex_operand_to_atoms(lhs, &mut symbols, &mut tops)?;
    let mut rhs = lex_operand_to_atoms(rhs, &mut symbols, &mut tops)?;
    let new_expression = if lhs.len() == rhs.len() {
        match expr {
            Expr::LexLt(..) => Expr::FlatLexLt(Metadata::new(), lhs, rhs),
            Expr::LexLeq(..) => Expr::FlatLexLeq(Metadata::new(), lhs, rhs),
            _ => unreachable!(),
        }
    } else {
        let first_longer = lhs.len() > rhs.len();
        let min_len = lhs.len().min(rhs.len());
        lhs.truncate(min_len);
        rhs.truncate(min_len);
        if first_longer {
            Expr::FlatLexLt(Metadata::new(), lhs, rhs)
        } else {
            Expr::FlatLexLeq(Metadata::new(), lhs, rhs)
        }
    };

    if tops.is_empty() {
        Ok(RuleEffect::pure(new_expression))
    } else {
        Ok(RuleEffect::new(new_expression, tops, symbols))
    }
}
