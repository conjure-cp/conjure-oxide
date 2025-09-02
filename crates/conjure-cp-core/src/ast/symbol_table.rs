//! The symbol table.
//!
//! See the item documentation for [`SymbolTable`] for more details.

use crate::bug;
use crate::representation::{Representation, get_repr_rule};

use super::comprehension::Comprehension;
use super::serde::RcRefCellAsId;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::declaration::{DeclarationPtr, serde::DeclarationPtrFull};
use super::serde::{DefaultWithId, HasId, ObjId};
use super::types::Typeable;
use itertools::{Itertools as _, izip};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use uniplate::Tree;
use uniplate::{Biplate, Uniplate};

use super::name::Name;
use super::{Domain, Expression, ReturnType, SubModel};
use derivative::Derivative;

// Count symbol tables per thread / model.
//
// We run tests in parallel and having the id's be per thread keeps them more deterministic in the
// JSON output. If this were not thread local, ids would be given to symbol tables differently in
// each test run (depending on how the threads were scheduled). These id changes would result in
// all the generated tests "changing" each time `ACCEPT=true cargo test` is ran.
//
// SAFETY: Symbol tables use Rc<RefCell<<>>, so a model is not thread-safe anyways.
thread_local! {
static ID_COUNTER: AtomicU32 = const { AtomicU32::new(0) };
}

/// The global symbol table, mapping names to their definitions.
///
/// Names in the symbol table are unique, including between different types of object stored in the
/// symbol table. For example, you cannot have a letting and decision variable with the same name.
///
/// # Symbol Kinds
///
/// The symbol table tracks the following types of symbol:
///
/// ## Decision Variables
///
/// ```text
/// find NAME: DOMAIN
/// ```
///
/// See [`DecisionVariable`](super::DecisionVariable).
///
/// ## Lettings
///
/// Lettings define constants, of which there are two types:
///
///   + **Constant values**: `letting val be A`, where A is an [`Expression`].
///
///     A can be any integer, boolean, or matrix expression.
///     A can include references to other lettings, model parameters, and, unlike Savile Row,
///     decision variables.
///
///   + **Constant domains**: `letting Domain be domain D`, where D is a [`Domain`].
///
///     D can include references to other lettings and model parameters, and, unlike Savile Row,
///     decision variables.
///
/// Unless otherwise stated, these follow the semantics specified in section 2.2.2 of the Savile
/// Row manual (version 1.9.1 at time of writing).
#[derive(Derivative)]
#[derivative(PartialEq)]
#[derive(Debug, Eq)]
#[serde_as]
#[derive(Serialize, Deserialize)]
pub struct SymbolTable {
    #[serde_as(as = "Vec<(_,DeclarationPtrFull)>")]
    table: BTreeMap<Name, DeclarationPtr>,

    /// A unique id for this symbol table, for serialisation and debugging.
    #[derivative(PartialEq = "ignore")] // eq by value not id.
    id: ObjId,

    #[serde_as(as = "Option<RcRefCellAsId>")]
    parent: Option<Rc<RefCell<SymbolTable>>>,

    next_machine_name: RefCell<i32>,
}

impl SymbolTable {
    /// Creates an empty symbol table.
    pub fn new() -> SymbolTable {
        SymbolTable::new_inner(None)
    }

    /// Creates an empty symbol table with the given parent.
    pub fn with_parent(parent: Rc<RefCell<SymbolTable>>) -> SymbolTable {
        SymbolTable::new_inner(Some(parent))
    }

    fn new_inner(parent: Option<Rc<RefCell<SymbolTable>>>) -> SymbolTable {
        let id = ID_COUNTER.with(|x| x.fetch_add(1, Ordering::Relaxed));
        SymbolTable {
            id,
            table: BTreeMap::new(),
            next_machine_name: RefCell::new(0),
            parent,
        }
    }

    /// Looks up the declaration with the given name in the current scope only.
    ///
    /// Returns `None` if there is no declaration with that name in the current scope.
    pub fn lookup_local(&self, name: &Name) -> Option<DeclarationPtr> {
        self.table.get(name).cloned()
    }

