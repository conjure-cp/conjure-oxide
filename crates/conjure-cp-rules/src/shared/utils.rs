use std::collections::VecDeque;

use conjure_cp::ast::eval_constant;
use conjure_cp::ast::{
    AbstractLiteral, Atom, DeclarationPtr, DomainPtr, Expression as Expr, Literal, Metadata, Moo,
    SymbolTable,
    categories::Category,
    comprehension::{Comprehension, ComprehensionQualifier},
    records::Field,
};
use conjure_cp::rule_engine::{ApplicationError, ApplicationError::RuleNotApplicable, RuleEffect};
use conjure_cp::{bug, bug_assert_eq, essence_expr, into_matrix_expr, matrix_expr};
use itertools::{Itertools, izip};

use tracing::{instrument, trace};
use uniplate::{Biplate, Uniplate};

/// True iff `expr` is an `Atom`.
pub fn is_atom(expr: &Expr) -> bool {
    matches!(expr, Expr::Atomic(_, _))
}

/// True iff `expr` is an `Atom` or `Not(Atom)`.
pub fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic(_, _) => true,
        Expr::Not(_, inner) => matches!(**inner, Expr::Atomic(_, _)),
        _ => false,
    }
}

/// True if `expr` is flat; i.e. it only contains atoms.
pub fn is_flat(expr: &Expr) -> bool {
    expr.children().iter().all(is_atom)
}

/// Rewrites the direct expression children of `expr`, preserving the number of children.
///
/// Returns the rebuilt expression and the number of children marked as changed by `rewrite`.
pub fn rewrite_children(
    expr: &Expr,
    mut rewrite: impl FnMut(Expr) -> (Expr, bool),
) -> (Expr, usize) {
    let mut num_changed = 0;
    let children: VecDeque<Expr> = expr
        .children()
        .into_iter()
        .map(|child| {
            let (new_child, changed) = rewrite(child);
            if changed {
                num_changed += 1;
            }
            new_child
        })
        .collect();

    (expr.with_children(children), num_changed)
}

/// Returns the only direct `Vec<Expr>` child of `expr`, if it has exactly one.
pub fn single_vec_child(expr: &Expr) -> Option<Vec<Expr>> {
    let mut child_vecs: VecDeque<Vec<Expr>> = expr.children_bi();
    if child_vecs.len() == 1 {
        child_vecs.pop_front()
    } else {
        None
    }
}

/// Rebuilds `expr` with a replacement for its only direct `Vec<Expr>` child.
pub fn with_single_vec_child(expr: &Expr, child: Vec<Expr>) -> Expr {
    expr.with_children_bi(VecDeque::from([child]))
}

pub fn eval_to_usize(expr: &Expr) -> usize {
    match eval_constant(expr) {
        Some(Literal::Int(n)) if n >= 0 => n as usize,
        Some(lit) => bug!("expected a non-negative integer, got `{lit}`"),
        None => bug!("expected a constant expression, got `{expr}`"),
    }
}

/// True iff the expression is a tuple literal.
pub fn is_tuple_lit(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::AbstractLiteral(_, AbstractLiteral::Tuple(..))
            | Expr::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Tuple(..)))
            )
    )
}

/// Gets the entries of a tuple expression, if it is one.
pub fn tuple_expr_entries(expr: &Expr) -> Option<Vec<Expr>> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Tuple(elems)) => Some(elems.clone()),
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)))) => {
            Some(elems.iter().cloned().map(Expr::from).collect())
        }
        _ => None,
    }
}

pub fn as_eq_or_neq(expr: &Expr) -> Result<(&Expr, &Expr, bool), ApplicationError> {
    match expr {
        Expr::Eq(_, left, right) => Ok((left.as_ref(), right.as_ref(), false)),
        Expr::Neq(_, left, right) => Ok((left.as_ref(), right.as_ref(), true)),
        _ => Err(RuleNotApplicable),
    }
}

