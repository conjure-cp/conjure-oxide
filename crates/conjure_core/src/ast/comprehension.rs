#![allow(clippy::arc_with_non_send_sync)]
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap},
    fmt::Display,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

use itertools::Itertools as _;
use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Uniplate};

use crate::{
    ast::{
        Atom, DeclarationKind,
        serde::{HasId as _, ObjId},
    },
    bug,
    context::Context,
    into_matrix_expr, matrix_expr,
    metadata::Metadata,
    solver::{Solver, SolverError},
};

use super::{DeclarationPtr, Domain, Expression, Model, Name, Range, SubModel, SymbolTable};

// TODO: do not use Names to compare variables, use DeclarationPtr and ids instead
// see issue #930
//
// this will simplify *a lot* of the knarly stuff here, but can only be done once everything else
// uses DeclarationPtr.
//
// ~ nikdewally, 10/06/25

pub enum ComprehensionKind {
    Sum,
    And,
    Or,
}
/// A comprehension.
#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
#[biplate(to=SubModel)]
pub struct Comprehension {
    return_expression_submodel: SubModel,
    generator_submodel: SubModel,
    induction_vars: Vec<Name>,
}

impl Comprehension {
    pub fn domain_of(&self) -> Option<Domain> {
        let return_expr_domain = self
            .return_expression_submodel
            .clone()
            .into_single_expression()
            .domain_of()?;

        // return a list (matrix with index domain int(1..)) of return_expr elements
        Some(Domain::Matrix(
            Box::new(return_expr_domain),
            vec![Domain::Int(vec![Range::UnboundedR(1)])],
        ))
    }

    /// Solves this comprehension using Minion, returning the resulting expressions.
    ///
    /// If successful, this modifies the symbol table given to add aux-variables needed inside the
    /// expanded expressions.
    pub fn solve_with_minion(
        self,
        symtab: &mut SymbolTable,
    ) -> Result<Vec<Expression>, SolverError> {
        let minion = Solver::new(crate::solver::adaptors::Minion::new());
        // FIXME: weave proper context through
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));

        // only branch on the induction variables.
        model.search_order = Some(self.induction_vars.clone());

        *model.as_submodel_mut() = self.generator_submodel.clone();

        let minion = minion.load_model(model.clone())?;

        let values = Arc::new(Mutex::new(Vec::new()));
        let values_ptr = Arc::clone(&values);

        tracing::debug!(model=%model.clone(),comprehension=%self.clone(),"Minion solving comprehension");
        minion.solve(Box::new(move |sols| {
            // TODO: deal with represented names if induction variables are abslits.
            let values = &mut *values_ptr.lock().unwrap();
            values.push(sols);
            true
        }))?;

        let values = values.lock().unwrap().clone();

        let mut return_expressions = vec![];

        for value in values {
            // convert back to an expression

            let return_expression_submodel = self.return_expression_submodel.clone();
            let child_symtab = return_expression_submodel.symbols().clone();
            let return_expression = return_expression_submodel.into_single_expression();

            // we only want to substitute induction variables.
            // (definitely not machine names, as they mean something different in this scope!)
            let value: HashMap<_, _> = value
                .into_iter()
                .filter(|(n, _)| self.induction_vars.contains(n))
                .collect();

            let value_ptr = Arc::new(value);
            let value_ptr_2 = Arc::clone(&value_ptr);

            // substitute in the values for the induction variables
            let return_expression = return_expression.transform_bi(&move |x: Atom| {
                let Atom::Reference(ref ptr) = x else {
                    return x;
                };

                // is this referencing an induction var?
                let Some(lit) = value_ptr_2.get(&ptr.name()) else {
                    return x;
                };

                Atom::Literal(lit.clone())
            });

            // Copy the return expression's symbols into parent scope.

            // For variables in the return expression with machine names, create new declarations
            // for them in the parent symbol table, so that the machine names used are unique.
            //
            // Store the declaration translations in `machine_name_translations`.
            // These are stored as a map of (old declaration id) -> (new declaration ptr), as
            // declaration pointers do not implement hash.
            //
            let mut machine_name_translations: HashMap<ObjId, DeclarationPtr> = HashMap::new();

            // Populate `machine_name_translations`
            for (name, decl) in child_symtab.into_iter_local() {
                // do not add givens for induction vars to the parent symbol table.
                if value_ptr.get(&name).is_some()
                    && matches!(&decl.kind() as &DeclarationKind, DeclarationKind::Given(_))
                {
                    continue;
                }

                let Name::Machine(_) = &name else {
                    bug!(
                        "the symbol table of the return expression of a comprehension should only contain machine names"
                    );
                };

                let id = decl.id();
                let new_decl = symtab.gensym(&decl.domain().unwrap());

                machine_name_translations.insert(id, new_decl);
            }

            // Update references to use the new delcarations.
            #[allow(clippy::arc_with_non_send_sync)]
            let return_expression = return_expression.transform_bi(&move |atom: Atom| {
                if let Atom::Reference(ref decl) = atom
                    && let id = decl.id()
                    && let Some(new_decl) = machine_name_translations.get(&id)
                {
                    Atom::Reference(new_decl.clone())
                } else {
                    atom
                }
            });

            return_expressions.push(return_expression);
        }
        Ok(return_expressions)
    }

    pub fn return_expression(self) -> Expression {
        self.return_expression_submodel.into_single_expression()
    }

    pub fn replace_return_expression(&mut self, new_expr: Expression) {
        let new_expr = match new_expr {
            Expression::And(_, exprs) if exprs.clone().unwrap_list().is_some() => {
                Expression::Root(Metadata::new(), exprs.unwrap_list().unwrap())
            }
            expr => Expression::Root(Metadata::new(), vec![expr]),
        };

        *self.return_expression_submodel.root_mut_unchecked() = new_expr;
    }

    /// Adds a guard to the comprehension. Returns false if the guard does not only reference induction variables.
    pub fn add_induction_guard(&mut self, guard: Expression) -> bool {
        if self.is_induction_guard(&guard) {
            self.generator_submodel.add_constraint(guard);
            true
        } else {
            false
        }
    }

    /// True iff expr only references induction variables.
    pub fn is_induction_guard(&self, expr: &Expression) -> bool {
        is_induction_guard(&(self.induction_vars.clone().into_iter().collect()), expr)
    }
}

