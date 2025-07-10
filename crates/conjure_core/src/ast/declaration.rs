#![allow(deprecated)]
use std::any::TypeId;
// allow use of Declaration in this file, and nowhere else
use std::cell::{Cell, Ref, RefCell, RefMut};
use std::fmt::{Debug, Display};
use std::rc::Rc;

use ::serde::{Deserialize, Serialize};
use uniplate::derive::Uniplate;
use uniplate::{Biplate, Tree, Uniplate};

use super::name::Name;
use super::serde::{DefaultWithId, HasId, ObjId};
use super::types::Typeable;
use super::{DecisionVariable, Domain, Expression, RecordEntry, ReturnType};

thread_local! {
    // make each thread have its own id counter.
    static DECLARATION_PTR_ID_COUNTER: Cell<u32> = const { Cell::new(0) };

    // We run integration tests in parallel threads - making this thread local ensures that
    // declarations in a test always have the same id, instead of the ids depending on how many
    // threads are running, how they are scheduled, etc.
}

#[doc(hidden)]
/// Resets the id counter of `DeclarationPtr` to 0.
///
/// This is probably always a bad idea.
pub fn reset_declaration_id_unchecked() {
    DECLARATION_PTR_ID_COUNTER.set(0);
}

/// A shared pointer to a [`Declaration`].
///
/// Two declaration pointers are equal if they point to the same underlying declaration.
///
/// # Id
///
///  The id of `DeclarationPtr` obeys the following invariants:
///
/// 1. Declaration pointers have the same id if they point to the same
///    underlying declaration.
///
/// 2. The id is immutable.
///
/// 3. Changing the declaration pointed to by the declaration pointer does not change the id. This
///    allows declarations to be updated by replacing them with a newer version of themselves.
///
/// `Ord`, `Hash`, and `Eq` use id for comparisons.
/// # Serde
///
/// Declaration pointers can be serialised using the following serializers:
///
/// + [`DeclarationPtrFull`](serde::DeclarationPtrFull)
/// + [`DeclarationPtrAsId`](serde::DeclarationPtrAsId)
///
/// See their documentation for more information.
#[derive(Clone, Debug)]
pub struct DeclarationPtr {
    // the shared bits of the pointer
    inner: Rc<DeclarationPtrInner>,
}

// The bits of a declaration that are shared between all pointers.
#[derive(Clone, Debug)]
struct DeclarationPtrInner {
    // We don't want this to be mutable, as `HashMap` and `BTreeMap` rely on the hash or order of
    // keys to be unchanging.
    //
    // See:  https://rust-lang.github.io/rust-clippy/master/index.html#mutable_key_type
    id: ObjId,

    // The contents of the declaration itself should be mutable.
    value: RefCell<Declaration>,
}

impl DeclarationPtrInner {
    fn new(value: RefCell<Declaration>) -> Rc<DeclarationPtrInner> {
        Rc::new(DeclarationPtrInner {
            id: DECLARATION_PTR_ID_COUNTER.replace(DECLARATION_PTR_ID_COUNTER.get() + 1),
            value,
        })
    }

    // SAFETY: only use if you are really really sure you arn't going to break the id invariants of
    // DeclarationPtr and HasId!
    fn new_with_id_unchecked(value: RefCell<Declaration>, id: ObjId) -> Rc<DeclarationPtrInner> {
        Rc::new(DeclarationPtrInner { id, value })
    }
}

#[allow(dead_code)]
impl DeclarationPtr {
    /******************************/
    /*        Constructors        */
    /******************************/

    /// Creates a `DeclarationPtr` for the given `Declaration`.
    fn from_declaration(declaration: Declaration) -> DeclarationPtr {
        DeclarationPtr {
            inner: DeclarationPtrInner::new(RefCell::new(declaration)),
        }
    }

    /// Creates a new declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,DeclarationKind,Domain,Range};
    ///
    /// // letting MyDomain be int(1..5)
    /// let declaration = DeclarationPtr::new(
    ///     Name::User("MyDomain".into()),
    ///     DeclarationKind::DomainLetting(Domain::Int(vec![
    ///         Range::Bounded(1,5)])));
    /// ```
    pub fn new(name: Name, kind: DeclarationKind) -> DeclarationPtr {
        DeclarationPtr::from_declaration(Declaration::new(name, kind))
    }

