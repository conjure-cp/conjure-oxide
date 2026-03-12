use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::sync::{Arc, RwLock};

use crate::context::Context;
use crate::{bug, into_matrix_expr};
use derivative::Derivative;
use indexmap::IndexSet;
use itertools::izip;
use parking_lot::{RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uniplate::{Biplate, Tree, Uniplate};

use super::serde::{HasId, ObjId, PtrAsInner};
use super::{
    Atom, CnfClause, DeclarationPtr, Expression, Literal, Metadata, Moo, Name, ReturnType,
    SymbolTable, SymbolTablePtr, Typeable,
    comprehension::Comprehension,
    declaration::DeclarationKind,
    pretty::{
        pretty_clauses, pretty_domain_letting_declaration, pretty_expressions_as_top_level,
        pretty_value_letting_declaration, pretty_variable_declaration,
    },
};

/// An Essence model.
#[serde_as]
#[derive(Derivative, Clone, Debug, Serialize, Deserialize)]
#[derivative(PartialEq, Eq)]
pub struct Model {
    constraints: Moo<Expression>,
    #[serde_as(as = "PtrAsInner")]
    symbols: SymbolTablePtr,
    cnf_clauses: Vec<CnfClause>,

    pub search_order: Option<Vec<Name>>,
    pub dominance: Option<Expression>,

    #[serde(skip, default = "default_context")]
    #[derivative(PartialEq = "ignore")]
    pub context: Arc<RwLock<Context<'static>>>,
}

fn default_context() -> Arc<RwLock<Context<'static>>> {
    Arc::new(RwLock::new(Context::default()))
}

impl Model {
    fn new_empty(symbols: SymbolTablePtr, context: Arc<RwLock<Context<'static>>>) -> Model {
        Model {
            constraints: Moo::new(Expression::Root(Metadata::new(), vec![])),
            symbols,
            cnf_clauses: Vec::new(),
            search_order: None,
            dominance: None,
            context,
        }
    }

    /// Creates a new top-level model from the given context.
    pub fn new(context: Arc<RwLock<Context<'static>>>) -> Model {
        Self::new_empty(SymbolTablePtr::new(), context)
    }

    /// Creates a new model whose symbol table has `parent` as parent scope.
    pub fn new_in_parent_scope(parent: SymbolTablePtr) -> Model {
        Self::new_empty(SymbolTablePtr::with_parent(parent), default_context())
    }

    /// The symbol table for this model as a pointer.
    pub fn symbols_ptr_unchecked(&self) -> &SymbolTablePtr {
        &self.symbols
    }

    /// The symbol table for this model as a mutable pointer.
    pub fn symbols_ptr_unchecked_mut(&mut self) -> &mut SymbolTablePtr {
        &mut self.symbols
    }

