//! The symbol table.
//!
//! See the item documentation for [`SymbolTable`] for more details.

use crate::bug;
use crate::representation::{Representation, get_repr_rule};
use std::any::TypeId;

use std::collections::BTreeSet;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use super::comprehension::Comprehension;
use super::serde::{AsId, DefaultWithId, HasId, IdPtr, ObjId, PtrAsInner};
use super::{
    DeclarationPtr, DomainPtr, Expression, GroundDomain, Moo, Name, ReturnType, SubModel, Typeable,
};
use itertools::{Itertools as _, izip};
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tracing::trace;
use uniplate::{Biplate, Tree, Uniplate};

/// Global counter of symbol tables.
/// Note that the counter is shared between all threads
/// Thus, when running multiple models in parallel, IDs may
/// be different with every run depending on scheduling order
static SYMBOL_TABLE_ID_COUNTER: AtomicU32 = const { AtomicU32::new(0) };

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SymbolTablePtr
where
    Self: Send + Sync,
{
    inner: Arc<SymbolTablePtrInner>,
}

impl SymbolTablePtr {
    /// Create an empty new [SymbolTable] and return a shared pointer to it
    pub fn new() -> Self {
        Self::new_with_data(SymbolTable::new())
    }

    /// Create an empty new [SymbolTable] with the given parent and return a shared pointer to it
    pub fn with_parent(symbols: SymbolTablePtr) -> Self {
        Self::new_with_data(SymbolTable::with_parent(symbols))
    }

    fn new_with_data(data: SymbolTable) -> Self {
        let object_id = SYMBOL_TABLE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        let id = ObjId {
            object_id,
            type_name: SymbolTablePtr::TYPE_NAME.into(),
        };
        Self::new_with_id_and_data(id, data)
    }

    fn new_with_id_and_data(id: ObjId, data: SymbolTable) -> Self {
        Self {
            inner: Arc::new(SymbolTablePtrInner {
                id,
                value: RwLock::new(data),
            }),
        }
    }

    /// Read the underlying symbol table.
    /// This will block the current thread until a read lock can be acquired.
    ///
    /// # WARNING
    ///
    /// - If the current thread already holds a lock over this table, this may deadlock.
    pub fn read(&self) -> RwLockReadGuard<'_, SymbolTable> {
        self.inner.value.read()
    }

    /// Mutate the underlying symbol table.
    /// This will block the current thread until an exclusive write lock can be acquired.
    ///
    /// # WARNING
    ///
    /// - If the current thread already holds a lock over this table, this may deadlock.
    /// - Trying to acquire any other lock until the write lock is released will cause a deadlock.
    /// - This will mutate the underlying data, which may be shared between other `SymbolTablePtr`s.
    ///   Make sure that this is what you want.
    ///
    /// To create a separate copy of the table, see [SymbolTablePtr::detach].
    ///
    pub fn write(&self) -> RwLockWriteGuard<'_, SymbolTable> {
        self.inner.value.write()
    }

    /// Create a new symbol table with the same contents as this one, but a new ID,
    /// and return a pointer to it.
    pub fn detach(&self) -> Self {
        Self::new_with_data(self.read().clone())
    }
}

impl Default for SymbolTablePtr {
    fn default() -> Self {
        Self::new()
    }
}

impl HasId for SymbolTablePtr {
    const TYPE_NAME: &'static str = "SymbolTable";

    fn id(&self) -> ObjId {
        self.inner.id.clone()
    }
}

impl DefaultWithId for SymbolTablePtr {
    fn default_with_id(id: ObjId) -> Self {
        Self::new_with_id_and_data(id, SymbolTable::default())
    }
}

impl IdPtr for SymbolTablePtr {
    type Data = SymbolTable;

    fn get_data(&self) -> Self::Data {
        self.read().clone()
    }

    fn with_id_and_data(id: ObjId, data: Self::Data) -> Self {
        Self::new_with_id_and_data(id, data)
    }
}

