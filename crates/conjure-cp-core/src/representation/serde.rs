use crate::ast::DeclarationPtr;
use crate::ast::serde::AsId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeAs, SerializeAs};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReprStateSerde;

impl<T> SerializeAs<T> for ReprStateSerde
where
    T: Serialize,
{
    fn serialize_as<S>(source: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        source.serialize(serializer)
    }
}

impl<'de, T> DeserializeAs<'de, T> for ReprStateSerde
where
    T: Deserialize<'de>,
{
    fn deserialize_as<D>(deserializer: D) -> Result<T, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize(deserializer)
    }
}

impl SerializeAs<DeclarationPtr> for ReprStateSerde {
    fn serialize_as<S>(source: &DeclarationPtr, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AsId::serialize_as(source, serializer)
    }
}

impl<'de> DeserializeAs<'de, DeclarationPtr> for ReprStateSerde {
    fn deserialize_as<D>(deserializer: D) -> Result<DeclarationPtr, D::Error>
    where
        D: Deserializer<'de>,
    {
        AsId::deserialize_as(deserializer)
    }
}
