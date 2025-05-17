use std::{
    cell::{Ref, RefCell},
    collections::HashSet,
    fmt::Display,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

use itertools::Itertools as _;
use serde::{Deserialize, Serialize};
use uniplate::{derive::Uniplate, Biplate as _};

use crate::{
    ast::Atom,
    context::Context,
    into_matrix_expr, matrix_expr,
    metadata::Metadata,
    solver::{Solver, SolverError},
};

use super::{Declaration, Domain, Expression, Model, Name, Range, SubModel, SymbolTable};

pub enum ComprehensionKind {
    Sum,
    And,
    Or,
}
/// A comprehension.
#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
#[uniplate(walk_into=[SubModel])]
#[biplate(to=SubModel,walk_into=[Expression])]
#[biplate(to=Expression,walk_into=[SubModel])]
pub struct Comprehension {
    expression: Expression,
    submodel: SubModel,
    induction_vars: Vec<Name>,
}

impl Comprehension {
    // Solves this comprehension using Minion, returning the resulting expressions.
    pub fn solve_with_minion(self) -> Result<Vec<Expression>, SolverError> {
        let minion = Solver::new(crate::solver::adaptors::Minion::new());
        // FIXME: weave proper context through
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));

        // only branch on the induction variables.
        model.search_order = Some(self.induction_vars.clone());

        *model.as_submodel_mut() = self.submodel.clone();

        let minion = minion.load_model(model.clone())?;

        let values = Arc::new(Mutex::new(Vec::new()));
        let values_ptr = Arc::clone(&values);

        tracing::debug!(model=%model.clone(),comprehension=%self.clone(),"Minion solving comprehension");
        let expression = self.expression;
        minion.solve(Box::new(move |sols| {
            // TODO: deal with represented names if induction variables are abslits.
            let values = &mut *values_ptr.lock().unwrap();
            values.push(sols);
            true
        }))?;

        let values = values.lock().unwrap().clone();
        Ok(values
            .clone()
            .into_iter()
            .map(|sols| {
                // substitute in values
                expression
                    .clone()
                    .transform_bi(Arc::new(move |atom: Atom| match atom {
                        Atom::Reference(name, _) if sols.contains_key(&name) => {
                            Atom::Literal(sols.get(&name).unwrap().clone())
                        }
                        x => x,
                    }))
            })
            .collect_vec())
    }

    pub fn domain_of(&self) -> Option<Domain> {
        self.expression
            .domain_of(&self.submodel.symbols())
            .map(|domain| {
                Domain::DomainMatrix(
                    Box::new(domain),
                    vec![Domain::IntDomain(vec![Range::UnboundedR(1)])],
                )
            })
    }
}

impl Display for Comprehension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let generators: String = self
            .submodel
            .symbols()
            .clone()
            .into_iter_local()
            .map(|(name, decl_rc): (Name, Rc<RefCell<Declaration>>)| {
                let borrowed_decl: Ref<'_, Declaration> = (*decl_rc).borrow();
                let domain: Domain = borrowed_decl.domain().unwrap().clone();
                (name, domain)
            })
            .map(|(name, domain): (Name, Domain)| format!("{name}: {domain}"))
            .join(",");

        let guards = self
            .submodel
            .constraints()
            .iter()
            .map(|x| format!("{x}"))
            .join(",");

        let generators_and_guards = itertools::join([generators, guards], ",");

        let expression = &self.expression;
        write!(f, "[{expression} | {generators_and_guards}]")
    }
}

/// A builder for a comprehension.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ComprehensionBuilder {
    guards: Vec<Expression>,
    generators: Vec<(Name, Domain)>,
    induction_variables: HashSet<Name>,
}

impl ComprehensionBuilder {
    pub fn new() -> Self {
        Default::default()
    }
    pub fn guard(mut self, guard: Expression) -> Self {
        self.guards.push(guard);
        self
    }

    pub fn generator(mut self, name: Name, domain: Domain) -> Self {
        assert!(!self.induction_variables.contains(&name));
        self.induction_variables.insert(name.clone());
        self.generators.push((name, domain));
        self
    }

    /// Creates a comprehension with the given return expression.
    ///
    /// If a comprehension kind is not given, comprehension guards containing decision variables
    /// are invalid, and will cause a panic.
    pub fn with_return_value(
        self,
        mut expression: Expression,
        parent: Rc<RefCell<SymbolTable>>,
        comprehension_kind: Option<ComprehensionKind>,
    ) -> Comprehension {
        let mut submodel = SubModel::new(parent);

        // TODO:also allow guards that reference lettings and givens.

        let induction_variables = self.induction_variables;

        // only guards referencing induction variables can go inside the comprehension
        let (induction_guards, other_guards): (Vec<_>, Vec<_>) = self
            .guards
            .into_iter()
            .partition(|x| is_induction_guard(&induction_variables, x));

        // handle guards that reference non-induction variables
        if !other_guards.is_empty() {
            let comprehension_kind = comprehension_kind.expect(
                "if any guards reference decision variables, a comprehension kind should be given",
            );

            let guard_expr = match other_guards.as_slice() {
                [x] => x.clone(),
                xs => Expression::And(Metadata::new(), Box::new(into_matrix_expr!(xs.to_vec()))),
            };

            expression = match comprehension_kind {
                ComprehensionKind::And => {
                    Expression::Imply(Metadata::new(), Box::new(guard_expr), Box::new(expression))
                }
                ComprehensionKind::Or => Expression::And(
                    Metadata::new(),
                    Box::new(Expression::And(
                        Metadata::new(),
                        Box::new(matrix_expr![guard_expr, expression]),
                    )),
                ),

                ComprehensionKind::Sum => {
                    panic!("guards that reference decision variables not yet implemented for sum");
                }
            }
        }

        submodel.add_constraints(induction_guards);
        for (name, domain) in self.generators {
            submodel
                .symbols_mut()
                .insert(Rc::new(RefCell::new(Declaration::new_var(name, domain))));
        }

        Comprehension {
            expression,
            submodel,
            induction_vars: induction_variables.into_iter().collect_vec(),
        }
    }
}

/// True iff the guard only references induction variables.
fn is_induction_guard(induction_variables: &HashSet<Name>, guard: &Expression) -> bool {
    guard
        .universe_bi()
        .iter()
        .all(|x| induction_variables.contains(x))
}