    /// Creates a new decision variable declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,DeclarationKind,Domain,Range};
    ///
    /// // find x: int(1..5)
    /// let declaration = DeclarationPtr::new_var(
    ///     Name::User("x".into()),
    ///     Domain::Int(vec![Range::Bounded(1,5)]));
    ///
    /// ```
    pub fn new_var(name: Name, domain: Domain) -> DeclarationPtr {
        let kind = DeclarationKind::DecisionVariable(DecisionVariable::new(domain));
        DeclarationPtr::new(name, kind)
    }

    /// Creates a new domain letting declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,DeclarationKind,Domain,Range};
    ///
    /// // letting MyDomain be int(1..5)
    /// let declaration = DeclarationPtr::new_domain_letting(
    ///     Name::User("MyDomain".into()),
    ///     Domain::Int(vec![Range::Bounded(1,5)]));
    ///
    /// ```
    pub fn new_domain_letting(name: Name, domain: Domain) -> DeclarationPtr {
        let kind = DeclarationKind::DomainLetting(domain);
        DeclarationPtr::new(name, kind)
    }

    /// Creates a new value letting declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,DeclarationKind,Domain,Range, Expression,
    /// Literal,Atom};
    /// use conjure_core::{matrix_expr,metadata::Metadata};
    ///
    /// // letting n be 10 + 10
    /// let ten = Expression::Atomic(Metadata::new(),Atom::Literal(Literal::Int(10)));
    /// let expression = Expression::Sum(Metadata::new(),Box::new(matrix_expr![ten.clone(),ten]));
    /// let declaration = DeclarationPtr::new_value_letting(
    ///     Name::User("n".into()),
    ///     expression);
    ///
    /// ```
    pub fn new_value_letting(name: Name, expression: Expression) -> DeclarationPtr {
        let kind = DeclarationKind::ValueLetting(expression);
        DeclarationPtr::new(name, kind)
    }

    /// Creates a new given declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,DeclarationKind,Domain,Range};
    ///
    /// // given n: int(1..5)
    /// let declaration = DeclarationPtr::new_given(
    ///     Name::User("n".into()),
    ///     Domain::Int(vec![Range::Bounded(1,5)]));
    ///
    /// ```
    pub fn new_given(name: Name, domain: Domain) -> DeclarationPtr {
        let kind = DeclarationKind::Given(domain);
        DeclarationPtr::new(name, kind)
    }

    /// Creates a new record field declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,records::RecordEntry,Domain,Range};
    ///
    /// // create declaration for field A in `find rec: record {A: int(0..1), B: int(0..2)}`
    ///
    /// let field = RecordEntry {
    ///     name: Name::User("n".into()),
    ///     domain: Domain::Int(vec![Range::Bounded(1,5)])
    /// };
    ///
    /// let declaration = DeclarationPtr::new_record_field(field);
    /// ```
    pub fn new_record_field(entry: RecordEntry) -> DeclarationPtr {
        let kind = DeclarationKind::RecordField(entry.domain);
        DeclarationPtr::new(entry.name, kind)
    }

    /**********************************************/
    /*        Declaration accessor methods        */
    /**********************************************/

    /// Gets the domain of the declaration, if it has one.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,Domain,Range};
    ///
    /// // find a: int(1..5)
    /// let declaration = DeclarationPtr::new_var(Name::User("a".into()),Domain::Int(vec![Range::Bounded(1,5)]));
    ///
    /// assert!(declaration.domain().is_some_and(|x| (&x as &Domain) == &Domain::Int(vec![Range::Bounded(1,5)])))
    ///
    /// ```
    pub fn domain(&self) -> Option<Ref<Domain>> {
        Ref::filter_map(self.borrow(), Declaration::domain).ok()
    }

    /// Gets the kind of the declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,DeclarationKind,Name,Domain,Range};
    ///
    /// // find a: int(1..5)
    /// let declaration = DeclarationPtr::new_var(Name::User("a".into()),Domain::Int(vec![Range::Bounded(1,5)]));
    /// assert!(matches!(&declaration.kind() as &DeclarationKind, DeclarationKind::DecisionVariable(_)))
    /// ```
    pub fn kind(&self) -> Ref<DeclarationKind> {
        self.map(Declaration::kind)
    }