pub fn collect_eq_or_neq<A, B>(neq: bool, itr: impl Iterator<Item = (A, B)>) -> Expr
where
    A: Into<Expr> + Clone,
    B: Into<Expr> + Clone,
{
    if neq {
        let constraints = itr.map(|(a, b)| essence_expr!(&a != &b)).collect_vec();
        Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
    } else {
        let constraints = itr.map(|(a, b)| essence_expr!(&a == &b)).collect_vec();
        Expr::And(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
    }
}

/// An atom is only genuinely ready for a flat solver-level comparison (`FlatLexLt`/`FlatLexLeq`,
/// `AllDiff`, ...) when it is a literal, or a reference whose own domain is already scalar. A
/// reference to a still-abstract domain (e.g. a set or tuple field) is wrapped in `Expr::Atomic`
/// like any other atom, but treating it as ready would leak the abstract declaration's name
/// straight into the backend instead of chasing through to whatever concrete representation it
/// ends up with.
pub(crate) fn as_resolved_atom(expr: &Expr) -> Option<Atom> {
    let Expr::Atomic(_, atom) = expr else {
        return None;
    };
    if let Atom::Reference(reference) = atom
        && reference.ptr().domain().is_some_and(|domain| {
            crate::passes::representation::domain_needs_representation(&domain)
        })
    {
        return None;
    }
    Some(atom.clone())
}

fn expressions_as_atoms(exprs: &[Expr]) -> Option<Vec<Atom>> {
    exprs.iter().map(as_resolved_atom).collect()
}

pub fn collect_cmp_exprs(cmp_op: &Expr, lhs_fields: Vec<Expr>, rhs_fields: Vec<Expr>) -> Expr {
    let len = lhs_fields.len();
    bug_assert_eq!(
        len,
        rhs_fields.len(),
        "comparison of collections with different shapes"
    );

    if let Some(lhs_atoms) = expressions_as_atoms(&lhs_fields)
        && let Some(rhs_atoms) = expressions_as_atoms(&rhs_fields)
    {
        match cmp_op {
            Expr::LexLeq(..) => return Expr::FlatLexLeq(Metadata::new(), lhs_atoms, rhs_atoms),
            Expr::LexLt(..) => return Expr::FlatLexLt(Metadata::new(), lhs_atoms, rhs_atoms),
            _ => {}
        }
    }

    // cases[j] means "fields 0..j are equal, and field j satisfies cmp_op" -- so field i's own
    // equality belongs to every *later* case's prefix (j > i), not to the earlier cases already
    // built from fields before it.
    let mut cases = vec![Vec::<Expr>::with_capacity(len); len];
    for (i, (lhs, rhs)) in izip!(lhs_fields, rhs_fields).enumerate() {
        let equal = essence_expr!(&lhs = &rhs);
        let comparison = field_cmp_expr(cmp_op, lhs, rhs);
        cases[i].push(comparison);
        for case in cases.iter_mut().skip(i + 1) {
            case.push(equal.clone());
        }
    }

    let conjunctions = cases
        .into_iter()
        .map(|case| Expr::And(Metadata::new(), Moo::new(into_matrix_expr!(case))))
        .collect();
    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(conjunctions)))
}