// TODO: this code is almost exactly copied from [DeclarationPtr].
//       It should be possible to eliminate the duplication...
//       Perhaps by merging SymbolTablePtr and DeclarationPtr together?
//       (Alternatively, a macro?)

impl Uniplate for SymbolTablePtr {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        let symtab = self.read();
        let (tree, recons) = Biplate::<SymbolTablePtr>::biplate(&symtab as &SymbolTable);

        let self2 = self.clone();
        (
            tree,
            Box::new(move |x| {
                let self3 = self2.clone();
                *(self3.write()) = recons(x);
                self3
            }),
        )
    }
}

impl<To> Biplate<To> for SymbolTablePtr
where
    SymbolTable: Biplate<To>,
    To: Uniplate,
{
    fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>) {
        if TypeId::of::<To>() == TypeId::of::<Self>() {
            unsafe {
                let self_as_to = std::mem::transmute::<&Self, &To>(self).clone();
                (
                    Tree::One(self_as_to),
                    Box::new(move |x| {
                        let Tree::One(x) = x else { panic!() };

                        let x_as_self = std::mem::transmute::<&To, &Self>(&x);
                        x_as_self.clone()
                    }),
                )
            }
        } else {
            // call biplate on the enclosed declaration
            let decl = self.read();
            let (tree, recons) = Biplate::<To>::biplate(&decl as &SymbolTable);

            let self2 = self.clone();
            (
                tree,
                Box::new(move |x| {
                    let self3 = self2.clone();
                    *(self3.write()) = recons(x);
                    self3
                }),
            )
        }
    }
}

#[derive(Debug)]
struct SymbolTablePtrInner {
    id: ObjId,
    value: RwLock<SymbolTable>,
}

impl Hash for SymbolTablePtrInner {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PartialEq for SymbolTablePtrInner {
    fn eq(&self, other: &Self) -> bool {
        self.value.read().eq(&other.value.read())
    }
}

impl Eq for SymbolTablePtrInner {}

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
#[serde_as]
#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct SymbolTable {
    #[serde_as(as = "Vec<(_,PtrAsInner)>")]
    table: BTreeMap<Name, DeclarationPtr>,

    #[serde_as(as = "Option<AsId>")]
    parent: Option<SymbolTablePtr>,

    next_machine_name: i32,
}

impl SymbolTable {
    /// Creates an empty symbol table.
    pub fn new() -> SymbolTable {
        SymbolTable::new_inner(None)
    }

    /// Creates an empty symbol table with the given parent.
    pub fn with_parent(parent: SymbolTablePtr) -> SymbolTable {
        SymbolTable::new_inner(Some(parent))
    }