    /// Gets the name of the declaration.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Name,Domain,Range};
    ///
    /// // find a: int(1..5)
    /// let declaration = DeclarationPtr::new_var(Name::User("a".into()),Domain::Int(vec![Range::Bounded(1,5)]));
    ///
    /// assert_eq!(&declaration.name() as &Name, &Name::User("a".into()))
    /// ```
    pub fn name(&self) -> Ref<Name> {
        self.map(Declaration::name)
    }

    /// This declaration as a decision variable, if it is one.
    pub fn as_var(&self) -> Option<Ref<DecisionVariable>> {
        Ref::filter_map(self.borrow(), |x| {
            if let DeclarationKind::DecisionVariable(var) = &x.kind {
                Some(var)
            } else {
                None
            }
        })
        .ok()
    }

    /// This declaration as a mutable decision variable, if it is one.
    pub fn as_var_mut(&mut self) -> Option<RefMut<DecisionVariable>> {
        RefMut::filter_map(self.borrow_mut(), |x| {
            if let DeclarationKind::DecisionVariable(var) = &mut x.kind {
                Some(var)
            } else {
                None
            }
        })
        .ok()
    }

    /// This declaration as a domain letting, if it is one.
    pub fn as_domain_letting(&self) -> Option<Ref<Domain>> {
        Ref::filter_map(self.borrow(), |x| {
            if let DeclarationKind::DomainLetting(domain) = &x.kind {
                Some(domain)
            } else {
                None
            }
        })
        .ok()
    }

    /// This declaration as a mutable domain letting, if it is one.
    pub fn as_domain_letting_mut(&mut self) -> Option<RefMut<Domain>> {
        RefMut::filter_map(self.borrow_mut(), |x| {
            if let DeclarationKind::DomainLetting(domain) = &mut x.kind {
                Some(domain)
            } else {
                None
            }
        })
        .ok()
    }

    /// This declaration as a value letting, if it is one.
    pub fn as_value_letting(&self) -> Option<Ref<Expression>> {
        Ref::filter_map(self.borrow(), |x| {
            if let DeclarationKind::ValueLetting(e) = &x.kind {
                Some(e)
            } else {
                None
            }
        })
        .ok()
    }

    /// This declaration as a mutable value letting, if it is one.
    pub fn as_value_letting_mut(&mut self) -> Option<RefMut<Expression>> {
        RefMut::filter_map(self.borrow_mut(), |x| {
            if let DeclarationKind::ValueLetting(e) = &mut x.kind {
                Some(e)
            } else {
                None
            }
        })
        .ok()
    }

    /// Changes the name in this declaration, returning the old one.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr, Domain, Range, Name};
    ///
    /// // find a: int(1..5)
    /// let mut declaration = DeclarationPtr::new_var(Name::User("a".into()),Domain::Int(vec![Range::Bounded(1,5)]));
    ///
    /// let old_name = declaration.replace_name(Name::User("b".into()));
    /// assert_eq!(old_name,Name::User("a".into()));
    /// assert_eq!(&declaration.name() as &Name,&Name::User("b".into()));
    /// ```
    pub fn replace_name(&mut self, name: Name) -> Name {
        let mut decl = self.borrow_mut();
        std::mem::replace(&mut decl.name, name)
    }

    /*****************************************/
    /*        Pointer utility methods        */
    /*****************************************/

    // These are mostly wrappers over RefCell, Ref, and RefMut methods, re-exported here for
    // convenience.

    /// Immutably borrows the declaration.
    fn borrow(&self) -> Ref<Declaration> {
        // unlike refcell.borrow(), this never panics
        self.inner.value.borrow()
    }

    /// Mutably borrows the declaration.
    fn borrow_mut(&mut self) -> RefMut<Declaration> {
        // unlike refcell.borrow_mut(), this never panics
        self.inner.value.borrow_mut()
    }