/// `cmp_op` is chosen for the tuple as a whole (e.g. `LexLt` because some *other* field is
/// abstract and needs list-style comparison), but each field must be compared with whatever
/// operator actually matches its own type. A scalar field has no list/lex structure of its own,
/// so a lexicographic operator must be downgraded to its plain counterpart; a still-abstract field
/// (e.g. a nested set) has no native `<`, so it keeps the lexicographic operator, wrapped as a
/// singleton list -- the shape a field's own representation-specific ordering rule (e.g.
/// `lex_explicit_sets`) expects -- and is left for that rule to expand.
fn field_cmp_expr(cmp_op: &Expr, lhs: Expr, rhs: Expr) -> Expr {
    let scalar = as_resolved_atom(&lhs).is_some() && as_resolved_atom(&rhs).is_some();
    match (cmp_op, scalar) {
        (Expr::LexLt(..), true) => Expr::Lt(Metadata::new(), Moo::new(lhs), Moo::new(rhs)),
        (Expr::LexLeq(..), true) => Expr::Leq(Metadata::new(), Moo::new(lhs), Moo::new(rhs)),
        (Expr::LexLt(..), false) => Expr::LexLt(
            Metadata::new(),
            Moo::new(matrix_expr![lhs]),
            Moo::new(matrix_expr![rhs]),
        ),
        (Expr::LexLeq(..), false) => Expr::LexLeq(
            Metadata::new(),
            Moo::new(matrix_expr![lhs]),
            Moo::new(matrix_expr![rhs]),
        ),
        _ => cmp_op.with_children(VecDeque::from([lhs, rhs])),
    }
}

pub fn as_comparison_op(expr: &Expr) -> Option<(Moo<Expr>, Moo<Expr>)> {
    match expr {
        Expr::Eq(_, lhs, rhs)
        | Expr::Neq(_, lhs, rhs)
        | Expr::Lt(_, lhs, rhs)
        | Expr::Gt(_, lhs, rhs)
        | Expr::Leq(_, lhs, rhs)
        | Expr::Geq(_, lhs, rhs) => Some((lhs.clone(), rhs.clone())),
        _ => None,
    }
}

pub fn as_lex_comparison_op(expr: &Expr) -> Option<(Moo<Expr>, Moo<Expr>)> {
    match expr {
        Expr::LexGt(_, lhs, rhs)
        | Expr::LexLt(_, lhs, rhs)
        | Expr::LexGeq(_, lhs, rhs)
        | Expr::LexLeq(_, lhs, rhs) => Some((lhs.clone(), rhs.clone())),
        _ => None,
    }
}

pub fn as_cmp_or_lex_op(expr: &Expr) -> Option<(Moo<Expr>, Moo<Expr>)> {
    as_lex_comparison_op(expr).or_else(|| as_comparison_op(expr))
}

pub fn eq_or_neq(neq: bool, lhs: Expr, rhs: Expr) -> Expr {
    if neq {
        essence_expr!(&lhs != &rhs)
    } else {
        essence_expr!(&lhs = &rhs)
    }
}

/// True if the entire AST is constants.
#[allow(dead_code)]
pub fn is_all_constant(expression: &Expr) -> bool {
    expression
        .universe_bi()
        .into_iter()
        .all(|atom| matches!(atom, Atom::Literal(_)))
}
/// Converts a vector of expressions to a vector of atoms.
///
/// # Returns
///
/// `Some(Vec<Atom>)` if the vectors direct children expressions are all atomic, otherwise `None`.
#[allow(dead_code)]
pub fn expressions_to_atoms(exprs: &Vec<Expr>) -> Option<Vec<Atom>> {
    let mut atoms: Vec<Atom> = vec![];
    for expr in exprs {
        let Expr::Atomic(_, atom) = expr else {
            return None;
        };
        atoms.push(atom.clone());
    }

    Some(atoms)
}

/// Creates a new auxiliary variable using the given expression.
///
/// # Returns
///
/// * `None` if `Expr` is a `Atom`, or `Expr` does not have a domain (for example, if it is a `Bubble`).
///
/// * `Some(ToAuxVarOutput)` if successful, containing:
///
///     + A new symbol table, modified to include the auxiliary variable.
///     + A new top level expression, containing the declaration of the auxiliary variable.
///     + A reference to the auxiliary variable to replace the existing expression with.
///
#[instrument(skip_all, fields(expr = %expr))]
pub fn to_aux_var(expr: &Expr, symbols: &SymbolTable) -> Option<ToAuxVarOutput> {
    let domain = to_aux_var_domain(expr)?;
    Some(materialise_aux_var(expr, symbols, &domain))
}

