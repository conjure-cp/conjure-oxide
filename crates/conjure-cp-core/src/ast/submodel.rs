use super::{
    Atom, DeclarationPtr, Literal, Moo,
    comprehension::Comprehension,
    declaration::DeclarationKind,
    pretty::{
        pretty_domain_letting_declaration, pretty_expressions_as_top_level,
        pretty_value_letting_declaration, pretty_variable_declaration,
    },
    serde::RcRefCellAsInner,
};
use itertools::izip;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uniplate::{Biplate, Tree, Uniplate};

use crate::{ast::Metadata, bug, into_matrix_expr};
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::VecDeque,
    fmt::Display,
    rc::Rc,
};

use super::{Expression, ReturnType, SymbolTable, types::Typeable};

/// A sub-model, representing a lexical scope in the model.
///
/// Each sub-model contains a symbol table representing its scope, as well as a expression tree.
///
/// The expression tree is formed of a root node of type [`Expression::Root`], which contains a
/// vector of top-level constraints.
#[serde_as]
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct SubModel {
    constraints: Moo<Expression>,
    #[serde_as(as = "RcRefCellAsInner")]
    symbols: Rc<RefCell<SymbolTable>>,
}

impl SubModel {
    /// Creates a new [`Submodel`] with no parent scope.
    ///
    /// Top level models are represented as [`Model`](super::model): consider using
    /// [`Model::new`](super::Model::new) instead.
    #[doc(hidden)]
    pub(super) fn new_top_level() -> SubModel {
        SubModel {
            constraints: Moo::new(Expression::Root(Metadata::new(), vec![])),
            symbols: Rc::new(RefCell::new(SymbolTable::new())),
        }
    }

    /// Creates a new [`Submodel`] as a child scope of `parent`.
    ///
    /// `parent` should be the symbol table of the containing scope of this sub-model.
    pub fn new(parent: Rc<RefCell<SymbolTable>>) -> SubModel {
        SubModel {
            constraints: Moo::new(Expression::Root(Metadata::new(), vec![])),
            symbols: Rc::new(RefCell::new(SymbolTable::with_parent(parent))),
        }
    }

    /// The symbol table for this sub-model as a pointer.
    ///
    /// The caller should only mutate the returned symbol table if this method was called on a
    /// mutable model.
    pub fn symbols_ptr_unchecked(&self) -> &Rc<RefCell<SymbolTable>> {
        &self.symbols
    }

    /// The symbol table for this sub-model as a mutable pointer.
    ///
    /// The caller should only mutate the returned symbol table if this method was called on a
    /// mutable model.
    pub fn symbols_ptr_unchecked_mut(&mut self) -> &mut Rc<RefCell<SymbolTable>> {
        &mut self.symbols
    }

    /// The symbol table for this sub-model as a reference.
    pub fn symbols(&self) -> Ref<SymbolTable> {
        (*self.symbols).borrow()
    }

    /// The symbol table for this sub-model as a mutable reference.
    pub fn symbols_mut(&mut self) -> RefMut<SymbolTable> {
        (*self.symbols).borrow_mut()
    }

    /// The root node of this sub-model.
    ///
    /// The root node is an [`Expression::Root`] containing a vector of the top level constraints
    /// in this sub-model.
    pub fn root(&self) -> &Expression {
        &self.constraints
    }

    /// The root node of this sub-model, as a mutable reference.
    ///
    /// The caller is responsible for ensuring that the root node remains an [`Expression::Root`].
    ///
    pub fn root_mut_unchecked(&mut self) -> &mut Expression {
        Moo::make_mut(&mut self.constraints)
    }

    /// Replaces the root node with `new_root`, returning the old root node.
    ///
    /// # Panics
    ///
    /// - If `new_root` is not an [`Expression::Root`].
    pub fn replace_root(&mut self, new_root: Expression) -> Expression {
        let Expression::Root(_, _) = new_root else {
            tracing::error!(new_root=?new_root,"new_root is not an Expression::root");
            panic!("new_root is not an Expression::Root");
        };

        // INVARIANT: already checked that `new_root` is an [`Expression::Root`]
        std::mem::replace(self.root_mut_unchecked(), new_root)
    }