    /// Creates a new declaration pointer with the same contents as `self` that is not shared with
    /// anyone else.
    ///
    /// As the resulting pointer is unshared, it will have a new id.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_core::ast::{DeclarationPtr,Declaration,Name,Domain,Range};
    ///
    /// // find a: int(1..5)
    /// let declaration = DeclarationPtr::new_var(Name::User("a".into()),Domain::Int(vec![Range::Bounded(1,5)]));
    ///
    /// let mut declaration2 = declaration.clone();
    ///
    /// declaration2.replace_name(Name::User("b".into()));
    ///
    /// assert_eq!(&declaration.name() as &Name, &Name::User("b".into()));
    /// assert_eq!(&declaration2.name() as &Name, &Name::User("b".into()));
    ///
    /// declaration2 = declaration2.detach();
    ///
    /// assert_eq!(&declaration2.name() as &Name, &Name::User("b".into()));
    ///
    /// declaration2.replace_name(Name::User("c".into()));
    ///
    /// assert_eq!(&declaration.name() as &Name, &Name::User("b".into()));
    /// assert_eq!(&declaration2.name() as &Name, &Name::User("c".into()));
    /// ```
    pub fn detach(self) -> DeclarationPtr {
        // despite having the same contents, the new declaration pointer is unshared, so it should
        // get a new id.
        DeclarationPtr {
            inner: DeclarationPtrInner::new(self.inner.value.clone()),
        }
    }

    /// Applies `f` to the declaration, returning the result as a reference.
    fn map<U>(&self, f: impl FnOnce(&Declaration) -> &U) -> Ref<U> {
        Ref::map(self.borrow(), f)
    }

    /// Applies mutable function `f` to the declaration, returning the result as a mutable reference.
    fn map_mut<U>(&mut self, f: impl FnOnce(&mut Declaration) -> &mut U) -> RefMut<U> {
        RefMut::map(self.borrow_mut(), f)
    }

    /// Replaces the declaration with a new one, returning the old value, without deinitialising
    /// either one.
    fn replace(&mut self, declaration: Declaration) -> Declaration {
        self.inner.value.replace(declaration)
    }
}

impl HasId for DeclarationPtr {
    fn id(&self) -> ObjId {
        self.inner.id
    }
}

impl DefaultWithId for DeclarationPtr {
    fn default_with_id(id: ObjId) -> Self {
        DeclarationPtr {
            inner: DeclarationPtrInner::new_with_id_unchecked(
                RefCell::new(Declaration {
                    name: Name::User("_UNKNOWN".into()),
                    kind: DeclarationKind::ValueLetting(false.into()),
                }),
                id,
            ),
        }
    }
}

impl Typeable for DeclarationPtr {
    fn return_type(&self) -> Option<ReturnType> {
        match &self.kind() as &DeclarationKind {
            DeclarationKind::DecisionVariable(var) => var.return_type(),
            DeclarationKind::ValueLetting(expression) => expression.return_type(),
            DeclarationKind::DomainLetting(domain) => domain.return_type(),
            DeclarationKind::Given(domain) => domain.return_type(),
            DeclarationKind::RecordField(domain) => domain.return_type(),
        }
    }
}

impl Uniplate for DeclarationPtr {
    fn uniplate(&self) -> (Tree<Self>, Box<dyn Fn(Tree<Self>) -> Self>) {
        let decl = self.borrow();
        let (tree, recons) = Biplate::<DeclarationPtr>::biplate(&decl as &Declaration);

        let self2 = self.clone();
        (
            tree,
            Box::new(move |x| {
                let mut self3 = self2.clone();
                let inner = recons(x);
                *(&mut self3.borrow_mut() as &mut Declaration) = inner;
                self3
            }),
        )
    }
}

impl<To> Biplate<To> for DeclarationPtr
where
    Declaration: Biplate<To>,
    To: Uniplate,
{
    fn biplate(&self) -> (Tree<To>, Box<dyn Fn(Tree<To>) -> Self>) {
        if TypeId::of::<To>() == TypeId::of::<Self>() {
            let self2 = self.clone();
            (Tree::Zero, Box::new(move |_| self2.clone()))
        } else {
            // call biplate on the enclosed declaration
            let decl = self.borrow();
            let (tree, recons) = Biplate::<To>::biplate(&decl as &Declaration);

            let self2 = self.clone();
            (
                tree,
                Box::new(move |x| {
                    let mut self3 = self2.clone();
                    let inner = recons(x);
                    *(&mut self3.borrow_mut() as &mut Declaration) = inner;
                    self3
                }),
            )
        }
    }
}

impl Ord for DeclarationPtr {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.inner.id.cmp(&other.inner.id)
    }
}

impl PartialOrd for DeclarationPtr {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for DeclarationPtr {
    fn eq(&self, other: &Self) -> bool {
        self.inner.id == other.inner.id
    }
}

impl Eq for DeclarationPtr {}

impl std::hash::Hash for DeclarationPtr {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // invariant: x == y -> hash(x) == hash(y)
        self.inner.id.hash(state);
    }
}

