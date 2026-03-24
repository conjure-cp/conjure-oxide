use bimap::hash::{Iter, LeftValues, RightValues};
use bimap::{BiHashMap, Overwritten};
use funcmap::{FuncMap, TryFuncMap, TypeParam};
use polyquine::Quine;
use proc_macro2::TokenStream;
use quote::quote;
use serde::{Deserialize, Serialize};
use serde_with::de::DeserializeAsWrap;
use serde_with::ser::SerializeAsWrap;
use serde_with::serde_as;
use serde_with::{DeserializeAs, SerializeAs};
use std::borrow::Borrow;
use std::hash::Hash;

#[serde_as]
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
{
    inner: BiHashMap<L, R>,
}

impl<L, R> BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
{
    pub fn new() -> Self {
        Self {
            inner: BiHashMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: BiHashMap::with_capacity(capacity),
        }
    }

    pub fn get_by_left<Q>(&self, left: &Q) -> Option<&R>
    where
        L: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.inner.get_by_left(left)
    }

    pub fn get_by_right<Q>(&self, right: &Q) -> Option<&L>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.inner.get_by_right(right)
    }

    pub fn contains_left<Q>(&self, left: &Q) -> bool
    where
        L: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.inner.contains_left(left)
    }

    pub fn contains_right<Q>(&self, right: &Q) -> bool
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.inner.contains_right(right)
    }

    pub fn remove_by_left<Q>(&mut self, left: &Q) -> Option<(L, R)>
    where
        L: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.inner.remove_by_left(left)
    }

    pub fn remove_by_right<Q>(&mut self, right: &Q) -> Option<(L, R)>
    where
        R: Borrow<Q>,
        Q: Eq + Hash + ?Sized,
    {
        self.inner.remove_by_right(right)
    }

    pub fn insert(&mut self, left: L, right: R) -> Overwritten<L, R> {
        self.inner.insert(left, right)
    }

    pub fn insert_no_overwrite(&mut self, left: L, right: R) -> Result<(), (L, R)> {
        self.inner.insert_no_overwrite(left, right)
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&L, &R) -> bool,
    {
        self.inner.retain(f)
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn clear(&mut self) {
        self.inner.clear()
    }

    pub fn iter(&self) -> Iter<'_, L, R> {
        self.inner.iter()
    }

    pub fn left_values(&self) -> LeftValues<'_, L, R> {
        self.inner.left_values()
    }

    pub fn right_values(&self) -> RightValues<'_, L, R> {
        self.inner.right_values()
    }
}

impl<L, R> IntoIterator for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
{
    type Item = (L, R);
    type IntoIter = <BiHashMap<L, R> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl<L, R> FromIterator<(L, R)> for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
{
    fn from_iter<T: IntoIterator<Item = (L, R)>>(iter: T) -> Self {
        Self {
            inner: BiHashMap::from_iter(iter),
        }
    }
}

impl<L, R, const N: usize> From<[(L, R); N]> for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
{
    fn from(v: [(L, R); N]) -> Self {
        Self::from_iter(v)
    }
}

impl<L, R, RAs> SerializeAs<BiMap<L, R>> for BiMap<L, RAs>
where
    L: Eq + Hash,
    L: Serialize,
    R: Eq + Hash,
    RAs: SerializeAs<R> + Eq + Hash,
{
    fn serialize_as<S>(source: &BiMap<L, R>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let entries = source
            .iter()
            .map(|(left, right)| (left, SerializeAsWrap::<R, RAs>::new(right)))
            .collect::<Vec<_>>();
        entries.serialize(serializer)
    }
}

impl<'de, L, R, RAs> DeserializeAs<'de, BiMap<L, R>> for BiMap<L, RAs>
where
    L: Eq + Hash,
    R: Eq + Hash,
    L: Deserialize<'de>,
    RAs: DeserializeAs<'de, R> + Eq + Hash,
{
    fn deserialize_as<D>(deserializer: D) -> Result<BiMap<L, R>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let entries: Vec<(L, DeserializeAsWrap<R, RAs>)> = Vec::deserialize(deserializer)?;

        Ok(entries
            .into_iter()
            .map(|(left, right)| (left, right.into_inner()))
            .collect())
    }
}

impl<L, R, NewL> FuncMap<L, NewL, TypeParam<0>> for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
    NewL: Eq + Hash,
{
    type Output = BiMap<NewL, R>;

    fn func_map<F>(self, mut f: F) -> Self::Output
    where
        F: FnMut(L) -> NewL,
    {
        let itr = self.into_iter().map(|(k, v)| (f(k), v));
        Self::Output::from_iter(itr)
    }
}

impl<L, R, NewR> FuncMap<R, NewR, TypeParam<1>> for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
    NewR: Eq + Hash,
{
    type Output = BiMap<L, NewR>;

    fn func_map<F>(self, mut f: F) -> Self::Output
    where
        F: FnMut(R) -> NewR,
    {
        let itr = self.into_iter().map(|(k, v)| (k, f(v)));
        Self::Output::from_iter(itr)
    }
}

impl<L, R, NewL> TryFuncMap<L, NewL, TypeParam<0>> for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
    NewL: Eq + Hash,
{
    type Output = BiMap<NewL, R>;

    fn try_func_map<E, F>(self, mut f: F) -> Result<Self::Output, E>
    where
        F: FnMut(L) -> Result<NewL, E>,
    {
        let pairs: Result<Vec<(NewL, R)>, E> =
            self.into_iter().map(|(k, v)| Ok((f(k)?, v))).collect();

        Ok(Self::Output::from_iter(pairs?))
    }
}

impl<L, R, NewR> TryFuncMap<R, NewR, TypeParam<1>> for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
    NewR: Eq + Hash,
{
    type Output = BiMap<L, NewR>;

    fn try_func_map<E, F>(self, mut f: F) -> Result<Self::Output, E>
    where
        F: FnMut(R) -> Result<NewR, E>,
    {
        let pairs: Result<Vec<(L, NewR)>, E> =
            self.into_iter().map(|(k, v)| Ok((k, f(v)?))).collect();

        Ok(Self::Output::from_iter(pairs?))
    }
}

impl<L: Quine, R: Quine> Quine for BiMap<L, R>
where
    L: Eq + Hash,
    R: Eq + Hash,
{
    fn ctor_tokens(&self) -> TokenStream {
        let inner = self
            .iter()
            .map(|item| item.ctor_tokens())
            .collect::<Vec<_>>();
        quote! {
            BiMap::from([#(#inner),*])
        }
    }
}