fn to_aux_var_domain(expr: &Expr) -> Option<DomainPtr> {
    // No need to put an atom in an aux_var
    if is_atom(expr) {
        if cfg!(debug_assertions) {
            trace!(why = "expression is an atom", "to_aux_var() failed");
        }
        return None;
    }

    // Anything that should be bubbled, bubble
    if !expr.is_safe() {
        if cfg!(debug_assertions) {
            trace!(why = "expression is unsafe", "to_aux_var() failed");
        }
        return None;
    }

    // Do not put abstract literals containing expressions into aux vars.
    //
    // e.g. for `[1,2,3,f/2,e][e]`, the lhs should not be put in an aux var.
    //
    // instead, we should flatten the elements inside this abstract literal, or wait for it to be
    // turned into an atom, or an abstract literal containing only literals - e.g. through an index
    // or slice operation.
    //
    if let Expr::AbstractLiteral(_, _) = expr {
        if cfg!(debug_assertions) {
            trace!(
                why = "expression is an abstract literal",
                "to_aux_var() failed"
            );
        }
        return None;
    }

    // Only flatten an expression if it contains decision variables or decision variables with some
    // constants.
    //
    // i.e. dont flatten things containing givens, quantified variables, just constants, etc.
    let categories = expr.universe_categories();

    assert!(!categories.is_empty());

    if !(categories.len() == 1 && categories.contains(&Category::Decision)
        || categories.len() == 2
            && categories.contains(&Category::Decision)
            && categories.contains(&Category::Constant))
    {
        if let Expr::ElementId(_, _, value) = expr {
            let value_categories = value.universe_categories();
            if !(value_categories.len() == 1 && value_categories.contains(&Category::Decision)
                || value_categories.len() == 2
                    && value_categories.contains(&Category::Decision)
                    && value_categories.contains(&Category::Constant))
            {
                if cfg!(debug_assertions) {
                    trace!(
                        why =
                            "expression has sub-expressions that are not in the decision category",
                        "to_aux_var() failed"
                    );
                }
                return None;
            }
        } else {
            if cfg!(debug_assertions) {
                trace!(
                    why = "expression has sub-expressions that are not in the decision category",
                    "to_aux_var() failed"
                );
            }
            return None;
        }
    }

    // Avoid introducing auxvars for generic matrix indexing (can create many redundant auxvars
    // before comprehension expansion). However, keep list indexing eligible so Minion lowering
    // can introduce `element` constraints in non-equality contexts.
    if let Expr::SafeIndex(_, subject, indices) = expr {
        let index_has_element_id = indices
            .iter()
            .any(|index| matches!(index, Expr::ElementId(..)));
        let can_lower_via_element = subject.clone().unwrap_list().is_some()
            && indices.iter().all(|i| matches!(i, Expr::Atomic(_, _)));

        if !can_lower_via_element && !index_has_element_id {
            if cfg!(debug_assertions) {
                trace!(expr=%expr, why = "matrix indexing is not element-lowerable", "to_aux_var() failed");
            }
            return None;
        }
    }

    let Some(domain) = expr.domain_of() else {
        if cfg!(debug_assertions) {
            trace!(expr=%expr, why = "could not find the domain of the expression", "to_aux_var() failed");
        }
        return None;
    };

    Some(domain)
}

fn materialise_aux_var(expr: &Expr, symbols: &SymbolTable, domain: &DomainPtr) -> ToAuxVarOutput {
    let mut symbols = symbols.clone();
    let decl = symbols.gen_find_auxiliary(domain);

    if cfg!(debug_assertions) {
        trace!(expr=%expr, "to_auxvar() succeeded in putting expr into an auxvar");
    }

    ToAuxVarOutput {
        aux_declaration: decl.clone(),
        aux_expression: Expr::AuxDeclaration(
            Metadata::new(),
            conjure_cp::ast::Reference::new(decl),
            Moo::new(expr.clone()),
        ),
        symbols,
        _unconstructable: (),
    }
}