impl Display for DeclarationPtr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value: &Declaration = &self.borrow();
        value.fmt(f)
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Eq, Uniplate)]
#[biplate(to=Expression,walk_into=[DeclarationKind])]
#[biplate(to=DeclarationPtr,walk_into=[DeclarationKind])]
#[biplate(to=Name)]
#[uniplate(walk_into=[DeclarationKind])]
#[deprecated = "use DeclarationPtr instead."]
pub struct Declaration {
    /// The name of the declared symbol.
    name: Name,

    /// The kind of the declaration.
    kind: DeclarationKind,
}

/// A specific kind of declaration.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[biplate(to=Expression)]
#[biplate(to=DeclarationPtr)]
#[biplate(to=Declaration)]
pub enum DeclarationKind {
    DecisionVariable(DecisionVariable),
    ValueLetting(Expression),
    DomainLetting(Domain),
    Given(Domain),

    /// A named field inside a record type.
    /// e.g. A, B in record{A: int(0..1), B: int(0..2)}
    RecordField(Domain),
}

impl Declaration {
    /// Creates a new declaration.
    #[deprecated = "use DeclarationPtr::new instead."]
    pub fn new(name: Name, kind: DeclarationKind) -> Declaration {
        Declaration { name, kind }
    }

    /// Creates a new decision variable declaration.
    #[deprecated = "use DeclarationPtr::new_var instead."]
    pub fn new_var(name: Name, domain: Domain) -> Declaration {
        Declaration {
            name,
            kind: DeclarationKind::DecisionVariable(DecisionVariable::new(domain)),
        }
    }

    /// Creates a new domain letting declaration.
    #[deprecated = "use DeclarationPtr::new_domain_letting instead."]
    pub fn new_domain_letting(name: Name, domain: Domain) -> Declaration {
        Declaration {
            name,
            kind: DeclarationKind::DomainLetting(domain),
        }
    }

    /// Creates a new value letting declaration.
    #[deprecated = "use DeclarationPtr::new_value_letting instead."]
    pub fn new_value_letting(name: Name, value: Expression) -> Declaration {
        Declaration {
            name,
            kind: DeclarationKind::ValueLetting(value),
        }
    }

    /// Creates a new given declaration.
    #[deprecated = "use DeclarationPtr::new_given instead."]
    pub fn new_given(name: Name, domain: Domain) -> Declaration {
        Declaration {
            name,
            kind: DeclarationKind::Given(domain),
        }
    }

    /// The name of this declaration.
    #[deprecated = "use DeclarationPtr::name instead."]
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The kind of this declaration.
    #[deprecated = "use DeclarationPtr::kind instead."]
    pub fn kind(&self) -> &DeclarationKind {
        &self.kind
    }

    /// The domain of this declaration, if it is known.
    #[deprecated = "use DeclarationPtr::domain instead."]
    pub fn domain(&self) -> Option<&Domain> {
        match self.kind() {
            DeclarationKind::DecisionVariable(var) => Some(&var.domain),
            DeclarationKind::ValueLetting(_) => None,
            DeclarationKind::DomainLetting(domain) => Some(domain),
            DeclarationKind::Given(domain) => Some(domain),
            DeclarationKind::RecordField(domain) => Some(domain),
        }
    }

    /// This declaration as a decision variable, if it is one.
    #[deprecated = "use DeclarationPtr::as_var instead."]
    pub fn as_var(&self) -> Option<&DecisionVariable> {
        if let DeclarationKind::DecisionVariable(var) = self.kind() {
            Some(var)
        } else {
            None
        }
    }

    /// This declaration as a mutable decision variable, if it is one.
    #[deprecated = "use DeclarationPtr::as_var_mut instead."]
    pub fn as_var_mut(&mut self) -> Option<&mut DecisionVariable> {
        if let DeclarationKind::DecisionVariable(var) = &mut self.kind {
            Some(var)
        } else {
            None
        }
    }

    /// This declaration as a domain letting, if it is one.
    #[deprecated = "use DeclarationPtr::as_domain_letting instead."]
    pub fn as_domain_letting(&self) -> Option<&Domain> {
        if let DeclarationKind::DomainLetting(domain) = self.kind() {
            Some(domain)
        } else {
            None
        }
    }