impl Display for Comprehension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let generators: String = self
            .generator_submodel
            .symbols()
            .clone()
            .into_iter_local()
            .map(|(name, decl): (Name, DeclarationPtr)| {
                let domain: Domain = decl.domain().unwrap().clone();
                (name, domain)
            })
            .map(|(name, domain): (Name, Domain)| format!("{name}: {domain}"))
            .join(",");

        let guards = self
            .generator_submodel
            .constraints()
            .iter()
            .map(|x| format!("{x}"))
            .join(",");

        let generators_and_guards = itertools::join([generators, guards], ",");

        let expression = &self.return_expression_submodel;
        write!(f, "[{expression} | {generators_and_guards}]")
    }
}

/// A builder for a comprehension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComprehensionBuilder {
    guards: Vec<Expression>,
    // symbol table containing all the generators
    // for now, this is just used during parsing - a new symbol table is created using this when we initialise the comprehension
    // this is not ideal, but i am chucking all this code very soon anyways...
    generator_symboltable: Rc<RefCell<SymbolTable>>,
    return_expr_symboltable: Rc<RefCell<SymbolTable>>,
    induction_variables: BTreeSet<Name>,
}

impl ComprehensionBuilder {
    pub fn new(symbol_table_ptr: Rc<RefCell<SymbolTable>>) -> Self {
        ComprehensionBuilder {
            guards: vec![],
            generator_symboltable: Rc::new(RefCell::new(SymbolTable::with_parent(
                symbol_table_ptr.clone(),
            ))),
            return_expr_symboltable: Rc::new(RefCell::new(SymbolTable::with_parent(
                symbol_table_ptr,
            ))),
            induction_variables: BTreeSet::new(),
        }
    }