    /// Looks up the declaration with the given name, checking all enclosing scopes.
    ///
    /// Returns `None` if there is no declaration with that name in scope.
    pub fn lookup(&self, name: &Name) -> Option<DeclarationPtr> {
        self.lookup_local(name).or_else(|| {
            self.parent
                .as_ref()
                .and_then(|parent| (*parent).borrow().lookup(name))
        })
    }

    /// Inserts a declaration into the symbol table.
    ///
    /// Returns `None` if there is already a symbol with this name in the local scope.
    pub fn insert(&mut self, declaration: DeclarationPtr) -> Option<()> {
        let name = declaration.name().clone();
        if let Entry::Vacant(e) = self.table.entry(name) {
            e.insert(declaration);
            Some(())
        } else {
            None
        }
    }

    /// Updates or adds a declaration in the immediate local scope.
    pub fn update_insert(&mut self, declaration: DeclarationPtr) {
        let name = declaration.name().clone();
        self.table.insert(name, declaration);
    }

    /// Looks up the return type for name if it has one and is in scope.
    pub fn return_type(&self, name: &Name) -> Option<ReturnType> {
        self.lookup(name).and_then(|x| x.return_type())
    }

    /// Looks up the return type for name if has one and is in the local scope.
    pub fn return_type_local(&self, name: &Name) -> Option<ReturnType> {
        self.lookup_local(name).and_then(|x| x.return_type())
    }

    /// Looks up the domain of name if it has one and is in scope.
    ///
    /// This method can return domain references: if a ground domain is always required, use
    /// [`SymbolTable::resolve_domain`].
    pub fn domain(&self, name: &Name) -> Option<Domain> {
        // TODO: do not clone here: in the future, we might want to wrap all domains in Rc's to get
        // clone-on-write behaviour (saving memory in scenarios such as matrix decomposition where
        // a lot of the domains would be the same).

        if let Name::WithRepresentation(name, _) = name {
            self.lookup(name)?.domain()
        } else {
            self.lookup(name)?.domain()
        }
    }

    /// Looks up the domain of name, resolving domain references to ground domains.
    ///
    /// See [`SymbolTable::domain`].
    pub fn resolve_domain(&self, name: &Name) -> Option<Domain> {
        self.domain(name).map(|domain| domain.resolve(self))
    }

    /// Iterates over entries in the local symbol table only.
    pub fn into_iter_local(self) -> LocalIntoIter {
        LocalIntoIter {
            inner: self.table.into_iter(),
        }
    }

    /// Extends the symbol table with the given symbol table, updating the gensym counter if
    /// necessary.
    pub fn extend(&mut self, other: SymbolTable) {
        if other.table.keys().count() > self.table.keys().count() {
            let new_vars = other.table.keys().collect::<BTreeSet<_>>();
            let old_vars = self.table.keys().collect::<BTreeSet<_>>();

            for added_var in new_vars.difference(&old_vars) {
                let mut next_var = self.next_machine_name.borrow_mut();
                if let Name::Machine(m) = *added_var {
                    if *m >= *next_var {
                        *next_var = *m + 1;
                    }
                }
            }
        }

        self.table.extend(other.table);
    }

    /// Creates a new variable in this symbol table with a unique name, and returns its
    /// declaration.
    pub fn gensym(&mut self, domain: &Domain) -> DeclarationPtr {
        let num = *self.next_machine_name.borrow();
        *(self.next_machine_name.borrow_mut()) += 1;
        let decl = DeclarationPtr::new_var(Name::Machine(num), domain.clone());
        self.insert(decl.clone());
        decl
    }

    /// Gets the parent of this symbol table as a mutable reference.
    ///
    /// This function provides no sanity checks.
    pub fn parent_mut_unchecked(&mut self) -> &mut Option<Rc<RefCell<SymbolTable>>> {
        &mut self.parent
    }