    /// This declaration as a mutable domain letting, if it is one.
    #[deprecated = "use DeclarationPtr::as_domain_letting_mut instead."]
    pub fn as_domain_letting_mut(&mut self) -> Option<&mut Domain> {
        if let DeclarationKind::DomainLetting(domain) = &mut self.kind {
            Some(domain)
        } else {
            None
        }
    }

    /// This declaration as a value letting, if it is one.
    #[deprecated = "use DeclarationPtr::as_value_letting instead."]
    pub fn as_value_letting(&self) -> Option<&Expression> {
        if let DeclarationKind::ValueLetting(expr) = &self.kind {
            Some(expr)
        } else {
            None
        }
    }

    /// This declaration as a mutable value letting, if it is one.
    #[deprecated = "use DeclarationPtr::as_value_letting_mut instead."]
    pub fn as_value_letting_mut(&mut self) -> Option<&mut Expression> {
        if let DeclarationKind::ValueLetting(expr) = &mut self.kind {
            Some(expr)
        } else {
            None
        }
    }

    /// Returns a clone of this declaration with a new name.
    #[deprecated = "use DeclarationPtr::replace_name instead."]
    pub fn with_new_name(mut self, name: Name) -> Declaration {
        self.name = name;
        self
    }
}

pub mod serde {
    use std::cell::RefCell;

    use crate::ast::serde::DefaultWithId;
    use crate::ast::{Name, serde::HasId};
    use ::serde::Deserialize;
    use ::serde::Serialize;
    use serde_with::{DeserializeAs, SerializeAs};

    use super::{Declaration, DeclarationKind, DeclarationPtr, DeclarationPtrInner};

    /// (De)serializes a [`DeclarationPtr`] as its id.
    ///
    /// On deserialization, each declaration is re-created with default dummy values, except for the
    /// id, which will be the same as the original declaration.
    ///
    /// It is left to the user to fix these values before use. Before serialization, each object in
    /// memory has a unique id; using this information, re-constructing the shared pointers should be
    /// possible, as long as the contents of each object were also stored, e.g. with [`DeclarationPtrFull`].
    ///
    /// # Examples
    ///
    /// Serialisation:
    ///
    /// ```
    /// use serde::Serialize;
    /// use serde_json::json;
    /// use serde_with::serde_as;
    /// use conjure_core::ast::{declaration::serde::DeclarationPtrAsId,Name,Declaration,DeclarationPtr,Domain,Range};
    ///
    /// // some struct containing a DeclarationPtr.
    /// #[serde_as]
    /// #[derive(Clone,PartialEq,Eq,Serialize)]
    /// struct Foo {
    ///     #[serde_as(as = "DeclarationPtrAsId")]
    ///     declaration: DeclarationPtr,
    ///
    ///     // serde as also supports nesting
    ///     #[serde_as(as = "Vec<(_,DeclarationPtrAsId)>")]
    ///     declarations: Vec<(i32,DeclarationPtr)>,
    ///
    ///     c: i32
    /// }
    ///
    /// let declaration = DeclarationPtr::new_var(Name::User("a".into()),Domain::Int(vec![Range::Bounded(1,5)]));
    /// let mut declarations: Vec<(i32,DeclarationPtr)>  = vec![];
    /// for i in (1..=2) {
    ///     declarations.push((i,DeclarationPtr::new_var(Name::User(format!("{i}").into()),Domain::Int(vec![Range::Bounded(1,5)]))));
    /// }
    ///
    /// let foo = Foo {
    ///     declaration,
    ///     declarations,
    ///     c: 3
    /// };
    ///
    /// let json = serde_json::to_value(foo).unwrap();
    ///
    /// let expected_json = json!({
    ///     "declaration": 0,
    ///     "declarations": [(1,1),(2,2)],
    ///     "c": 3
    /// });
    ///
    /// assert_eq!(json,expected_json);
    /// ```
    ///
    /// Deserialisation:
    ///
    /// ```
    /// use serde::Deserialize;
    /// use serde_json::json;
    /// use serde_with::serde_as;
    /// use conjure_core::ast::{serde::{HasId},declaration::serde::DeclarationPtrAsId,Name,Declaration,DeclarationKind, DeclarationPtr,Domain,Range, ReturnType};
    ///
    /// // some struct containing a DeclarationPtr.
    /// #[serde_as]
    /// #[derive(Clone,PartialEq,Eq,Deserialize)]
    /// struct Foo {
    ///     #[serde_as(as = "DeclarationPtrAsId")]
    ///     declaration: DeclarationPtr,
    ///
    ///     // serde as also supports nesting
    ///     #[serde_as(as = "Vec<(_,DeclarationPtrAsId)>")]
    ///     declarations: Vec<(i32,DeclarationPtr)>,
    ///     c: i32
    /// }
    ///
    /// let input_json = json!({
    ///     "declaration": 10,
    ///     "declarations": [(11,11),(12,12)],
    ///     "c": 3
    /// });
    ///
    ///
    /// let foo: Foo = serde_json::from_value(input_json).unwrap();
    ///
    ///
    /// // all declarations should have the dummy values: name: User("_UNKNOWN"), kind: value_letting;
    /// assert_eq!(&foo.declaration.name() as &Name,&Name::User("_UNKNOWN".into()));
    /// assert_eq!(&foo.declarations[0].1.name() as &Name,&Name::User("_UNKNOWN".into()));
    /// assert_eq!(&foo.declarations[1].1.name() as &Name,&Name::User("_UNKNOWN".into()));
    ///
    /// assert!(matches!(&foo.declaration.kind() as &DeclarationKind,&DeclarationKind::ValueLetting(_)));
    /// assert!(matches!(&foo.declarations[0].1.kind() as &DeclarationKind,&DeclarationKind::ValueLetting(_)));
    /// assert!(matches!(&foo.declarations[1].1.kind() as &DeclarationKind,&DeclarationKind::ValueLetting(_)));
    ///
    /// // but ids should be the same
    ///
    /// assert_eq!(*&foo.declaration.id(),10);
    /// assert_eq!(*&foo.declarations[0].1.id(),11);
    /// assert_eq!(*&foo.declarations[1].1.id(),12);
    /// ```
    ///
    pub struct DeclarationPtrAsId;