/// Defers auxiliary variable allocation until a selected rule is materialised.
pub fn defer_aux_var(
    expr: &Expr,
    build: impl Fn(ToAuxVarOutput) -> RuleEffect + Send + Sync + 'static,
) -> Option<RuleEffect> {
    let domain = to_aux_var_domain(expr)?;
    let expr = expr.clone();

    Some(RuleEffect::deferred(move |symbols| {
        let aux = materialise_aux_var(&expr, symbols, &domain);
        build(aux)
    }))
}

/// Output data of `to_aux_var`.
pub struct ToAuxVarOutput {
    aux_declaration: DeclarationPtr,
    aux_expression: Expr,
    symbols: SymbolTable,
    _unconstructable: (),
}

impl ToAuxVarOutput {
    /// Returns the new auxiliary variable as an `Atom`.
    pub fn as_atom(&self) -> Atom {
        Atom::Reference(conjure_cp::ast::Reference::new(
            self.aux_declaration.clone(),
        ))
    }

    /// Returns the new auxiliary variable as an `Expression`.
    ///
    /// This expression will have default `Metadata`.
    pub fn as_expr(&self) -> Expr {
        Expr::Atomic(Metadata::new(), self.as_atom())
    }

    /// Returns the top level `Expression` to add to the model.
    pub fn top_level_expr(&self) -> Expr {
        self.aux_expression.clone()
    }

    /// Returns the new `SymbolTable`, modified to contain this auxiliary variable in the symbol table.
    pub fn symbols(&self) -> SymbolTable {
        self.symbols.clone()
    }
}

/// Clone comprehension with expression generator into its own detached comprehension scope
/// and rewrite all uses of the original quantified declaration to a fresh branch-local
/// expression generator.
pub fn replace_expression_generator_source(
    comp: &Comprehension,
    gen_decl: &DeclarationPtr,
    replacement_expr: Expr,
) -> (Comprehension, DeclarationPtr) {
    let replacement_ptr =
        DeclarationPtr::new_quantified_expr(gen_decl.name().clone(), replacement_expr);
    let mut comprehension = comp.clone();

    // detach the scope so rewriting this branch does not mutate the original
    // comprehension through shared pointers
    comprehension.symbols = comprehension.symbols.detach();

    // rewrite all uses of the original quantified declaration to the branch-local
    // generator declaration
    comprehension.return_expression =
        comprehension
            .return_expression
            .transform_bi(&|decl: DeclarationPtr| {
                if decl == *gen_decl {
                    replacement_ptr.clone()
                } else {
                    decl
                }
            });

    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .map(|qualifier| {
            qualifier.transform_bi(&|decl: DeclarationPtr| {
                if decl == *gen_decl {
                    replacement_ptr.clone()
                } else {
                    decl
                }
            })
        })
        .collect();

    // keep the detached local scope in sync with the rewritten generator
    // declarations used by this branch
    comprehension
        .symbols
        .write()
        .update_insert(replacement_ptr.clone());
    for qualifier in &comprehension.qualifiers {
        match qualifier {
            ComprehensionQualifier::ExpressionGenerator { ptr }
            | ComprehensionQualifier::Generator { ptr } => {
                comprehension.symbols.write().update_insert(ptr.clone());
            }
            ComprehensionQualifier::Condition(_) => {}
        }
    }

    (comprehension, replacement_ptr)
}

/// `Some(minimum)` iff `values` is a contiguous run of `Literal::Int`s starting at `minimum` --
/// the cheap case for [`unpack_literal_digit`], decodable as plain arithmetic rather than an
/// indexed lookup.
pub fn contiguous_int_min(values: &[Literal]) -> Option<i32> {
    let Literal::Int(minimum) = values.first()? else {
        return None;
    };
    values
        .iter()
        .enumerate()
        .all(|(offset, value)| *value == Literal::Int(*minimum + offset as i32))
        .then_some(*minimum)
}