    /// The symbol table for this model as a reference.
    pub fn symbols(&self) -> RwLockReadGuard<'_, SymbolTable> {
        self.symbols.read()
    }

    /// The symbol table for this model as a mutable reference.
    pub fn symbols_mut(&mut self) -> RwLockWriteGuard<'_, SymbolTable> {
        self.symbols.write()
    }

    /// The root node of this model.
    pub fn root(&self) -> &Expression {
        &self.constraints
    }

    /// The root node of this model, as a mutable reference.
    ///
    /// The caller is responsible for ensuring that the root node remains an [`Expression::Root`].
    pub fn root_mut_unchecked(&mut self) -> &mut Expression {
        Moo::make_mut(&mut self.constraints)
    }

    /// Replaces the root node with `new_root`, returning the old root node.
    pub fn replace_root(&mut self, new_root: Expression) -> Expression {
        let Expression::Root(_, _) = new_root else {
            tracing::error!(new_root=?new_root,"new_root is not an Expression::Root");
            panic!("new_root is not an Expression::Root");
        };

        std::mem::replace(self.root_mut_unchecked(), new_root)
    }

    /// The top-level constraints in this model.
    pub fn constraints(&self) -> &Vec<Expression> {
        let Expression::Root(_, constraints) = self.constraints.as_ref() else {
            bug!("The top level expression in a model should be Expr::Root");
        };
        constraints
    }

    /// The cnf clauses in this model.
    pub fn clauses(&self) -> &Vec<CnfClause> {
        &self.cnf_clauses
    }

    /// The top-level constraints in this model as a mutable vector.
    pub fn constraints_mut(&mut self) -> &mut Vec<Expression> {
        let Expression::Root(_, constraints) = Moo::make_mut(&mut self.constraints) else {
            bug!("The top level expression in a model should be Expr::Root");
        };

        constraints
    }

    /// The cnf clauses in this model as a mutable vector.
    pub fn clauses_mut(&mut self) -> &mut Vec<CnfClause> {
        &mut self.cnf_clauses
    }

    /// Replaces the top-level constraints with `new_constraints`, returning the old ones.
    pub fn replace_constraints(&mut self, new_constraints: Vec<Expression>) -> Vec<Expression> {
        std::mem::replace(self.constraints_mut(), new_constraints)
    }

    /// Replaces the cnf clauses with `new_clauses`, returning the old ones.
    pub fn replace_clauses(&mut self, new_clauses: Vec<CnfClause>) -> Vec<CnfClause> {
        std::mem::replace(self.clauses_mut(), new_clauses)
    }

    /// Adds a top-level constraint.
    pub fn add_constraint(&mut self, constraint: Expression) {
        self.constraints_mut().push(constraint);
    }

    /// Adds a cnf clause.
    pub fn add_clause(&mut self, clause: CnfClause) {
        self.clauses_mut().push(clause);
    }

    /// Adds top-level constraints.
    pub fn add_constraints(&mut self, constraints: Vec<Expression>) {
        self.constraints_mut().extend(constraints);
    }

    /// Adds cnf clauses.
    pub fn add_clauses(&mut self, clauses: Vec<CnfClause>) {
        self.clauses_mut().extend(clauses);
    }

    /// Adds a new symbol to the symbol table.
    pub fn add_symbol(&mut self, decl: DeclarationPtr) -> Option<()> {
        self.symbols_mut().insert(decl)
    }

    /// Converts the constraints in this model to a single expression suitable for use inside
    /// another expression tree.
    pub fn into_single_expression(self) -> Expression {
        let constraints = self.constraints().clone();
        match constraints.len() {
            0 => Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
            1 => constraints[0].clone(),
            _ => Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints])),
        }
    }

    /// Collects all ObjId values from the model using uniplate traversal.
    pub fn collect_stable_id_mapping(&self) -> HashMap<ObjId, ObjId> {
        fn visit_symbol_table(symbol_table: SymbolTablePtr, id_list: &mut IndexSet<ObjId>) {
            if !id_list.insert(symbol_table.id()) {
                return;
            }

            let table_ref = symbol_table.read();
            table_ref.iter_local().for_each(|(_, decl)| {
                id_list.insert(decl.id());
            });
        }

        let mut id_list: IndexSet<ObjId> = IndexSet::new();

        visit_symbol_table(self.symbols_ptr_unchecked().clone(), &mut id_list);

        let mut exprs: VecDeque<Expression> = self.universe_bi();
        if let Some(dominance) = &self.dominance {
            exprs.push_back(dominance.clone());
        }

        for symbol_table in Biplate::<SymbolTablePtr>::universe_bi(&exprs) {
            visit_symbol_table(symbol_table, &mut id_list);
        }
        for declaration in Biplate::<DeclarationPtr>::universe_bi(&exprs) {
            id_list.insert(declaration.id());
        }

        let mut id_map = HashMap::new();
        for (stable_id, original_id) in id_list.into_iter().enumerate() {
            let type_name = original_id.type_name;
            id_map.insert(
                original_id,
                ObjId {
                    object_id: stable_id as u32,
                    type_name,
                },
            );
        }

        id_map
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new(default_context())
    }
}

impl Typeable for Model {
    fn return_type(&self) -> ReturnType {
        ReturnType::Bool
    }
}

impl Hash for Model {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.constraints.hash(state);
        self.symbols.hash(state);
        self.cnf_clauses.hash(state);
        self.search_order.hash(state);
        self.dominance.hash(state);
    }
}

// At time of writing (03/02/2025), the Uniplate derive macro doesn't like the lifetimes inside
// context, and we do not yet have a way of ignoring this field.
impl Uniplate for Model {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        let self2 = self.clone();
        (Tree::Zero, Box::new(move |_| self2.clone()))
    }
}

