//! Serde serialization/ deserialization helpers.
//!
//! These are used in combination with the
//! [`serde_as`](https://docs.rs/serde_with/3.12.0/serde_with/index.html) annotation on AST types.

use std::cell::RefCell;
use std::rc::Rc;

use serde::Deserialize;
use serde::Serialize;
use serde::de::Error;
use serde_with::{DeserializeAs, SerializeAs};
use ustr::Ustr;

/// A unique id, used to distinguish between objects of the same type.
pub type ObjectId = u32;

/// A unique "global" id, used to distinguish between objects of any type.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub struct GlobalId {
    pub type_name: Ustr,
    pub oid: ObjectId,
}

/// A type with an id.
///
/// Each object of the implementing type has a unique [`ObjectId`]; however, object ids are not unique
/// for different type of objects. For this, use the [`GlobalId`] instead.
///
/// Implementing types should ensure that the id is updated when an object is cloned.
pub trait HasId {
    /// A unique string to identify this type.
    const TYPE_NAME: &'static str;

    /// The [`ObjectId`] of this object.
    ///
    /// Each object of this type has a unique [`ObjectId`]; however, object ids are not unique for
    /// different type of objects.
    fn object_id(&self) -> ObjectId;

    /// The [`GlobalId`] for this object.
    ///
    /// This id is unique for objects of any type, so can be used to compare two objects of
    /// different types.
    fn global_id(&self) -> GlobalId {
        GlobalId {
            type_name: Self::TYPE_NAME.into(),
            oid: self.object_id(),
        }
    }
}

/// A type that can be created with default values and an id.
pub trait DefaultWithId: HasId {
    /// Creates a new default value of type `T`, but with the given [`ObjId`].
    fn default_with_object_id(id: ObjectId) -> Self;

    /// Creates a new default value of type `T`, but with the given [`GlobalId`].
    ///
    /// # Panics
    ///
    /// If the type associated with the given global id is not the type being constructed.
    fn default_with_global_id(id: GlobalId) -> Self
    where
        Self: Sized,
    {
        if id.type_name != Self::TYPE_NAME {
            panic!(
                "Expected global id for an object of type {}, but got a global id for an object of type {}",
                id.type_name,
                Self::TYPE_NAME
            );
        };

        Self::default_with_object_id(id.oid)
    }
}

/// De/Serialize an `Rc<RefCell<T>>` as the id of the inner value `T`.
///
/// On de-serialization, each object is created as the default value for that type, except with the
/// id's being retained.
///
/// It is left to the user to fix these values before use. Before serialization, each object in
/// memory has a unique id; using this information, re-constructing the shared pointers should be
/// possible, as long as the contents of each object were also stored, e.g. with
/// [`RcRefCellAsInner`].
pub struct RcRefCellAsId;

// https://docs.rs/serde_with/3.12.0/serde_with/trait.SerializeAs.html#implementing-a-converter-type

impl<T> SerializeAs<Rc<RefCell<T>>> for RcRefCellAsId
where
    T: HasId,
{
    fn serialize_as<S>(source: &Rc<RefCell<T>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let id = (**source).borrow().global_id();
        id.serialize(serializer)
    }
}

// https://docs.rs/serde_with/3.12.0/serde_with/trait.DeserializeAs.html
impl<'de, T> DeserializeAs<'de, Rc<RefCell<T>>> for RcRefCellAsId
where
    T: HasId + DefaultWithId,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Rc<RefCell<T>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let id = GlobalId::deserialize(deserializer).map_err(Error::custom)?;
        Ok(Rc::new(RefCell::new(T::default_with_global_id(id))))
    }
}

/// De/Serialize an `Rc<RefCell<T>>` as its inner value `T`.
///
/// This makes no attempt to restore the pointers - each value is de-serialized into a new
/// Rc<RefCell<T>> with a reference count of one.
///
/// The shared references can be reconstructed using the ids stored, as before serialization these
/// were unique for each separate instance of `T` in memory. See [`RcRefCellAsId`].
pub struct RcRefCellAsInner;

impl<T> SerializeAs<Rc<RefCell<T>>> for RcRefCellAsInner
where
    T: Serialize + HasId,
{
    fn serialize_as<S>(source: &Rc<RefCell<T>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (**source).borrow().serialize(serializer)
    }
}

impl<T> SerializeAs<Rc<T>> for RcRefCellAsInner
where
    T: Serialize + HasId,
{
    fn serialize_as<S>(source: &Rc<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        source.serialize(serializer)
    }
}

impl<'de, T> DeserializeAs<'de, Rc<RefCell<T>>> for RcRefCellAsInner
where
    T: Deserialize<'de> + HasId + DefaultWithId,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Rc<RefCell<T>>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = T::deserialize(deserializer)?;
        Ok(Rc::new(RefCell::new(val)))
    }
}

impl<'de, T> DeserializeAs<'de, Rc<T>> for RcRefCellAsInner
where
    T: Deserialize<'de> + HasId + DefaultWithId,
{
    fn deserialize_as<D>(deserializer: D) -> Result<Rc<T>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let val = T::deserialize(deserializer)?;
        Ok(Rc::new(val))
    }
}