    /// The top-level constraints in this sub-model.
    pub fn constraints(&self) -> &Vec<Expression> {
        let Expression::Root(_, constraints) = self.constraints.as_ref() else {
            bug!("The top level expression in a submodel should be Expr::Root");
        };

        constraints
    }

    /// The top-level constraints in this sub-model as a mutable vector.
    pub fn constraints_mut(&mut self) -> &mut Vec<Expression> {
        let Expression::Root(_, constraints) = Moo::make_mut(&mut self.constraints) else {
            bug!("The top level expression in a submodel should be Expr::Root");
        };

        constraints
    }

    /// Replaces the top-level constraints with `new_constraints`, returning the old ones.
    pub fn replace_constraints(&mut self, new_constraints: Vec<Expression>) -> Vec<Expression> {
        std::mem::replace(self.constraints_mut(), new_constraints)
    }

    /// Adds a top-level constraint.
    pub fn add_constraint(&mut self, constraint: Expression) {
        self.constraints_mut().push(constraint);
    }

    /// Adds top-level constraints.
    pub fn add_constraints(&mut self, constraints: Vec<Expression>) {
        self.constraints_mut().extend(constraints);
    }

    /// Adds a new symbol to the symbol table
    /// (Wrapper over `SymbolTable.insert`)
    pub fn add_symbol(&mut self, decl: DeclarationPtr) -> Option<()> {
        self.symbols_mut().insert(decl)
    }

    /// Converts the constraints in this submodel to a single expression suitable for use inside
    /// another expression tree.
    ///
    /// * If this submodel has no constraints, true is returned.
    /// * If this submodel has a single constraint, that constraint is returned.
    /// * If this submodel has multiple constraints, they are returned as an `and` constraint.
    pub fn into_single_expression(self) -> Expression {
        let constraints = self.constraints().clone();
        match constraints.len() {
            0 => Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
            1 => constraints[0].clone(),
            _ => Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints])),
        }
    }
}

impl Typeable for SubModel {
    fn return_type(&self) -> Option<super::ReturnType> {
        Some(ReturnType::Bool)
    }
}

impl Display for SubModel {
    #[allow(clippy::unwrap_used)] // [rustdocs]: should only fail iff the formatter fails
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (name, decl) in self.symbols().clone().into_iter_local() {
            match &decl.kind() as &DeclarationKind {
                DeclarationKind::DecisionVariable(_) => {
                    writeln!(
                        f,
                        "{}",
                        pretty_variable_declaration(&self.symbols(), &name).unwrap()
                    )?;
                }
                DeclarationKind::ValueLetting(_) => {
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

                DeclarationKind::RecordField(_) => {
                    // Do not print a record field as it is an internal type
                    writeln!(f)?;
                    // TODO: is this correct?
                }
            }
        }

        writeln!(f, "\nsuch that\n")?;

        writeln!(f, "{}", pretty_expressions_as_top_level(self.constraints()))?;

        Ok(())
    }
}

// Using manual implementations of Uniplate so that we can update the old Rc<RefCell<<>>> with the
// new value instead of creating a new one. This will keep the parent pointers in sync.
//
// I considered adding Rc RefCell shared-mutability to Uniplate, but I think this is unsound in
// generality: e.g. two pointers to the same object are in our tree, and both get modified in
// different ways.
//
// Shared mutability is probably fine here, as we only have one pointer to each symbol table
// reachable via Uniplate, the one in its Submodel. The SymbolTable implementation doesn't return
// or traverse through the parent pointers.
//
// -- nd60

impl Uniplate for SubModel {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        // Look inside constraint tree and symbol tables.

        let (expr_tree, expr_ctx) = <Expression as Biplate<SubModel>>::biplate(self.root());