impl Biplate<Expression> for Model {
    fn biplate(&self) -> (Tree<Expression>, Box<dyn Fn(Tree<Expression>) -> Self>) {
        let (symtab_tree, symtab_ctx) =
            <SymbolTable as Biplate<Expression>>::biplate(&self.symbols());

        let dom_tree = match &self.dominance {
            Some(expr) => Tree::One(expr.clone()),
            None => Tree::Zero,
        };

        let tree = Tree::Many(VecDeque::from([
            Tree::One(self.root().clone()),
            symtab_tree,
            dom_tree,
        ]));

        let self2 = self.clone();
        let ctx = Box::new(move |x| {
            let Tree::Many(xs) = x else {
                panic!("Expected a tree with three children");
            };
            if xs.len() != 3 {
                panic!("Expected a tree with three children");
            }

            let Tree::One(root) = xs[0].clone() else {
                panic!("Expected root expression tree");
            };

            let symtab = symtab_ctx(xs[1].clone());
            let dominance = match xs[2].clone() {
                Tree::One(expr) => Some(expr),
                Tree::Zero => None,
                _ => panic!("Expected dominance tree"),
            };

            let mut self3 = self2.clone();

            let Expression::Root(_, _) = root else {
                bug!("root expression not root");
            };

            *self3.root_mut_unchecked() = root;
            *self3.symbols_mut() = symtab;
            self3.dominance = dominance;

            self3
        });

        (tree, ctx)
    }
}

impl Biplate<Atom> for Model {
    fn biplate(&self) -> (Tree<Atom>, Box<dyn Fn(Tree<Atom>) -> Self>) {
        let (expression_tree, rebuild_self) = <Model as Biplate<Expression>>::biplate(self);
        let (expression_list, rebuild_expression_tree) = expression_tree.list();

        let (atom_trees, reconstruct_exprs): (VecDeque<_>, VecDeque<_>) = expression_list
            .iter()
            .map(|e| <Expression as Biplate<Atom>>::biplate(e))
            .unzip();

        let tree = Tree::Many(atom_trees);
        let ctx = Box::new(move |atom_tree: Tree<Atom>| {
            let Tree::Many(atoms) = atom_tree else {
                panic!();
            };

            assert_eq!(
                atoms.len(),
                reconstruct_exprs.len(),
                "the number of children should not change when using Biplate"
            );

            let expression_list: VecDeque<Expression> = izip!(atoms, &reconstruct_exprs)
                .map(|(atom, recons)| recons(atom))
                .collect();

            let expression_tree = rebuild_expression_tree(expression_list);
            rebuild_self(expression_tree)
        });

        (tree, ctx)
    }
}

impl Biplate<Comprehension> for Model {
    fn biplate(
        &self,
    ) -> (
        Tree<Comprehension>,
        Box<dyn Fn(Tree<Comprehension>) -> Self>,
    ) {
        let (f1_tree, f1_ctx) = <_ as Biplate<Comprehension>>::biplate(&self.constraints);
        let (f2_tree, f2_ctx) = <SymbolTable as Biplate<Comprehension>>::biplate(&self.symbols());

        let tree = Tree::Many(VecDeque::from([f1_tree, f2_tree]));
        let self2 = self.clone();
        let ctx = Box::new(move |x| {
            let Tree::Many(xs) = x else {
                panic!();
            };

            let root = f1_ctx(xs[0].clone());
            let symtab = f2_ctx(xs[1].clone());

            let mut self3 = self2.clone();

            let Expression::Root(_, _) = &*root else {
                bug!("root expression not root");
            };

            *self3.symbols_mut() = symtab;
            self3.constraints = root;

            self3
        });

        (tree, ctx)
    }
}

impl Display for Model {
    #[allow(clippy::unwrap_used)]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, decl) in self.symbols().clone().into_iter_local() {
            match &decl.kind() as &DeclarationKind {
                DeclarationKind::Find(_) => {
                    writeln!(
                        f,
                        "{}",
                        pretty_variable_declaration(&self.symbols(), &name).unwrap()
                    )?;
                }
                DeclarationKind::ValueLetting(_) | DeclarationKind::TemporaryValueLetting(_) => {
                    writeln!(
                        f,
                        "{}",
                        pretty_value_letting_declaration(&self.symbols(), &name).unwrap()
                    )?;
                }
                DeclarationKind::DomainLetting(_) => {
                    writeln!(
                        f,
                        "{}",
                        pretty_domain_letting_declaration(&self.symbols(), &name).unwrap()
                    )?;
                }
                DeclarationKind::Given(d) => {
                    writeln!(f, "given {name}: {d}")?;
                }
                DeclarationKind::Quantified(inner) => {
                    writeln!(f, "quantified {name}: {}", inner.domain())?;
                }
                DeclarationKind::RecordField(_) => {
                    writeln!(f)?;
                }
            }
        }

        if !self.constraints().is_empty() {
            writeln!(f, "\nsuch that\n")?;
            writeln!(f, "{}", pretty_expressions_as_top_level(self.constraints()))?;
        }

        if !self.clauses().is_empty() {
            writeln!(f, "\nclauses:\n")?;
            writeln!(f, "{}", pretty_clauses(self.clauses()))?;
        }
        Ok(())
    }
}