/// Given a decoded position `digit` (a decision-variable expression, not necessarily constant --
/// this is what makes the compound case below non-trivial) and the ordered list of possible
/// literal values at that position, build an expression for "the value at position `digit`".
/// Shared between `TuplePacked` and `RecordPacked`'s own digit-decoding, since a packed field or
/// element can be either shape (and can nest one inside the other).
///
/// Scalar (bool, or non-contiguous int) values fall back to a plain `SafeIndex` into a literal
/// matrix of those values, dynamically indexed by `digit` -- the Minion backend already supports
/// that (the same shape `FunctionExplicit::values_matrix` uses). A *compound* (tuple or record)
/// value can't take that path: indexing a matrix of compound elements by a non-constant position
/// is not something the backend can turn into a per-field `Element` constraint chain, and it
/// silently produces an unresolved literal handed straight to the solver ("expected a literal but
/// got `AbstractLiteral(...)`"). Instead, each candidate's own fields are projected out (every
/// candidate shares the same shape, since the caller builds them all from one domain), and the
/// value is rebuilt as an inline tuple/record literal expression whose *own* fields are each
/// decoded recursively by this same function -- reusing `digit`, since every candidate is keyed by
/// the same packed position. A field that's itself a contiguous int range (the common case) hits
/// the cheap arithmetic path one level down; a doubly-nested compound value recurses again.
pub fn unpack_literal_digit(digit: &Expr, values: &[Literal]) -> Expr {
    if let Some(minimum) = contiguous_int_min(values) {
        return match minimum {
            0 => digit.clone(),
            minimum => essence_expr!(&digit + &minimum),
        };
    }
    if let Some(rebuilt) = unpack_compound_literal_digit(digit, values) {
        return rebuilt;
    }
    let values = values.iter().cloned().map(Expr::from).collect::<Vec<_>>();
    Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(values)),
        vec![essence_expr!(&digit + 1)],
    )
}

/// The compound-value case of [`unpack_literal_digit`]: `None` unless every value is a tuple (or
/// every value is a record with the same field names) -- guaranteed for a genuine packed field's
/// own value list, but checked directly rather than assumed, since a mismatch here should fall
/// back to the -- still correct, if unreduced until something else eliminates it -- generic path
/// instead of panicking.
fn unpack_compound_literal_digit(digit: &Expr, values: &[Literal]) -> Option<Expr> {
    match values.first()? {
        Literal::AbstractLiteral(AbstractLiteral::Tuple(first_fields)) => {
            let arity = first_fields.len();
            let mut field_values: Vec<Vec<Literal>> = vec![Vec::with_capacity(values.len()); arity];
            for value in values {
                let Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)) = value else {
                    return None;
                };
                if fields.len() != arity {
                    return None;
                }
                for (slot, field) in field_values.iter_mut().zip(fields) {
                    slot.push(field.clone());
                }
            }
            let rebuilt_fields = field_values
                .into_iter()
                .map(|values| unpack_literal_digit(digit, &values))
                .collect();
            Some(Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Tuple(rebuilt_fields),
            ))
        }
        Literal::AbstractLiteral(AbstractLiteral::Record(first_fields)) => {
            let names: Vec<_> = first_fields.iter().map(|field| field.name.clone()).collect();
            let mut field_values: Vec<Vec<Literal>> =
                vec![Vec::with_capacity(values.len()); names.len()];
            for value in values {
                let Literal::AbstractLiteral(AbstractLiteral::Record(fields)) = value else {
                    return None;
                };
                if fields.len() != names.len() {
                    return None;
                }
                for (name, slot) in names.iter().zip(field_values.iter_mut()) {
                    let field_value = fields.iter().find(|field| &field.name == name)?.value.clone();
                    slot.push(field_value);
                }
            }
            let rebuilt_fields = names
                .into_iter()
                .zip(field_values)
                .map(|(name, values)| Field {
                    name,
                    value: unpack_literal_digit(digit, &values),
                })
                .collect();
            Some(Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Record(rebuilt_fields),
            ))
        }
        _ => None,
    }
}