    impl SerializeAs<DeclarationPtr> for DeclarationPtrAsId {
        fn serialize_as<S>(source: &DeclarationPtr, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let id = source.id();
            serializer.serialize_u32(id)
        }
    }

    impl<'de> DeserializeAs<'de, DeclarationPtr> for DeclarationPtrAsId {
        fn deserialize_as<D>(deserializer: D) -> Result<DeclarationPtr, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let id = u32::deserialize(deserializer)?;
            Ok(DeclarationPtr::default_with_id(id))
        }
    }

    /// (De)serializes a [`DeclarationPtr`] as the declaration it references.
    ///
    /// This makes no attempt to restore the pointers - each value is deserialized into a new
    /// `DeclarationPtr` with a reference count of one.
    ///
    /// The shared references can be reconstructed using the ids stored, as before serialization these
    /// were unique for each separate instance of `T` in memory. See [`DeclarationPtrAsId`].
    ///
    /// # Examples
    ///
    /// Serialisation:
    ///
    /// ```
    /// use serde::Serialize;
    /// use serde_json::json;
    /// use serde_with::serde_as;
    /// use conjure_core::ast::{declaration::serde::DeclarationPtrFull,Name,Declaration,DeclarationPtr,Domain,Range};
    ///
    /// // some struct containing a DeclarationPtr.
    /// #[serde_as]
    /// #[derive(Clone,PartialEq,Eq,Serialize)]
    /// struct Foo {
    ///     #[serde_as(as = "DeclarationPtrFull")]
    ///     declaration: DeclarationPtr,
    ///
    ///     // serde as also supports nesting
    ///     #[serde_as(as = "Vec<(_,DeclarationPtrFull)>")]
    ///     declarations: Vec<(i32,DeclarationPtr)>,
    ///
    ///     c: i32
    /// }
    ///
    /// let declaration = DeclarationPtr::new_var(Name::User("a".into()),Domain::Int(vec![Range::Bounded(1,5)]));
    /// let mut declarations = vec![];
    ///
    /// for i in (1..=2) {
    ///     let d = DeclarationPtr::new_var(Name::User(format!("{i}").into()),Domain::Int(vec![Range::Bounded(1,5)]));
    ///     declarations.push((i,d));
    /// }
    ///
    /// let foo = Foo {
    ///     declaration,
    ///     declarations,
    ///     c: 3
    /// };
    ///
    /// let json = serde_json::to_value(foo).unwrap();
    ///
    /// let expected_json = json!({
    ///     "declaration": {
    ///         "name": { "User": "a"},
    ///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [1,5]}]}}},
    ///         "id": 0
    ///     },
    ///
    ///     "declarations": [
    ///         [1,{
    ///         "name": { "User": "1"},
    ///         "id": 1,
    ///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [1,5]}]}}},
    ///         }],
    ///         [2,{
    ///         "name": { "User": "2"},
    ///         "id": 2,
    ///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [1,5]}]}}},
    ///         }]
    ///     ],
    ///         
    ///     "c": 3
    /// });
    ///
    /// assert_eq!(json,expected_json);
    /// ```
    ///
    /// Deserialisation:
    ///
    /// ```
    /// use serde::Deserialize;
    /// use serde_json::json;
    /// use serde_with::serde_as;
    /// use conjure_core::ast::{serde::{HasId},declaration::serde::DeclarationPtrFull,Name,Declaration,DeclarationKind, DeclarationPtr,Domain,Range, ReturnType};
    ///
    /// // some struct containing a DeclarationPtr.
    /// #[serde_as]
    /// #[derive(Clone,PartialEq,Eq,Deserialize)]
    /// struct Foo {
    ///     #[serde_as(as = "DeclarationPtrFull")]
    ///     declaration: DeclarationPtr,
    ///
    ///     // serde as also supports nesting
    ///     #[serde_as(as = "Vec<(_,DeclarationPtrFull)>")]
    ///     declarations: Vec<(i32,DeclarationPtr)>,
    ///     c: i32
    /// }
    ///
    /// let input_json = json!({
    ///     "declaration": {
    ///         "name": { "User": "a"},
    ///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [0,5]}]}}},
    ///         "id": 10,
    ///     },
    ///
    ///     "declarations": [
    ///         [1,{
    ///         "name": { "User": "1"},
    ///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [0,5]}]}}},
    ///         "id": 11,
    ///         }],
    ///         [2,{
    ///         "name": { "User": "2"},
    ///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [0,5]}]}}},
    ///         "id": 12,
    ///         }]
    ///     ],
    ///     "c": 3
    /// });
    ///
    ///
    /// let foo: Foo = serde_json::from_value(input_json).unwrap();
    ///
    /// assert_eq!(&foo.declaration.name() as &Name,&Name::User("a".into()));
    /// assert_eq!(&foo.declarations[0].1.name() as &Name,&Name::User("1".into()));
    /// assert_eq!(&foo.declarations[1].1.name() as &Name,&Name::User("2".into()));
    ///
    /// assert!(matches!(&foo.declaration.kind() as &DeclarationKind,&DeclarationKind::DecisionVariable(_)));
    /// assert!(matches!(&foo.declarations[0].1.kind() as &DeclarationKind,&DeclarationKind::DecisionVariable(_)));
    /// assert!(matches!(&foo.declarations[1].1.kind() as &DeclarationKind,&DeclarationKind::DecisionVariable(_)));
    ///
    /// // ids should be the same as in the json
    /// assert_eq!(*&foo.declaration.id(),10);
    /// assert_eq!(*&foo.declarations[0].1.id(),11);
    /// assert_eq!(*&foo.declarations[1].1.id(),12);
    /// ```
    pub struct DeclarationPtrFull;

    // temporary structs to put things in the right format befo:re we (de)serialize
    //
    // this is a bit of a hack to get around the nested types in declarationPtr.
    #[derive(Serialize)]
    struct DeclarationSe<'a> {
        name: &'a Name,
        kind: &'a DeclarationKind,
        id: u32,
    }

    #[derive(Deserialize)]
    struct DeclarationDe {
        name: Name,
        kind: DeclarationKind,
        id: u32,
    }

    impl SerializeAs<DeclarationPtr> for DeclarationPtrFull {
        fn serialize_as<S>(source: &DeclarationPtr, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let id = source.id();
            let decl: &Declaration = &source.borrow();
            let x = DeclarationSe {
                name: &decl.name,
                kind: &decl.kind,
                id,
            };

            x.serialize(serializer)
        }
    }

    impl<'de> DeserializeAs<'de, DeclarationPtr> for DeclarationPtrFull {
        fn deserialize_as<D>(deserializer: D) -> Result<DeclarationPtr, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let x = DeclarationDe::deserialize(deserializer)?;
            Ok(DeclarationPtr {
                inner: DeclarationPtrInner::new_with_id_unchecked(
                    RefCell::new(Declaration::new(x.name, x.kind)),
                    x.id,
                ),
            })
        }
    }
}