        let symtab_ptr = self.symbols();
        let (symtab_tree, symtab_ctx) = <SymbolTable as Biplate<SubModel>>::biplate(&symtab_ptr);

        let tree = Tree::Many(VecDeque::from([expr_tree, symtab_tree]));

        let self2 = self.clone();
        let ctx = Box::new(move |x| {
            let Tree::Many(xs) = x else {
                panic!();
            };

            let root = expr_ctx(xs[0].clone());
            let symtab = symtab_ctx(xs[1].clone());

            let mut self3 = self2.clone();

            let Expression::Root(_, _) = root else {
                bug!("root expression not root");
            };

            *self3.root_mut_unchecked() = root;

            *self3.symbols_mut() = symtab;

            self3
        });

        (tree, ctx)
    }
}

impl Biplate<Expression> for SubModel {
    fn biplate(&self) -> (Tree<Expression>, Box<dyn Fn(Tree<Expression>) -> Self>) {
        // Return constraints tree and look inside symbol table.
        let symtab_ptr = self.symbols();
        let (symtab_tree, symtab_ctx) = <SymbolTable as Biplate<Expression>>::biplate(&symtab_ptr);

        let tree = Tree::Many(VecDeque::from([
            Tree::One(self.root().clone()),
            symtab_tree,
        ]));

        let self2 = self.clone();
        let ctx = Box::new(move |x| {
            let Tree::Many(xs) = x else {
                panic!();
            };

            let Tree::One(root) = xs[0].clone() else {
                panic!();
            };

            let symtab = symtab_ctx(xs[1].clone());

            let mut self3 = self2.clone();

            let Expression::Root(_, _) = root else {
                bug!("root expression not root");
            };

            *self3.root_mut_unchecked() = root;

            *self3.symbols_mut() = symtab;

            self3
        });

        (tree, ctx)
    }
}

impl Biplate<SubModel> for SubModel {
    fn biplate(&self) -> (Tree<SubModel>, Box<dyn Fn(Tree<SubModel>) -> Self>) {
        (
            Tree::One(self.clone()),
            Box::new(move |x| {
                let Tree::One(x) = x else {
                    panic!();
                };
                x
            }),
        )
    }
}

impl Biplate<Atom> for SubModel {
    fn biplate(&self) -> (Tree<Atom>, Box<dyn Fn(Tree<Atom>) -> Self>) {
        // As atoms are only found in expressions, create a tree of atoms by
        //
        //  1. getting the expression tree
        //  2. Turning that into a list
        //  3. For each expression in the list, use Biplate<Atom> to turn it into an atom
        //
        //  Reconstruction works in reverse.

        let (expression_tree, rebuild_self) = <SubModel as Biplate<Expression>>::biplate(self);
        let (expression_list, rebuild_expression_tree) = expression_tree.list();

        // Let the atom tree be a Tree::Many where each element is the result of running Biplate<Atom>::biplate on an expression in the expression list.
        let (atom_trees, reconstruct_exprs): (VecDeque<_>, VecDeque<_>) = expression_list
            .iter()
            .map(|e| <Expression as Biplate<Atom>>::biplate(e))
            .unzip();

        let tree = Tree::Many(atom_trees);
        let ctx = Box::new(move |atom_tree: Tree<Atom>| {
            // 1. reconstruct expression_list from the atom tree

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

            // 2. reconstruct expression_tree from expression_list
            let expression_tree = rebuild_expression_tree(expression_list);

            // 3. reconstruct submodel from expression_tree
            rebuild_self(expression_tree)
        });

        (tree, ctx)
    }
}

impl Biplate<Comprehension> for SubModel {
    fn biplate(
        &self,
    ) -> (
        Tree<Comprehension>,
        Box<dyn Fn(Tree<Comprehension>) -> Self>,
    ) {
        let (f1_tree, f1_ctx) = <_ as Biplate<Comprehension>>::biplate(&self.constraints);
        let (f2_tree, f2_ctx) =
            <SymbolTable as Biplate<Comprehension>>::biplate(&self.symbols.borrow());

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
