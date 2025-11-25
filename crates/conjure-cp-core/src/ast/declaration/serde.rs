use std::cell::RefCell;

use crate::ast::serde::DefaultWithId;
use crate::ast::{Name, serde::HasId};
use serde::Deserialize;
use serde::Serialize;
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
/// use conjure_cp_core::ast::{declaration::serde::DeclarationPtrAsId,Name,DeclarationPtr,Domain,Range};
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
/// use conjure_cp_core::ast::{serde::{HasId},declaration::serde::DeclarationPtrAsId,Name,DeclarationKind, DeclarationPtr,Domain,Range, ReturnType};
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
/// use conjure_cp_core::ast::{declaration::serde::DeclarationPtrFull,Name,DeclarationPtr,Domain,Range};
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
///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [1,5]}]}, "category":
///         "Decision"}},
///         "id": 0
///     },
///
///     "declarations": [
///         [1,{
///         "name": { "User": "1"},
///         "id": 1,
///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [1,5]}]},
///         "category":"Decision"}},
///         }],
///         [2,{
///         "name": { "User": "2"},
///         "id": 2,
///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [1,5]}]},"category":
///         "Decision"}},
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
/// use conjure_cp_core::ast::{serde::{HasId},declaration::serde::DeclarationPtrFull,Name,DeclarationKind, DeclarationPtr,Domain,Range, ReturnType};
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
///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [0,5]}]}, "category":
///         "Decision"}},
///         "id": 10,
///     },
///
///     "declarations": [
///         [1,{
///         "name": { "User": "1"},
///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [0,5]}]}, "category":
///         "Decision"}},
///         "id": 11,
///         }],
///         [2,{
///         "name": { "User": "2"},
///         "kind": {"DecisionVariable": {"domain": {"Int": [{"Bounded": [0,5]}]}, "category":
///         "Decision"}},
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