    /// The symbol table for the comprehension generators
    pub fn generator_symboltable(&mut self) -> Rc<RefCell<SymbolTable>> {
        Rc::clone(&self.generator_symboltable)
    }

    /// The symbol table for the comprehension return expression
    pub fn return_expr_symboltable(&mut self) -> Rc<RefCell<SymbolTable>> {
        Rc::clone(&self.return_expr_symboltable)
    }

    pub fn guard(mut self, guard: Expression) -> Self {
        self.guards.push(guard);
        self
    }

    pub fn generator(mut self, declaration: DeclarationPtr) -> Self {
        let name = declaration.name().clone();
        let domain = declaration.domain().unwrap().clone();
        assert!(!self.induction_variables.contains(&name));

        self.induction_variables.insert(name.clone());

        // insert into generator symbol table as a variable
        (*self.generator_symboltable)
            .borrow_mut()
            .insert(declaration.clone());

        // insert into return expression symbol table as a given
        (*self.return_expr_symboltable)
            .borrow_mut()
            .insert(DeclarationPtr::new_given(name, domain));

        self
    }

    /// Creates a comprehension with the given return expression.
    ///
    /// If a comprehension kind is not given, comprehension guards containing decision variables
    /// are invalid, and will cause a panic.
    pub fn with_return_value(
        self,
        mut expression: Expression,
        comprehension_kind: Option<ComprehensionKind>,
    ) -> Comprehension {
        let parent_symboltable = self
            .generator_symboltable
            .as_ref()
            .borrow_mut()
            .parent_mut_unchecked()
            .clone()
            .unwrap();
        let mut generator_submodel = SubModel::new(parent_symboltable.clone());
        let mut return_expression_submodel = SubModel::new(parent_symboltable.clone());

        *generator_submodel.symbols_ptr_unchecked_mut() = self.generator_symboltable;
        *return_expression_submodel.symbols_ptr_unchecked_mut() = self.return_expr_symboltable;

        // TODO:also allow guards that reference lettings and givens.

        let induction_variables = self.induction_variables;

        // only guards referencing induction variables can go inside the comprehension
        let (mut induction_guards, mut other_guards): (Vec<_>, Vec<_>) = self
            .guards
            .into_iter()
            .partition(|x| is_induction_guard(&induction_variables, x));

        let induction_variables_2 = induction_variables.clone();
        let generator_symboltable_ptr = generator_submodel.symbols_ptr_unchecked().clone();

        // fix induction guard pointers so that they all point to variables in the generator model
        induction_guards =
            Biplate::<DeclarationPtr>::transform_bi(&induction_guards, &move |decl| {
                if induction_variables_2.contains(&decl.name()) {
                    (*generator_symboltable_ptr)
                        .borrow()
                        .lookup_local(&decl.name())
                        .unwrap()
                } else {
                    decl
                }
            })
            .into_iter()
            .collect_vec();

        let induction_variables_2 = induction_variables.clone();
        let return_expr_symboltable_ptr =
            return_expression_submodel.symbols_ptr_unchecked().clone();

        // fix other guard pointers so that they all point to variables in the return expr model
        other_guards = Biplate::<DeclarationPtr>::transform_bi(&other_guards, &move |decl| {
            if induction_variables_2.contains(&decl.name()) {
                (*return_expr_symboltable_ptr)
                    .borrow()
                    .lookup_local(&decl.name())
                    .unwrap()
            } else {
                decl
            }
        })
        .into_iter()
        .collect_vec();

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

        generator_submodel.add_constraints(induction_guards);

        return_expression_submodel.add_constraint(expression);

        Comprehension {
            return_expression_submodel,
            generator_submodel,
            induction_vars: induction_variables.into_iter().collect_vec(),
        }
    }
}

/// True iff the guard only references induction variables.
fn is_induction_guard(induction_variables: &BTreeSet<Name>, guard: &Expression) -> bool {
    guard
        .universe_bi()
        .iter()
        .all(|x| induction_variables.contains(x))
}