/// A model that is de/serializable using `serde`.
///
/// To turn this into a rewritable model, it needs to be initialised using
/// [`initialise`](SerdeModel::initialise).
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerdeModel {
    constraints: Moo<Expression>,
    #[serde_as(as = "PtrAsInner")]
    symbols: SymbolTablePtr,
    cnf_clauses: Vec<CnfClause>,
    search_order: Option<Vec<Name>>,
    dominance: Option<Expression>,
}

impl SerdeModel {
    /// Initialises the model for rewriting.
    pub fn initialise(mut self, context: Arc<RwLock<Context<'static>>>) -> Option<Model> {
        let mut tables: HashMap<ObjId, SymbolTablePtr> = HashMap::new();

        // Root model symbol table is always definitive.
        tables.insert(self.symbols.id(), self.symbols.clone());

        let mut exprs: VecDeque<Expression> = self.constraints.universe_bi();
        if let Some(dominance) = &self.dominance {
            exprs.push_back(dominance.clone());
        }

        // Some expressions (e.g. abstract comprehensions) contain additional symbol tables.
        for table in Biplate::<SymbolTablePtr>::universe_bi(&exprs) {
            tables.entry(table.id()).or_insert(table);
        }

        for table in tables.clone().into_values() {
            let mut table_mut = table.write();
            let parent_mut = table_mut.parent_mut_unchecked();

            #[allow(clippy::unwrap_used)]
            if let Some(parent) = parent_mut {
                let parent_id = parent.id();
                *parent = tables.get(&parent_id).unwrap().clone();
            }
        }

        let mut all_declarations: HashMap<ObjId, DeclarationPtr> = HashMap::new();
        for table in tables.values() {
            for (_, decl) in table.read().iter_local() {
                let id = decl.id();
                all_declarations.insert(id, decl.clone());
            }
        }

        self.constraints = self.constraints.transform_bi(&move |decl: DeclarationPtr| {
            let id = decl.id();
            all_declarations
                .get(&id)
                .unwrap_or_else(|| {
                    panic!(
                        "A declaration used in the expression tree should exist in the symbol table. The missing declaration has id {id}."
                    )
                })
                .clone()
        });

        Some(Model {
            constraints: self.constraints,
            symbols: self.symbols,
            cnf_clauses: self.cnf_clauses,
            search_order: self.search_order,
            dominance: self.dominance,
            context,
        })
    }
}

impl From<Model> for SerdeModel {
    fn from(val: Model) -> Self {
        SerdeModel {
            constraints: val.constraints,
            symbols: val.symbols,
            cnf_clauses: val.cnf_clauses,
            search_order: val.search_order,
            dominance: val.dominance,
        }
    }
}

impl Display for SerdeModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let model = Model {
            constraints: self.constraints.clone(),
            symbols: self.symbols.clone(),
            cnf_clauses: self.cnf_clauses.clone(),
            search_order: self.search_order.clone(),
            dominance: self.dominance.clone(),
            context: default_context(),
        };
        std::fmt::Display::fmt(&model, f)
    }
}

impl SerdeModel {
    /// Collects all ObjId values from the model and maps them to stable sequential IDs.
    pub fn collect_stable_id_mapping(&self) -> HashMap<ObjId, ObjId> {
        let model = Model {
            constraints: self.constraints.clone(),
            symbols: self.symbols.clone(),
            cnf_clauses: self.cnf_clauses.clone(),
            search_order: self.search_order.clone(),
            dominance: self.dominance.clone(),
            context: default_context(),
        };
        model.collect_stable_id_mapping()
    }
}