    fn new_inner(parent: Option<SymbolTablePtr>) -> SymbolTable {
        let id = SYMBOL_TABLE_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
        trace!(
            "new symbol table: id = {id}  parent_id = {}",
            parent
                .as_ref()
                .map(|x| x.id().to_string())
                .unwrap_or(String::from("none"))
        );
        SymbolTable {
            table: BTreeMap::new(),
            next_machine_name: 0,
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
                .and_then(|parent| parent.read().lookup(name))
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
        self.lookup(name).map(|x| x.return_type())
    }

    /// Looks up the return type for name if has one and is in the local scope.
    pub fn return_type_local(&self, name: &Name) -> Option<ReturnType> {
        self.lookup_local(name).map(|x| x.return_type())
    }

    /// Looks up the domain of name if it has one and is in scope.
    ///
    /// This method can return domain references: if a ground domain is always required, use
    /// [`SymbolTable::resolve_domain`].
    pub fn domain(&self, name: &Name) -> Option<DomainPtr> {
        if let Name::WithRepresentation(name, _) = name {
            self.lookup(name)?.domain()
        } else {
            self.lookup(name)?.domain()
        }
    }

    /// Looks up the domain of name, resolving domain references to ground domains.
    ///
    /// See [`SymbolTable::domain`].
    pub fn resolve_domain(&self, name: &Name) -> Option<Moo<GroundDomain>> {
        self.domain(name)?.resolve()
    }

    /// Iterates over entries in the LOCAL symbol table.
    pub fn into_iter_local(self) -> impl Iterator<Item = (Name, DeclarationPtr)> {
        self.table.into_iter()
    }

    /// Iterates over entries in the LOCAL symbol table, by reference.
    pub fn iter_local(&self) -> impl Iterator<Item = (&Name, &DeclarationPtr)> {
        self.table.iter()
    }

    /// Extends the symbol table with the given symbol table, updating the gensym counter if
    /// necessary.
    pub fn extend(&mut self, other: SymbolTable) {
        if other.table.keys().count() > self.table.keys().count() {
            let new_vars = other.table.keys().collect::<BTreeSet<_>>();
            let old_vars = self.table.keys().collect::<BTreeSet<_>>();

            for added_var in new_vars.difference(&old_vars) {
                let next_var = &mut self.next_machine_name;
                if let Name::Machine(m) = *added_var
                    && *m >= *next_var
                {
                    *next_var = *m + 1;
                }
            }
        }

        self.table.extend(other.table);
    }

    /// Creates a new variable in this symbol table with a unique name, and returns its
    /// declaration.
    pub fn gensym(&mut self, domain: &DomainPtr) -> DeclarationPtr {
        let num = self.next_machine_name;
        self.next_machine_name += 1;
        let decl = DeclarationPtr::new_var(Name::Machine(num), domain.clone());
        self.insert(decl.clone());
        decl
    }

    /// Gets the parent of this symbol table as a mutable reference.
    ///
    /// This function provides no sanity checks.
    pub fn parent_mut_unchecked(&mut self) -> &mut Option<SymbolTablePtr> {
        &mut self.parent
    }

    /// Gets the parent of this symbol table.
    pub fn parent(&self) -> &Option<SymbolTablePtr> {
        &self.parent
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

        if let Some(var) = decl.as_var()
            && let Some(existing_reprs) = var
                .representations
                .iter()
                .find(|x| &x.iter().map(|r| r.repr_name()).collect_vec()[..] == representation)
                .cloned()
        {
            return Some(existing_reprs); // Found: return early
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

    type IntoIter = SymbolTableIter;

    /// Iterates over symbol table entries in scope.
    fn into_iter(self) -> Self::IntoIter {
        SymbolTableIter {
            inner: self.table.into_iter(),
            parent: self.parent,
        }
    }
}

/// Iterator over all symbol table entries in scope.
pub struct SymbolTableIter {
    // iterator over the current scopes' btreemap
    inner: std::collections::btree_map::IntoIter<Name, DeclarationPtr>,

    // the parent scope
    parent: Option<SymbolTablePtr>,
}

impl Iterator for SymbolTableIter {
    type Item = (Name, DeclarationPtr);

    fn next(&mut self) -> Option<Self::Item> {
        let mut val = self.inner.next();

        // Go up the tree until we find a parent symbol table with declarations to iterate over.
        //
        // Note that the parent symbol table may be empty - this is why this is a loop!
        while val.is_none() {
            let parent = self.parent.clone()?;

            let guard = parent.read();
            self.inner = guard.table.clone().into_iter();
            self.parent.clone_from(&guard.parent);

            val = self.inner.next();
        }

        val
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new_inner(None)
    }
}

// TODO: if we could override `Uniplate` impl but still derive `Biplate` instances,
//       we could remove some of this manual code
impl Uniplate for SymbolTable {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        // do not recurse up parents, that would be weird?
        let self2 = self.clone();
        (Tree::Zero, Box::new(move |_| self2.clone()))
    }
}

impl Biplate<SymbolTablePtr> for SymbolTable {
    fn biplate(
        &self,
    ) -> (
        Tree<SymbolTablePtr>,
        Box<dyn Fn(Tree<SymbolTablePtr>) -> Self>,
    ) {
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