    /// Gets the representation `representation` for `name`.
    ///
    /// # Returns
    ///
    /// + `None` if `name` does not exist, is not a decision variable, or does not have that representation.
    pub fn get_representation(
        &self,
        name: &Name,
        representation: &[&str],
    ) -> Option<Vec<Box<dyn Representation>>> {
        // TODO: move representation stuff to declaration / variable to avoid cloning? (we have to
        // move inside of an rc here, so cannot return borrows)
        //
        // Also would prevent constant "does exist" "is var" checks.
        //
        // The reason it is not there now is because I'm getting serde issues...
        //
        // Also might run into issues putting get_or_add into declaration/variable, as that
        // requires us to mutably borrow both the symbol table, and the variable inside the symbol
        // table..

        let decl = self.lookup(name)?;
        let var = &decl.as_var()?;

        var.representations
            .iter()
            .find(|x| &x.iter().map(|r| r.repr_name()).collect_vec()[..] == representation)
            .cloned()
    }

    /// Gets all initialised representations for `name`.
    ///
    /// # Returns
    ///
    /// + `None` if `name` does not exist, or is not a decision variable.
    pub fn representations_for(&self, name: &Name) -> Option<Vec<Vec<Box<dyn Representation>>>> {
        let decl = self.lookup(name)?;
        decl.as_var().map(|x| x.representations.clone())
    }

    /// Gets the representation `representation` for `name`, creating it if it does not exist.
    ///
    /// If the representation does not exist, this method initialises the representation in this
    /// symbol table, adding the representation to `name`, and the declarations for the represented
    /// variables to the symbol table.
    ///
    /// # Usage
    ///
    /// Representations for variable references should be selected and created by the
    /// `select_representation` rule. Therefore, this method should not be used in other rules.
    /// Consider using [`get_representation`](`SymbolTable::get_representation`) instead.
    ///
    /// # Returns
    ///
    /// + `None` if `name` does not exist, is not a decision variable, or cannot be given that
    ///   representation.
    pub fn get_or_add_representation(
        &mut self,
        name: &Name,
        representation: &[&str],
    ) -> Option<Vec<Box<dyn Representation>>> {
        // Lookup the declaration reference
        let mut decl = self.lookup(name)?;

        if let Some(var) = decl.as_var() {
            if let Some(existing_reprs) = var
                .representations
                .iter()
                .find(|x| &x.iter().map(|r| r.repr_name()).collect_vec()[..] == representation)
                .cloned()
            {
                return Some(existing_reprs); // Found: return early
            }
        }
        // Representation not found

        // TODO: nested representations logic...
        if representation.len() != 1 {
            bug!("nested representations not implemented")
        }
        let repr_name_str = representation[0];
        let repr_init_fn = get_repr_rule(repr_name_str)?;

        let reprs = vec![repr_init_fn(name, self)?];

        // Get mutable access to the variable part
        let mut var = decl.as_var_mut()?;

        for repr_instance in &reprs {
            repr_instance
                .declaration_down()
                .ok()?
                .into_iter()
                .for_each(|x| self.update_insert(x));
        }

        var.representations.push(reprs.clone());

        Some(reprs)
    }
}

impl IntoIterator for SymbolTable {
    type Item = (Name, DeclarationPtr);

    type IntoIter = IntoIter;

    /// Iterates over symbol table entries in scope.
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.table.into_iter(),
            parent: self.parent,
        }
    }
}

/// Iterator over symbol table entries in the current scope only.
pub struct LocalIntoIter {
    // iterator over the btreemap
    inner: std::collections::btree_map::IntoIter<Name, DeclarationPtr>,
}

impl Iterator for LocalIntoIter {
    type Item = (Name, DeclarationPtr);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Iterator over all symbol table entries in scope.
pub struct IntoIter {
    // iterator over the current scopes' btreemap
    inner: std::collections::btree_map::IntoIter<Name, DeclarationPtr>,

    // the parent scope
    parent: Option<Rc<RefCell<SymbolTable>>>,
}

impl Iterator for IntoIter {
    type Item = (Name, DeclarationPtr);

