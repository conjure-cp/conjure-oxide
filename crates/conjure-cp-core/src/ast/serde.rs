//! Serde serialization/ deserialization helpers.
//!
//! These are used in combination with the
//! [`serde_as`](https://docs.rs/serde_with/3.12.0/serde_with/index.html) annotation on AST types.

use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;
use serde::de::Deserialize;
use serde::de::Error;
use serde_with::{DeserializeAs, SerializeAs};

/// A unique id, used to distinguish between objects of the same type.
///
///
/// This is used for pointer translation during (de)serialisation.
pub type ObjId = u32;

/// A type with an [`ObjectId`].
///
/// Each object of the implementing type has a unique id; however, ids are not unique for different
/// type of objects.
///
/// Implementing types should ensure that the id is updated when an object is cloned.
pub trait HasId {
    /// The id of this object.
    ///
    /// Each object of this type has a unique id; however, ids are not unique for different type of
    /// objects.
    fn id(&self) -> ObjId;
}

/// A type that can be created with default values and an id.
pub trait DefaultWithId: HasId {
    /// Creates a new default value of type `T`, but with the given id.
    fn default_with_id(id: ObjId) -> Self;
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
        let id = (**source).borrow().id();
        serializer.serialize_u32(id)
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
        let id = u32::deserialize(deserializer).map_err(Error::custom)?;
        Ok(Rc::new(RefCell::new(T::default_with_id(id))))
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
