//! Serde serialization/ deserialization helpers.
//!
//! These are used in combination with the
//! [`serde_as`](https://docs.rs/serde_with/3.12.0/serde_with/index.html) annotation on AST types.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeAs, SerializeAs};
use std::cell::RefCell;
use std::fmt::Display;
use std::rc::Rc;
use ustr::Ustr;

/// A unique id, used to distinguish between objects of the same type.
///
///
/// This is used for pointer translation during (de)serialisation.
#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ObjId {
    /// a unique identifier of the type of this object
    pub type_name: Ustr,

    /// unique between objects of the same type
    pub object_id: u32,
}

impl Display for ObjId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "obj_id_{}_{}", self.type_name, self.object_id)
    }
}

/// A type with an [`ObjectId`].
///
/// Implementing types should ensure that the id is updated when an object is cloned.
pub trait HasId {
    const TYPE_NAME: &'static str;

    /// The id of this object.
    fn id(&self) -> ObjId;
}

/// A type that can be created with default values and an id.
pub trait DefaultWithId: HasId {
    /// Creates a new default value of type `T`, but with the given id.
    fn default_with_id(id: ObjId) -> Self;
}

/// A "fat pointer" to some shared data with an ID, such as [DeclarationPtr] or [SymbolTablePtr].
///
/// These are usually an [Arc] containing
/// - A unique, immutable ID
/// - A shared container for the data, such as an [RwLock]
///
/// Implementing this trait makes it possible to serialise the **contents** of such pointers
/// with [PtrAsInner]; See its docstring for more information.
pub(super) trait IdPtr: HasId + DefaultWithId {
    type Data: Serialize + for<'de> Deserialize<'de>;

    /// Get a copy of the underlying data
    fn get_data(&self) -> Self::Data;

    /// Re-construct the pointer given the ID and inner data
    fn with_id_and_data(id: ObjId, data: Self::Data) -> Self;
}

// TEMPORARY (so existing code builds);
// We will get rid of all bare Rc<RefCell>'s in the future!

impl<T> HasId for Rc<RefCell<T>>
where
    T: HasId,
{
    const TYPE_NAME: &'static str = T::TYPE_NAME;

    fn id(&self) -> ObjId {
        self.borrow().id()
    }
}

impl<T> DefaultWithId for Rc<RefCell<T>>
where
    T: DefaultWithId,
{
    fn default_with_id(id: ObjId) -> Self {
        Rc::new(RefCell::new(T::default_with_id(id)))
    }
}

/// Serialises the object's ID. The actual data is **NOT stored**.
///
/// # WARNING
///
/// The object is de-serialised with the correct ID, but an empty default value.
///
/// After de-serialising an entire structure, it is **the user's responsibility**
/// to find a complete copy of the object and restore the shared pointer to it.
/// This should be possible as long as at least one instance of the object has
/// been serialised with [PtrAsInner].
///
/// See the de-serialisation code for [Model] for an example.
///
pub struct AsId;

impl<T> SerializeAs<T> for AsId
where
    T: HasId,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.id().serialize(serializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for AsId
where
    T: HasId + DefaultWithId,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let id = ObjId::deserialize(deserializer)?;
        Ok(T::default_with_id(id))
    }
}

/// Serialises a shared pointer to some object `x` as a tuple `(ID, X)`, where:
/// - `ID` is the unique and immutable ID of the object. (See: [HasId])
/// - `X` is a full copy of the object's value.
///   (See `x`'s implementation of the [Serialize] trait for details)
///
/// On de-serialisation, **independent copies** of the object with the same ID are created.
///
/// # WARNING
///
/// De-serialisation makes **no attempt** to restore shared pointers.
/// Two pointers to the same value `x` will be de-serialised as **two separate copies**
/// of it, `x'` and `x''`. Mutating the value of `x'` will **not** change the value of `x''`.
///
/// After de-serialising an entire structure, it is the user's responsibility to go through it
/// and manually restore the shared pointers. See the de-serialisation code for [Model]
/// for an example.
pub struct PtrAsInner;

#[derive(Serialize, Deserialize)]
struct PtrAsInnerStored<T> {
    id: ObjId,
    #[serde(flatten)]
    data: T,
}

impl<T> SerializeAs<T> for PtrAsInner
where
    T: IdPtr,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let stored = PtrAsInnerStored {
            id: source.id(),
            data: source.get_data(),
        };
        stored.serialize(serializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for PtrAsInner
where
    T: IdPtr,
    T::Data: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        let stored = PtrAsInnerStored::deserialize(deserializer)?;
        Ok(T::with_id_and_data(stored.id, stored.data))
    }
}

// TODO: temporary, remove when we are no longer using
//       raw Rc<RefCell>s for symbol tables

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