    fn next(&mut self) -> Option<Self::Item> {
        let mut val = self.inner.next();

        // Go up the tree until we find a parent symbol table with declarations to iterate over.
        //
        // Note that the parent symbol table may be empty - this is why this is a loop!
        while val.is_none() {
            let parent = self.parent.clone()?;
            let parent_ref = (*parent).borrow();
            self.parent.clone_from(&parent_ref.parent);
            self.inner = parent_ref.table.clone().into_iter();

            val = self.inner.next();
        }

        val
    }
}

impl HasId for SymbolTable {
    fn id(&self) -> ObjId {
        self.id
    }
}

impl DefaultWithId for SymbolTable {
    fn default_with_id(id: ObjId) -> Self {
        Self {
            table: BTreeMap::new(),
            id,
            parent: None,
            next_machine_name: RefCell::new(0),
        }
    }
}

impl Clone for SymbolTable {
    fn clone(&self) -> Self {
        Self {
            table: self.table.clone(),
            id: ID_COUNTER.with(|x| x.fetch_add(1, Ordering::Relaxed)),
            parent: self.parent.clone(),
            next_machine_name: self.next_machine_name.clone(),
        }
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new_inner(None)
    }
}

impl Uniplate for SymbolTable {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        // do not recurse up parents, that would be weird?
        let self2 = self.clone();
        (Tree::Zero, Box::new(move |_| self2.clone()))
    }
}

impl Biplate<Expression> for SymbolTable {
    fn biplate(&self) -> (Tree<Expression>, Box<dyn Fn(Tree<Expression>) -> Self>) {
        let (child_trees, ctxs): (VecDeque<_>, Vec<_>) = self
            .table
            .values()
            .map(Biplate::<Expression>::biplate)
            .unzip();

        let tree = Tree::Many(child_trees);

        let self2 = self.clone();
        let ctx = Box::new(move |tree| {
            let Tree::Many(exprs) = tree else {
                panic!("unexpected children structure");
            };

            let mut self3 = self2.clone();
            let self3_iter = self3.table.iter_mut();
            for (ctx, tree, (_, decl)) in izip!(&ctxs, exprs, self3_iter) {
                // update declaration inside the pointer instead of creating a new one, so all
                // things referencing this keep referencing this.
                *decl = ctx(tree)
            }

            self3
        });

        (tree, ctx)
    }
}

impl Biplate<Comprehension> for SymbolTable {
    fn biplate(
        &self,
    ) -> (
        Tree<Comprehension>,
        Box<dyn Fn(Tree<Comprehension>) -> Self>,
    ) {
        let (expr_tree, expr_ctx) = <SymbolTable as Biplate<Expression>>::biplate(self);

        let (exprs, recons_expr_tree) = expr_tree.list();

        let (comprehension_tree, comprehension_ctx) =
            <VecDeque<Expression> as Biplate<Comprehension>>::biplate(&exprs);

        let ctx = Box::new(move |x| {
            // 1. turn comprehension tree into a list of expressions
            let exprs = comprehension_ctx(x);

            // 2. turn list of expressions into an expression tree
            let expr_tree = recons_expr_tree(exprs);

            // 3. turn expression tree into a symbol table
            expr_ctx(expr_tree)
        });

        (comprehension_tree, ctx)
    }
}

impl Biplate<SubModel> for SymbolTable {
    // walk into expressions
    fn biplate(&self) -> (Tree<SubModel>, Box<dyn Fn(Tree<SubModel>) -> Self>) {
        let (expr_tree, expr_ctx) = <SymbolTable as Biplate<Expression>>::biplate(self);

        let (exprs, recons_expr_tree) = expr_tree.list();

        let (submodel_tree, submodel_ctx) =
            <VecDeque<Expression> as Biplate<SubModel>>::biplate(&exprs);

        let ctx = Box::new(move |x| {
            // 1. turn submodel tree into a list of expressions
            let exprs = submodel_ctx(x);

            // 2. turn list of expressions into an expression tree
            let expr_tree = recons_expr_tree(exprs);

            // 3. turn expression tree into a symbol table
            expr_ctx(expr_tree)
        });
        (submodel_tree, ctx)
    }
}
