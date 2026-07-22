use crate::shared::utils::to_aux_var;
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
        if let Ok(atom) = element.clone().try_into() {
            atoms.push(atom);
        } else if let Some(aux) = to_aux_var(&element, symbols) {
            *symbols = aux.symbols();
            tops.push(aux.top_level_expr());
            atoms.push(aux.as_atom());
        } else {
            return Err(RuleNotApplicable);
        }
    }
    Ok(atoms)
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
