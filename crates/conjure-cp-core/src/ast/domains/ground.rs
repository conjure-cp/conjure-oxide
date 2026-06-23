use crate::ast::domains::attrs::PartitionAttr;
use crate::ast::domains::{MSetAttr, SequenceAttr};
use crate::ast::pretty::pretty_vec;
use crate::ast::{
    AbstractLiteral, DomainOpError, FuncAttr, HasDomain, Literal, Moo, Name, RelAttr, SetAttr,
    Typeable,
    domains::{domain::Int, range::Range},
    matrix,
    records::Field,
};
use crate::range;
use crate::utils::count_combinations;
use conjure_cp_core::ast::ReturnType;
use funcmap::FuncMap;
use itertools::{Itertools, izip};
use num_traits::ToPrimitive;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::iter::zip;
use uniplate::Uniplate;

pub(super) type FieldGround = Field<Moo<GroundDomain>>;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[path_prefix(conjure_cp::ast)]
pub enum GroundDomain {
    /// An empty domain of a given type
    Empty(ReturnType),
    /// A boolean value (true / false)
    Bool,
    /// An integer value in the given ranges (e.g. int(1, 3..5))
    Int(Vec<Range<Int>>),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<Int>, Moo<GroundDomain>),
    /// A multiset of elements drawn from the inner domain
    MSet(MSetAttr<Int>, Moo<GroundDomain>),
    /// An N-dimensional matrix of elements drawn from the inner domain,
    /// and indices from the n index domains
    Matrix(Moo<GroundDomain>, Vec<Moo<GroundDomain>>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<Moo<GroundDomain>>),
    /// A record
    Record(Vec<FieldGround>),
    /// A Partition
    Partition(PartitionAttr, Moo<GroundDomain>),
    /// A sequence of elements drawn from the inner domain
    Sequence(SequenceAttr, Moo<GroundDomain>),
    /// A function with a domain and codomain
    Function(FuncAttr, Moo<GroundDomain>, Moo<GroundDomain>),
    /// A relation as a set of tuples
    Relation(RelAttr, Vec<Moo<GroundDomain>>),
    /// A variant domain with its domain options (reusing field entries)
    Variant(Vec<FieldGround>),
}

impl GroundDomain {
    pub fn union(&self, other: &GroundDomain) -> Result<GroundDomain, DomainOpError> {
        match (self, other) {
            (GroundDomain::Empty(ty), dom) | (dom, GroundDomain::Empty(ty)) => {
                if *ty == dom.return_type() {
                    Ok(dom.clone())
                } else {
                    Err(DomainOpError::WrongType)
                }
            }
            (GroundDomain::Bool, GroundDomain::Bool) => Ok(GroundDomain::Bool),
            (GroundDomain::Bool, _) | (_, GroundDomain::Bool) => Err(DomainOpError::WrongType),
            (GroundDomain::Int(r1), GroundDomain::Int(r2)) => {
                let mut rngs = r1.clone();
                rngs.extend(r2.clone());
                Ok(GroundDomain::Int(Range::squeeze(&rngs)))
            }
            (GroundDomain::Int(_), _) | (_, GroundDomain::Int(_)) => Err(DomainOpError::WrongType),
            (GroundDomain::Set(_, in1), GroundDomain::Set(_, in2)) => Ok(GroundDomain::Set(
                SetAttr::default(),
                Moo::new(in1.union(in2)?),
            )),
            (GroundDomain::Set(_, _), _) | (_, GroundDomain::Set(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (GroundDomain::MSet(_, in1), GroundDomain::MSet(_, in2)) => Ok(GroundDomain::MSet(
                MSetAttr::default(),
                Moo::new(in1.union(in2)?),
            )),
            (GroundDomain::Matrix(in1, idx1), GroundDomain::Matrix(in2, idx2)) if idx1 == idx2 => {
                Ok(GroundDomain::Matrix(
                    Moo::new(in1.union(in2)?),
                    idx1.clone(),
                ))
            }
            (GroundDomain::Matrix(_, _), _) | (_, GroundDomain::Matrix(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (GroundDomain::Tuple(in1s), GroundDomain::Tuple(in2s)) if in1s.len() == in2s.len() => {
                let mut inners = Vec::new();
                for (in1, in2) in zip(in1s, in2s) {
                    inners.push(Moo::new(in1.union(in2)?));
                }
                Ok(GroundDomain::Tuple(inners))
            }
            (GroundDomain::Tuple(_), _) | (_, GroundDomain::Tuple(_)) => {
                Err(DomainOpError::WrongType)
            }
            (GroundDomain::Record(in1s), GroundDomain::Record(in2s))
                if in1s.len() == in2s.len() =>
            {
                let lhs_fields: BTreeMap<&Name, &Moo<GroundDomain>> =
                    in1s.iter().map(|x| (&x.name, &x.value)).collect();
                let rhs_fields: BTreeMap<&Name, &Moo<GroundDomain>> =
                    in2s.iter().map(|x| (&x.name, &x.value)).collect();
                let mut new_fields = Vec::with_capacity(in1s.len());
                for (n, d) in lhs_fields {
                    let d2 = rhs_fields.get(&n).ok_or(DomainOpError::WrongType)?;
                    let dom = d.union(d2)?;
                    new_fields.push(Field {
                        name: n.clone(),
                        value: dom.into(),
                    });
                }
                Ok(GroundDomain::Record(new_fields))
            }
            (GroundDomain::Record(_), _) | (_, GroundDomain::Record(_)) => {
                Err(DomainOpError::WrongType)
            }
            (GroundDomain::Relation(_, in1s), GroundDomain::Relation(_, in2s)) => {
                let mut inners = Vec::new();
                for (in1, in2) in zip(in1s, in2s) {
                    inners.push(Moo::new(in1.union(in2)?));
                }
                Ok(GroundDomain::Tuple(inners))
            }
            (GroundDomain::Relation(..), _) | (_, GroundDomain::Relation(..)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (GroundDomain::Sequence(_, _), _) | (_, GroundDomain::Sequence(_, _)) => {
                todo!("union sequence domains")
            }
            #[allow(unreachable_patterns)]
            (GroundDomain::Variant(_), _) | (_, GroundDomain::Variant(_)) => {
                todo!("union variant domains")
            }
            #[allow(unreachable_patterns)]
            (GroundDomain::Function(..), _) | (_, GroundDomain::Function(..)) => {
                todo!("union function domains")
            }
            #[allow(unreachable_patterns)]
            (GroundDomain::Partition(..), _) | (_, GroundDomain::Partition(..)) => {
                todo!("union partition domains")
            }
        }
    }

    /// Calculates the intersection of two domains.
    ///
    /// # Errors
    ///
    ///  - [`DomainOpError::Unbounded`] if either of the input domains are unbounded.
    ///  - [`DomainOpError::WrongType`] if the input domains are different types, or are not integer or set domains.
    pub fn intersect(&self, other: &GroundDomain) -> Result<GroundDomain, DomainOpError> {
        // TODO: does not consider unbounded domains yet
        // needs to be tested once comprehension rules are written

        match (self, other) {
            // one or more arguments is an empty int domain
            (d @ GroundDomain::Empty(ReturnType::Int), GroundDomain::Int(_)) => Ok(d.clone()),
            (GroundDomain::Int(_), d @ GroundDomain::Empty(ReturnType::Int)) => Ok(d.clone()),
            (GroundDomain::Empty(ReturnType::Int), d @ GroundDomain::Empty(ReturnType::Int)) => {
                Ok(d.clone())
            }

            // one or more arguments is an empty set(int) domain
            (GroundDomain::Set(_, inner1), d @ GroundDomain::Empty(ReturnType::Set(inner2)))
                if matches!(
                    **inner1,
                    GroundDomain::Int(_) | GroundDomain::Empty(ReturnType::Int)
                ) && matches!(**inner2, ReturnType::Int) =>
            {
                Ok(d.clone())
            }
            (d @ GroundDomain::Empty(ReturnType::Set(inner1)), GroundDomain::Set(_, inner2))
                if matches!(**inner1, ReturnType::Int)
                    && matches!(
                        **inner2,
                        GroundDomain::Int(_) | GroundDomain::Empty(ReturnType::Int)
                    ) =>
            {
                Ok(d.clone())
            }
            (
                d @ GroundDomain::Empty(ReturnType::Set(inner1)),
                GroundDomain::Empty(ReturnType::Set(inner2)),
            ) if matches!(**inner1, ReturnType::Int) && matches!(**inner2, ReturnType::Int) => {
                Ok(d.clone())
            }

            // both arguments are non-empy
            (GroundDomain::Set(_, x), GroundDomain::Set(_, y)) => Ok(GroundDomain::Set(
                SetAttr::default(),
                Moo::new((*x).intersect(y)?),
            )),

            (GroundDomain::Int(_), GroundDomain::Int(_)) => {
                let mut v: BTreeSet<i32> = BTreeSet::new();

                let v1 = self.values_i32()?;
                let v2 = other.values_i32()?;
                for value1 in v1.iter() {
                    if v2.contains(value1) && !v.contains(value1) {
                        v.insert(*value1);
                    }
                }
                Ok(GroundDomain::from_set_i32(&v))
            }
            (GroundDomain::Relation(_, _), GroundDomain::Relation(_, _)) => {
                todo!("Relation union not yet supported")
            }
            _ => Err(DomainOpError::WrongType),
        }
    }

    pub fn values(&self) -> Result<Box<dyn Iterator<Item = Literal>>, DomainOpError> {
        match self {
            GroundDomain::Empty(_) => Ok(Box::new(vec![].into_iter())),
            GroundDomain::Bool => Ok(Box::new(
                vec![Literal::from(false), Literal::from(true)].into_iter(),
            )),
            GroundDomain::Int(rngs) => {
                let rng_iters = rngs
                    .iter()
                    .map(Range::iter)
                    .collect::<Option<Vec<_>>>()
                    .ok_or(DomainOpError::Unbounded)?;
                Ok(Box::new(
                    rng_iters.into_iter().flat_map(|ri| ri.map(Literal::from)),
                ))
            }
            GroundDomain::Matrix(elem_dom, idx_doms) => {
                let shape = matrix::shape_of_dom(self)?;
                let idx_doms = idx_doms.clone();

                // Collect all possible element values
                let elem_values: Vec<Literal> = elem_dom.values()?.collect();

                // Generate all possible cell assignments in lexicographic order
                let iter = std::iter::repeat_n(elem_values, shape.size)
                    .multi_cartesian_product()
                    .map(move |flat_elems| {
                        matrix::unflatten_matrix::<Literal>(&flat_elems, &idx_doms, &shape.strides)
                    });

                Ok(Box::new(iter))
            }
            GroundDomain::Tuple(elem_doms) => {
                // Collect the possible values for each element
                let elem_value_pools: Vec<Vec<Literal>> = elem_doms
                    .iter()
                    .map(|d| d.values().map(|it| it.collect()))
                    .collect::<Result<_, _>>()?;

                // Generate all combinations in lexicographic order
                let iter = elem_value_pools
                    .into_iter()
                    .multi_cartesian_product()
                    .map(|elems| Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)));

                Ok(Box::new(iter))
            }
            GroundDomain::Record(entries) => {
                // Sort entries by name
                let mut sorted: Vec<&_> = entries.iter().collect();
                sorted.sort_by(|a, b| a.name.cmp(&b.name));

                let names: Vec<_> = sorted.iter().map(|e| e.name.clone()).collect();
                let value_pools: Vec<Vec<Literal>> = sorted
                    .iter()
                    .map(|e| e.value.values().map(|it| it.collect()))
                    .collect::<Result<_, _>>()?;

                // Generate all combinations in lexicographic order
                let iter = value_pools
                    .into_iter()
                    .multi_cartesian_product()
                    .map(move |vals| {
                        let record_entries = names
                            .iter()
                            .cloned()
                            .zip(vals)
                            .map(|(name, value)| Field { name, value })
                            .collect();
                        Literal::AbstractLiteral(AbstractLiteral::Record(record_entries))
                    });

                Ok(Box::new(iter))
            }
            GroundDomain::Set(..) => todo!("Enumerating set domains is not yet supported"),
            GroundDomain::MSet(..) => todo!("Enumerating multi-set domains is not yet supported"),
            GroundDomain::Function(..) => {
                todo!("Enumerating function domains is not yet supported")
            }
            GroundDomain::Partition(..) => {
                todo!("Enumerating partition domains is not yet supported")
            }
            GroundDomain::Relation(..) => {
                todo!("Enumerating relation domains is not yet supported")
            }
            GroundDomain::Sequence(..) => {
                todo!("Enumerating sequence domains is not yet supported")
            }
            GroundDomain::Variant(..) => {
                todo!("Enumerating variant domains is not yet supported")
            }
        }
    }

    /// Gets the length of this domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::Unbounded`] if the input domain is of infinite size.
    pub fn length(&self) -> Result<u64, DomainOpError> {
        match self {
            GroundDomain::Empty(_) => Ok(0),
            GroundDomain::Bool => Ok(2),
            GroundDomain::Int(ranges) => {
                if ranges.is_empty() {
                    return Err(DomainOpError::Unbounded);
                }

                let mut length = 0u64;
                for range in ranges {
                    if let Some(range_length) = range.length() {
                        length += range_length as u64;
                    } else {
                        return Err(DomainOpError::Unbounded);
                    }
                }
                Ok(length)
            }
            GroundDomain::Set(set_attr, inner_domain) => {
                let inner_len = inner_domain.length()?;
                let (min_sz, max_sz) = match set_attr.size {
                    Range::Unbounded => (0, inner_len),
                    Range::Single(n) => (n as u64, n as u64),
                    Range::UnboundedR(n) => (n as u64, inner_len),
                    Range::UnboundedL(n) => (0, n as u64),
                    Range::Bounded(min, max) => (min as u64, max as u64),
                };
                let mut ans = 0u64;
                for sz in min_sz..=max_sz {
                    let c = count_combinations(inner_len, sz)?;
                    ans = ans.checked_add(c).ok_or(DomainOpError::TooLarge)?;
                }
                Ok(ans)
            }
            GroundDomain::MSet(mset_attr, inner_domain) => {
                let inner_len = inner_domain.length()?;
                let (min_sz, max_sz) = match mset_attr.size {
                    Range::Unbounded => (0, inner_len),
                    Range::Single(n) => (n as u64, n as u64),
                    Range::UnboundedR(n) => (n as u64, inner_len),
                    Range::UnboundedL(n) => (0, n as u64),
                    Range::Bounded(min, max) => (min as u64, max as u64),
                };
                let mut ans = 0u64;
                for sz in min_sz..=max_sz {
                    // need  "multichoose", ((n  k)) == (n+k-1  k)
                    // Where n=inner_len and k=sz
                    let c = count_combinations(inner_len + sz - 1, sz)?;
                    ans = ans.checked_add(c).ok_or(DomainOpError::TooLarge)?;
                }
                Ok(ans)
            }
            GroundDomain::Sequence(_, _) => {
                // If jectivity is not set, the sequence can have any permutation.
                //
                todo!("Length bound currently not supported");
            }
            GroundDomain::Tuple(domains) => {
                let mut ans = 1u64;
                for domain in domains {
                    ans = ans
                        .checked_mul(domain.length()?)
                        .ok_or(DomainOpError::TooLarge)?;
                }
                Ok(ans)
            }
            GroundDomain::Record(entries) => {
                // A record is just a named tuple
                let mut ans = 1u64;
                for entry in entries {
                    let sz = entry.value.length()?;
                    ans = ans.checked_mul(sz).ok_or(DomainOpError::TooLarge)?;
                }
                Ok(ans)
            }
            GroundDomain::Matrix(inner_domain, idx_domains) => {
                let inner_sz = inner_domain.length()?;
                let exp = idx_domains.iter().try_fold(1u32, |acc, val| {
                    let len = val.length()? as u32;
                    acc.checked_mul(len).ok_or(DomainOpError::TooLarge)
                })?;
                inner_sz.checked_pow(exp).ok_or(DomainOpError::TooLarge)
            }
            GroundDomain::Function(_, _, _) => {
                todo!("Length bound of functions is not yet supported")
            }
            GroundDomain::Variant(entries) => {
                let mut ans = 1u64;
                for entry in entries {
                    let sz = entry.value.length()?;
                    // Only one field can be in the variant at once
                    ans = ans.checked_add(sz).ok_or(DomainOpError::TooLarge)?;
                }
                Ok(ans)
            }
            GroundDomain::Relation(_, domains) => {
                // Cannot currently use attributes to better infer length because of i32 u64 mismatch
                let dom_sizes_result: Result<Vec<u64>, DomainOpError> =
                    domains.iter().map(|x| x.length()).collect();
                let dom_sizes = dom_sizes_result?;
                Ok(dom_sizes.iter().product())
            }
            GroundDomain::Partition(_, _) => {
                todo!("Length bound of Partitions is not yet supported")
            }
        }
    }

    /// Get size of this domain as a [usize]
    pub fn len_usize(&self) -> Result<usize, DomainOpError> {
        self.length()?
            .try_into()
            .map_err(|_| DomainOpError::TooLarge)
    }

    pub fn contains(&self, lit: &Literal) -> Result<bool, DomainOpError> {
        // not adding a generic wildcard condition for all domains, so that this gives a compile
        // error when a domain is added.
        match self {
            // empty domain can't contain anything
            GroundDomain::Empty(_) => Ok(false),
            GroundDomain::Bool => match lit {
                Literal::Bool(_) => Ok(true),
                _ => Ok(false),
            },
            GroundDomain::Int(ranges) => match lit {
                Literal::Int(x) => {
                    // unconstrained int domain - contains all integers
                    if ranges.is_empty() {
                        return Ok(true);
                    };

                    Ok(ranges.iter().any(|range| range.contains(x)))
                }
                _ => Ok(false),
            },
            GroundDomain::Set(set_attr, inner_domain) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::Set(lit_elems)) => {
                    // check if the literal's size is allowed by the set attribute
                    let sz = lit_elems.len().to_i32().ok_or(DomainOpError::TooLarge)?;
                    if !set_attr.size.contains(&sz) {
                        return Ok(false);
                    }

                    for elem in lit_elems {
                        if !inner_domain.contains(elem)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            GroundDomain::MSet(mset_attr, inner_domain) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::MSet(lit_elems)) => {
                    // check if the literal's size is allowed by the mset attribute
                    let sz = lit_elems.len().to_i32().ok_or(DomainOpError::TooLarge)?;
                    if !mset_attr.size.contains(&sz) {
                        return Ok(false);
                    }

                    for elem in lit_elems {
                        if !inner_domain.contains(elem)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            GroundDomain::Sequence(seq_attr, inner_dom) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::Sequence(elems)) => {
                    let sz = elems.len().to_i32().ok_or(DomainOpError::TooLarge)?;
                    if !seq_attr.size.contains(&sz) {
                        return Ok(false);
                    }

                    for elem in elems {
                        if !inner_dom.contains(elem)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            GroundDomain::Matrix(elem_domain, index_domains) => {
                match lit {
                    Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx_domain)) => {
                        // Matrix literals are represented as nested 1d matrices, so the elements of
                        // the matrix literal will be the inner dimensions of the matrix.

                        let Some((current_index_domain, remaining_index_domains)) =
                            index_domains.split_first()
                        else {
                            panic!("a matrix should have at least one index domain");
                        };

                        if *current_index_domain != *idx_domain {
                            return Ok(false);
                        };

                        let next_elem_domain = if remaining_index_domains.is_empty() {
                            // Base case - we have a 1D row. Now check if all elements in the
                            // literal are in this row's element domain.
                            elem_domain.as_ref().clone()
                        } else {
                            // Otherwise, go down a dimension (e.g. 2D matrix inside a 3D tensor)
                            GroundDomain::Matrix(
                                elem_domain.clone(),
                                remaining_index_domains.to_vec(),
                            )
                        };

                        for elem in elems {
                            if !next_elem_domain.contains(elem)? {
                                return Ok(false);
                            }
                        }

                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            GroundDomain::Tuple(elem_domains) => {
                match lit {
                    Literal::AbstractLiteral(AbstractLiteral::Tuple(literal_elems)) => {
                        if elem_domains.len() != literal_elems.len() {
                            return Ok(false);
                        }

                        // for every element in the tuple literal, check if it is in the corresponding domain
                        for (elem_domain, elem) in itertools::izip!(elem_domains, literal_elems) {
                            if !elem_domain.contains(elem)? {
                                return Ok(false);
                            }
                        }

                        Ok(true)
                    }
                    _ => Ok(false),
                }
            }
            GroundDomain::Record(entries) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::Record(lit_entries)) => {
                    if entries.len() != lit_entries.len() {
                        return Ok(false);
                    }

                    for (entry, lit_entry) in itertools::izip!(entries, lit_entries) {
                        if entry.name != lit_entry.name
                            || !(entry.value.contains(&lit_entry.value)?)
                        {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            GroundDomain::Function(func_attr, domain, codomain) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::Function(lit_elems)) => {
                    let sz = Int::try_from(lit_elems.len()).expect("Should convert");
                    if !func_attr.size.contains(&sz) {
                        return Ok(false);
                    }
                    for lit in lit_elems {
                        let domain_element = &lit.0;
                        let codomain_element = &lit.1;
                        if !domain.contains(domain_element)? {
                            return Ok(false);
                        }
                        if !codomain.contains(codomain_element)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            GroundDomain::Variant(entries) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::Variant(lit_entry)) => {
                    for entry in entries {
                        if entry.name == lit_entry.name
                            && !(entry.value.contains(&lit_entry.value)?)
                        {
                            return Ok(true);
                        }
                    }
                    Ok(false)
                }
                _ => Ok(false),
            },
            GroundDomain::Relation(rel_attr, inner_domains) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::Relation(lit_elems)) => {
                    // check if the literal's size is allowed by the attributes
                    let sz = lit_elems.len().to_i32().ok_or(DomainOpError::TooLarge)?;
                    if !rel_attr.size.contains(&sz) {
                        return Ok(false);
                    }

                    for elem_tuple in lit_elems {
                        if elem_tuple.len() == inner_domains.len() {
                            for (elem, inner_dom) in elem_tuple.iter().zip(inner_domains.iter()) {
                                if !inner_dom.contains(elem)? {
                                    return Ok(false);
                                }
                            }
                        } else {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
            GroundDomain::Partition(attr, dom) => match lit {
                Literal::AbstractLiteral(AbstractLiteral::Partition(lit_elems)) => {
                    // let sz = lit_elems.len().to_i32().ok_or(DomainOpError::TooLarge)?;
                    let sz: i32 = lit_elems
                        .iter()
                        .flatten()
                        .count()
                        .to_i32()
                        .ok_or(DomainOpError::TooLarge)?;

                    let min: Option<i32> = match (attr.num_parts.low(), attr.part_len.low()) {
                        (Some(x), Some(y)) => Some(x * y),
                        _ => None,
                    };

                    let max: Option<i32> = match (attr.num_parts.high(), attr.part_len.high()) {
                        (Some(x), Some(y)) => Some(x * y),
                        _ => None,
                    };

                    let rng = Range::new(min, max);
                    if rng.contains(&sz) {
                        return Ok(false);
                    }

                    for elem in lit_elems.iter().flatten() {
                        if !dom.contains(elem)? {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                _ => Ok(false),
            },
        }
    }

    /// Returns a list of all possible values in an integer domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::NotInteger`] if the domain is not an integer domain.
    /// - [`DomainOpError::Unbounded`] if the domain is unbounded.
    pub fn values_i32(&self) -> Result<Vec<i32>, DomainOpError> {
        if let GroundDomain::Empty(ReturnType::Int) = self {
            return Ok(vec![]);
        }
        let GroundDomain::Int(ranges) = self else {
            return Err(DomainOpError::NotInteger(self.return_type()));
        };

        if ranges.is_empty() {
            return Err(DomainOpError::Unbounded);
        }

        let mut values = vec![];
        for range in ranges {
            match range {
                Range::Single(i) => {
                    values.push(*i);
                }
                Range::Bounded(i, j) => {
                    values.extend(*i..=*j);
                }
                Range::UnboundedR(_) | Range::UnboundedL(_) | Range::Unbounded => {
                    return Err(DomainOpError::Unbounded);
                }
            }
        }

        Ok(values)
    }

    /// Creates an [`Domain::Int`] containing the given integers.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_cp_core::ast::{GroundDomain, Range};
    /// use conjure_cp_core::{domain_int_ground,range};
    /// use std::collections::BTreeSet;
    ///
    /// let elements = BTreeSet::from([1,2,3,4,5]);
    ///
    /// let domain = GroundDomain::from_set_i32(&elements);
    ///
    /// assert_eq!(domain,domain_int_ground!(1..5));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{GroundDomain,Range};
    /// use conjure_cp_core::{domain_int_ground,range};
    /// use std::collections::BTreeSet;
    ///
    /// let elements = BTreeSet::from([1,2,4,5,7,8,9,10]);
    ///
    /// let domain = GroundDomain::from_set_i32(&elements);
    ///
    /// assert_eq!(domain,domain_int_ground!(1..2,4..5,7..10));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{GroundDomain,Range,ReturnType};
    /// use std::collections::BTreeSet;
    ///
    /// let elements = BTreeSet::from([]);
    ///
    /// let domain = GroundDomain::from_set_i32(&elements);
    ///
    /// assert!(matches!(domain,GroundDomain::Empty(ReturnType::Int)))
    /// ```
    pub fn from_set_i32(elements: &BTreeSet<i32>) -> GroundDomain {
        if elements.is_empty() {
            return GroundDomain::Empty(ReturnType::Int);
        }
        if elements.len() == 1 {
            return GroundDomain::Int(vec![Range::Single(*elements.first().unwrap())]);
        }

        let mut elems_iter = elements.iter().copied();

        let mut ranges: Vec<Range<i32>> = vec![];

        // Loop over the elements in ascending order, turning all sequential runs of
        // numbers into ranges.

        // the bounds of the current run of numbers.
        let mut lower = elems_iter
            .next()
            .expect("if we get here, elements should have => 2 elements");
        let mut upper = lower;

        for current in elems_iter {
            // As elements is a BTreeSet, current is always strictly larger than lower.

            if current == upper + 1 {
                // current is part of the current run - we now have the run lower..current
                //
                upper = current;
            } else {
                // the run lower..upper has ended.
                //
                // Add the run lower..upper to the domain, and start a new run.

                if lower == upper {
                    ranges.push(range!(lower));
                } else {
                    ranges.push(range!(lower..upper));
                }

                lower = current;
                upper = current;
            }
        }

        // add the final run to the domain
        if lower == upper {
            ranges.push(range!(lower));
        } else {
            ranges.push(range!(lower..upper));
        }

        ranges = Range::squeeze(&ranges);
        GroundDomain::Int(ranges)
    }

    /// Returns the domain that is the result of applying a binary operation to two integer domains.
    ///
    /// The given operator may return `None` if the operation is not defined for its arguments.
    /// Undefined values will not be included in the resulting domain.
    ///
    /// # Errors
    ///
    /// - [`DomainOpError::Unbounded`] if either of the input domains are unbounded.
    /// - [`DomainOpError::NotInteger`] if either of the input domains are not integers.
    pub fn apply_i32(
        &self,
        op: fn(i32, i32) -> Option<i32>,
        other: &GroundDomain,
    ) -> Result<GroundDomain, DomainOpError> {
        let vs1 = self.values_i32()?;
        let vs2 = other.values_i32()?;

        let mut set = BTreeSet::new();
        for (v1, v2) in itertools::iproduct!(vs1, vs2) {
            if let Some(v) = op(v1, v2) {
                set.insert(v);
            }
        }

        Ok(GroundDomain::from_set_i32(&set))
    }

    /// Returns true if the domain is finite.
    pub fn is_finite(&self) -> bool {
        for domain in self.universe() {
            if let GroundDomain::Int(ranges) = domain {
                if ranges.is_empty() {
                    return false;
                }

                if ranges.iter().any(|range| {
                    matches!(
                        range,
                        Range::UnboundedL(_) | Range::UnboundedR(_) | Range::Unbounded
                    )
                }) {
                    return false;
                }
            }
        }
        true
    }

    /// For a vector of literals, creates a domain that contains all the elements.
    ///
    /// The literals must all be of the same type.
    ///
    /// For abstract literals, this method merges the element domains of the literals, but not the
    /// index domains. Thus, for fixed-sized abstract literals (matrices, tuples, records, etc.),
    /// all literals in the vector must also have the same size / index domain:
    ///
    /// + Matrices: all literals must have the same index domain.
    /// + Tuples: all literals must have the same number of elements.
    /// + Records: all literals must have the same fields.
    ///
    /// # Errors
    ///
    /// - [DomainOpError::WrongType] if the input literals are of a different type to
    ///   each-other, as described above.
    ///
    /// # Examples
    ///
    /// ```
    /// use conjure_cp_core::ast::{Range, Literal, ReturnType, GroundDomain};
    ///
    /// let domain = GroundDomain::from_literal_vec(&vec![]);
    /// assert_eq!(domain,Ok(GroundDomain::Empty(ReturnType::Unknown)));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{GroundDomain,Range,Literal, AbstractLiteral};
    /// use conjure_cp_core::{domain_int_ground, range, matrix};
    ///
    /// // `[1,2;int(2..3)], [4,5; int(2..3)]` has domain
    /// // `matrix indexed by [int(2..3)] of int(1..2,4..5)`
    ///
    /// let matrix_1 = Literal::AbstractLiteral(matrix![Literal::Int(1),Literal::Int(2);domain_int_ground!(2..3)]);
    /// let matrix_2 = Literal::AbstractLiteral(matrix![Literal::Int(4),Literal::Int(5);domain_int_ground!(2..3)]);
    ///
    /// let domain = GroundDomain::from_literal_vec(&vec![matrix_1,matrix_2]);
    ///
    /// let expected_domain = Ok(GroundDomain::Matrix(
    ///     domain_int_ground!(1..2,4..5),vec![domain_int_ground!(2..3)]));
    ///
    /// assert_eq!(domain,expected_domain);
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{GroundDomain,Range,Literal, AbstractLiteral,DomainOpError};
    /// use conjure_cp_core::{domain_int_ground, range, matrix};
    ///
    /// // `[1,2;int(2..3)], [4,5; int(1..2)]` cannot be combined
    /// // `matrix indexed by [int(2..3)] of int(1..2,4..5)`
    ///
    /// let matrix_1 = Literal::AbstractLiteral(matrix![Literal::Int(1),Literal::Int(2);domain_int_ground!(2..3)]);
    /// let matrix_2 = Literal::AbstractLiteral(matrix![Literal::Int(4),Literal::Int(5);domain_int_ground!(1..2)]);
    ///
    /// let domain = GroundDomain::from_literal_vec(&vec![matrix_1,matrix_2]);
    ///
    /// assert_eq!(domain,Err(DomainOpError::WrongType));
    /// ```
    ///
    /// ```
    /// use conjure_cp_core::ast::{GroundDomain,Range,Literal, AbstractLiteral};
    /// use conjure_cp_core::{domain_int_ground,range, matrix};
    ///
    /// // `[[1,2; int(1..2)];int(2)], [[4,5; int(1..2)]; int(2)]` has domain
    /// // `matrix indexed by [int(2),int(1..2)] of int(1..2,4..5)`
    ///
    ///
    /// let matrix_1 = Literal::AbstractLiteral(matrix![Literal::AbstractLiteral(matrix![Literal::Int(1),Literal::Int(2);domain_int_ground!(1..2)]); domain_int_ground!(2)]);
    /// let matrix_2 = Literal::AbstractLiteral(matrix![Literal::AbstractLiteral(matrix![Literal::Int(4),Literal::Int(5);domain_int_ground!(1..2)]); domain_int_ground!(2)]);
    ///
    /// let domain = GroundDomain::from_literal_vec(&vec![matrix_1,matrix_2]);
    ///
    /// let expected_domain = Ok(GroundDomain::Matrix(
    ///     domain_int_ground!(1..2,4..5),
    ///     vec![domain_int_ground!(2),domain_int_ground!(1..2)]));
    ///
    /// assert_eq!(domain,expected_domain);
    /// ```
    ///
    ///
    pub fn from_literal_vec(literals: &[Literal]) -> Result<GroundDomain, DomainOpError> {
        // TODO: use proptest to test this better?

        if literals.is_empty() {
            return Ok(GroundDomain::Empty(ReturnType::Unknown));
        }

        let first_literal = literals.first().unwrap();

        match first_literal {
            Literal::Int(_) => {
                // check all literals are ints, then pass this to Domain::from_set_i32.
                let mut ints = BTreeSet::new();
                for lit in literals {
                    let Literal::Int(i) = lit else {
                        return Err(DomainOpError::WrongType);
                    };

                    ints.insert(*i);
                }

                Ok(GroundDomain::from_set_i32(&ints))
            }
            Literal::Bool(_) => {
                // check all literals are bools
                if literals.iter().any(|x| !matches!(x, Literal::Bool(_))) {
                    Err(DomainOpError::WrongType)
                } else {
                    Ok(GroundDomain::Bool)
                }
            }
            Literal::AbstractLiteral(AbstractLiteral::Set(_)) => {
                let mut all_elems = vec![];

                for lit in literals {
                    let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = lit else {
                        return Err(DomainOpError::WrongType);
                    };

                    all_elems.extend(elems.clone());
                }
                let elem_domain = GroundDomain::from_literal_vec(&all_elems)?;

                Ok(GroundDomain::Set(SetAttr::default(), Moo::new(elem_domain)))
            }
            Literal::AbstractLiteral(AbstractLiteral::MSet(_)) => {
                let mut all_elems = vec![];

                for lit in literals {
                    let Literal::AbstractLiteral(AbstractLiteral::MSet(elems)) = lit else {
                        return Err(DomainOpError::WrongType);
                    };

                    all_elems.extend(elems.clone());
                }
                let elem_domain = GroundDomain::from_literal_vec(&all_elems)?;

                Ok(GroundDomain::MSet(
                    MSetAttr::default(),
                    Moo::new(elem_domain),
                ))
            }
            Literal::AbstractLiteral(AbstractLiteral::Partition(_)) => {
                todo!("Need to figure out how this is going to work")
            }
            l @ Literal::AbstractLiteral(AbstractLiteral::Matrix(_, _)) => {
                let mut first_index_domain = vec![];
                // flatten index domains of n-d matrix into list
                let mut l = l.clone();
                while let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx)) = l {
                    assert!(
                        !matches!(idx.as_ref(), GroundDomain::Matrix(_, _)),
                        "n-dimensional matrix literals should be represented as a matrix inside a matrix"
                    );
                    first_index_domain.push(idx);
                    l = elems[0].clone();
                }

                let mut all_elems: Vec<Literal> = vec![];

                // check types and index domains
                for lit in literals {
                    let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx)) = lit else {
                        return Err(DomainOpError::NotGround);
                    };

                    all_elems.extend(elems.clone());

                    let mut index_domain = vec![idx.clone()];
                    let mut l = elems[0].clone();
                    while let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, idx)) = l {
                        assert!(
                            !matches!(idx.as_ref(), GroundDomain::Matrix(_, _)),
                            "n-dimensional matrix literals should be represented as a matrix inside a matrix"
                        );
                        index_domain.push(idx);
                        l = elems[0].clone();
                    }

                    if index_domain != first_index_domain {
                        return Err(DomainOpError::WrongType);
                    }
                }

                // extract all the terminal elements (those that are not nested matrix literals) from the matrix literal.
                let mut terminal_elements: Vec<Literal> = vec![];
                while let Some(elem) = all_elems.pop() {
                    if let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)) = elem {
                        all_elems.extend(elems);
                    } else {
                        terminal_elements.push(elem);
                    }
                }

                let element_domain = GroundDomain::from_literal_vec(&terminal_elements)?;

                Ok(GroundDomain::Matrix(
                    Moo::new(element_domain),
                    first_index_domain,
                ))
            }

            Literal::AbstractLiteral(AbstractLiteral::Tuple(first_elems)) => {
                let n_fields = first_elems.len();

                // for each field, calculate the element domain and add it to this list
                let mut elem_domains = vec![];

                for i in 0..n_fields {
                    let mut all_elems = vec![];
                    for lit in literals {
                        let Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)) = lit else {
                            return Err(DomainOpError::NotGround);
                        };

                        if elems.len() != n_fields {
                            return Err(DomainOpError::NotGround);
                        }

                        all_elems.push(elems[i].clone());
                    }

                    elem_domains.push(Moo::new(GroundDomain::from_literal_vec(&all_elems)?));
                }

                Ok(GroundDomain::Tuple(elem_domains))
            }

            Literal::AbstractLiteral(AbstractLiteral::Sequence(_)) => {
                let mut all_elems = vec![];

                for lit in literals {
                    let Literal::AbstractLiteral(AbstractLiteral::Sequence(elems)) = lit else {
                        return Err(DomainOpError::WrongType);
                    };

                    all_elems.extend(elems.clone());
                }
                let elem_domain = GroundDomain::from_literal_vec(&all_elems)?;

                Ok(GroundDomain::Sequence(
                    SequenceAttr::default(),
                    Moo::new(elem_domain),
                ))
            }

            Literal::AbstractLiteral(AbstractLiteral::Record(first_elems)) => {
                let n_fields = first_elems.len();
                let field_names = first_elems.iter().map(|x| x.name.clone()).collect_vec();

                // for each field, calculate the element domain and add it to this list
                let mut elem_domains = vec![];

                for i in 0..n_fields {
                    let mut all_elems = vec![];
                    for lit in literals {
                        let Literal::AbstractLiteral(AbstractLiteral::Record(elems)) = lit else {
                            return Err(DomainOpError::NotGround);
                        };

                        if elems.len() != n_fields {
                            return Err(DomainOpError::NotGround);
                        }

                        let elem = elems[i].clone();
                        if elem.name != field_names[i] {
                            return Err(DomainOpError::NotGround);
                        }

                        all_elems.push(elem.value);
                    }

                    elem_domains.push(Moo::new(GroundDomain::from_literal_vec(&all_elems)?));
                }

                Ok(GroundDomain::Record(
                    izip!(field_names, elem_domains)
                        .map(|(name, value)| FieldGround { name, value })
                        .collect(),
                ))
            }
            Literal::AbstractLiteral(AbstractLiteral::Function(items)) => {
                if items.is_empty() {
                    return Err(DomainOpError::NotGround);
                }

                let (x1, y1) = &items[0];
                let d1 = x1.domain_of();
                let d1 = d1.as_ground().ok_or(DomainOpError::NotGround)?;
                let d2 = y1.domain_of();
                let d2 = d2.as_ground().ok_or(DomainOpError::NotGround)?;

                // Check that all items have the same domains
                for (x, y) in items {
                    let dx = x.domain_of();
                    let dx = dx.as_ground().ok_or(DomainOpError::NotGround)?;

                    let dy = y.domain_of();
                    let dy = dy.as_ground().ok_or(DomainOpError::NotGround)?;

                    if (dx != d1) || (dy != d2) {
                        return Err(DomainOpError::WrongType);
                    }
                }

                todo!();
            }
            Literal::AbstractLiteral(AbstractLiteral::Variant(_)) => {
                todo!();
            }
            Literal::AbstractLiteral(AbstractLiteral::Relation(_)) => {
                todo!();
            }
        }
    }

    pub fn element_domain(&self) -> Option<Moo<GroundDomain>> {
        match self {
            GroundDomain::Set(_, inner) => Some(inner.clone()),
            GroundDomain::MSet(_, inner) => Some(inner.clone()),
            GroundDomain::Matrix(_, _) => todo!("Unwrap one dimension of the domain"),
            _ => None,
        }
    }
}

impl Typeable for GroundDomain {
    fn return_type(&self) -> ReturnType {
        match self {
            GroundDomain::Empty(ty) => ty.clone(),
            GroundDomain::Bool => ReturnType::Bool,
            GroundDomain::Int(_) => ReturnType::Int,
            GroundDomain::Set(_attr, inner) => ReturnType::Set(Box::new(inner.return_type())),
            GroundDomain::MSet(_attr, inner) => ReturnType::MSet(Box::new(inner.return_type())),
            GroundDomain::Sequence(_attr, inner) => {
                ReturnType::Sequence(Box::new(inner.return_type()))
            }
            GroundDomain::Matrix(inner, _idx) => ReturnType::Matrix(Box::new(inner.return_type())),
            GroundDomain::Tuple(inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Tuple(inner_types)
            }
            GroundDomain::Function(_, dom, cdom) => {
                ReturnType::Function(Box::new(dom.return_type()), Box::new(cdom.return_type()))
            }
            GroundDomain::Record(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.clone().func_map(|x| x.return_type()));
                }
                ReturnType::Record(entry_types)
            }
            GroundDomain::Variant(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.clone().func_map(|x| x.return_type()));
                }
                ReturnType::Variant(entry_types)
            }
            GroundDomain::Relation(_, inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Relation(inner_types)
            }
            GroundDomain::Partition(_, inner) => ReturnType::Set(Box::new(inner.return_type())),
        }
    }
}

impl Display for FieldGround {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.value)
    }
}

impl Display for GroundDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            GroundDomain::Empty(ty) => write!(f, "empty({ty})"),
            GroundDomain::Bool => write!(f, "bool"),
            GroundDomain::Int(ranges) => {
                if ranges.iter().all(Range::is_lower_or_upper_bounded) {
                    let rngs: String = ranges.iter().map(|r| format!("{r}")).join(", ");
                    write!(f, "int({})", rngs)
                } else {
                    write!(f, "int")
                }
            }
            GroundDomain::Set(attrs, inner_dom) => write!(f, "set {attrs} of {inner_dom}"),
            GroundDomain::MSet(attrs, inner_dom) => write!(f, "mset {attrs} of {inner_dom}"),
            GroundDomain::Sequence(attrs, inner_dom) => {
                write!(f, "sequence {attrs} of {inner_dom}")
            }
            GroundDomain::Matrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by {} of {value_domain}",
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
            GroundDomain::Tuple(domains) => {
                write!(f, "tuple ({})", &domains.iter().join(", "))
            }
            GroundDomain::Record(entries) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "record {{{inners}}}",)
            }
            GroundDomain::Variant(entries) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "variant {{{inners}}}",)
            }
            GroundDomain::Function(attribute, domain, codomain) => {
                write!(f, "function {} {} --> {} ", attribute, domain, codomain)
            }
            GroundDomain::Relation(attrs, domains) => {
                write!(f, "relation {} of ({})", attrs, domains.iter().join(" * "))
            }
            GroundDomain::Partition(attrs, inner) => {
                write!(f, "partition {attrs} from {inner}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Name;
    use crate::{domain_int_ground, matrix_lit};

    #[test]
    fn matrix_values_1d_bool_of_bool() {
        // matrix indexed by [bool] of bool
        // 2 cells, 2 possible values => 2^2 = 4 matrices
        let dom = GroundDomain::Matrix(
            Moo::new(GroundDomain::Bool),
            vec![Moo::new(GroundDomain::Bool)],
        );

        let values: Vec<Literal> = dom.values().unwrap().collect();

        assert_eq!(values.len(), 4);
        assert_eq!(
            values[0],
            matrix_lit![false, false; Moo::new(GroundDomain::Bool)]
        );
        assert_eq!(
            values[1],
            matrix_lit![false, true; Moo::new(GroundDomain::Bool)]
        );
        assert_eq!(
            values[2],
            matrix_lit![true, false; Moo::new(GroundDomain::Bool)]
        );
        assert_eq!(
            values[3],
            matrix_lit![true, true; Moo::new(GroundDomain::Bool)]
        );
    }

    #[test]
    fn matrix_values_1d_int() {
        // matrix indexed by [int(1..2)] of int(0..1)
        // 2 cells, 2 possible values => 4 matrices
        let dom = GroundDomain::Matrix(domain_int_ground!(0..1), vec![domain_int_ground!(1..2)]);

        let values: Vec<Literal> = dom.values().unwrap().collect();

        assert_eq!(values.len(), 4);
        assert_eq!(values[0], matrix_lit![0, 0; domain_int_ground!(1..2)]);
        assert_eq!(values[1], matrix_lit![0, 1; domain_int_ground!(1..2)]);
        assert_eq!(values[2], matrix_lit![1, 0; domain_int_ground!(1..2)]);
        assert_eq!(values[3], matrix_lit![1, 1; domain_int_ground!(1..2)]);
    }

    #[test]
    fn matrix_values_2d_lexicographic() {
        // matrix indexed by [int(1..2), int(1..2)] of int(0..1)
        // 4 cells, 2 possible values => 2^4 = 16 matrices
        let dom = GroundDomain::Matrix(
            domain_int_ground!(0..1),
            vec![domain_int_ground!(1..2), domain_int_ground!(1..2)],
        );

        let values: Vec<Literal> = dom.values().unwrap().collect();

        assert_eq!(values.len(), 16);

        // First: [[0,0],[0,0]]
        assert_eq!(
            values[0],
            matrix_lit![[0, 0], [0, 0]; [domain_int_ground!(1..2), domain_int_ground!(1..2)]]
        );
        // Second: [[0,0],[0,1]]
        assert_eq!(
            values[1],
            matrix_lit![[0, 0], [0, 1]; [domain_int_ground!(1..2), domain_int_ground!(1..2)]]
        );
        // Third: [[0,0],[1,0]]
        assert_eq!(
            values[2],
            matrix_lit![[0, 0], [1, 0]; [domain_int_ground!(1..2), domain_int_ground!(1..2)]]
        );
        // Fourth: [[0,0],[1,1]]
        assert_eq!(
            values[3],
            matrix_lit![[0, 0], [1, 1]; [domain_int_ground!(1..2), domain_int_ground!(1..2)]]
        );
        // Last: [[1,1],[1,1]]
        assert_eq!(
            values[15],
            matrix_lit![[1, 1], [1, 1]; [domain_int_ground!(1..2), domain_int_ground!(1..2)]]
        );
    }

    #[test]
    fn matrix_values_count_matches_length() {
        // matrix indexed by [int(1..3)] of int(0..1)
        // 3 cells, 2 possible values => 2^3 = 8 matrices
        let dom = GroundDomain::Matrix(domain_int_ground!(0..1), vec![domain_int_ground!(1..3)]);

        let count = dom.values().unwrap().count();
        let length = dom.length().unwrap();

        assert_eq!(count as u64, length);
    }

    #[test]
    fn tuple_values_two_bools() {
        // tuple of (bool, bool) => 2*2 = 4 values
        let dom = GroundDomain::Tuple(vec![
            Moo::new(GroundDomain::Bool),
            Moo::new(GroundDomain::Bool),
        ]);

        let values: Vec<Literal> = dom.values().unwrap().collect();

        assert_eq!(values.len(), 4);
        let t = |a, b| {
            Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
                Literal::Bool(a),
                Literal::Bool(b),
            ]))
        };
        assert_eq!(values[0], t(false, false));
        assert_eq!(values[1], t(false, true));
        assert_eq!(values[2], t(true, false));
        assert_eq!(values[3], t(true, true));
    }

    #[test]
    fn tuple_values_mixed_domains() {
        // tuple of (bool, int(0..2)) => 2*3 = 6 values, lexicographic
        let dom = GroundDomain::Tuple(vec![Moo::new(GroundDomain::Bool), domain_int_ground!(0..2)]);

        let values: Vec<Literal> = dom.values().unwrap().collect();

        assert_eq!(values.len(), 6);
        let t = |b: bool, i: i32| {
            Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
                Literal::Bool(b),
                Literal::Int(i),
            ]))
        };
        // bool false first, then ints 0,1,2
        assert_eq!(values[0], t(false, 0));
        assert_eq!(values[1], t(false, 1));
        assert_eq!(values[2], t(false, 2));
        // then bool true
        assert_eq!(values[3], t(true, 0));
        assert_eq!(values[4], t(true, 1));
        assert_eq!(values[5], t(true, 2));
    }

    #[test]
    fn tuple_values_count_matches_length() {
        let dom = GroundDomain::Tuple(vec![
            domain_int_ground!(1..3),
            Moo::new(GroundDomain::Bool),
            domain_int_ground!(0..1),
        ]);
        let count = dom.values().unwrap().count();
        let length = dom.length().unwrap();
        assert_eq!(count as u64, length);
    }

    #[test]
    fn record_values_lexicographic_by_name() {
        // record {b: bool, a: int(0..1)}
        // Entries should be ordered by name: a first, then b
        let dom = GroundDomain::Record(vec![
            Field {
                name: Name::user("b"),
                value: Moo::new(GroundDomain::Bool),
            },
            Field {
                name: Name::user("a"),
                value: domain_int_ground!(0..1),
            },
        ]);

        let values: Vec<Literal> = dom.values().unwrap().collect();

        // 2 * 2 = 4 values
        assert_eq!(values.len(), 4);

        // Entries should be sorted by name: "a" before "b"
        let r = |a_val: i32, b_val: bool| {
            Literal::AbstractLiteral(AbstractLiteral::Record(vec![
                Field {
                    name: Name::user("a"),
                    value: Literal::Int(a_val),
                },
                Field {
                    name: Name::user("b"),
                    value: Literal::Bool(b_val),
                },
            ]))
        };

        // "a" (int) varies slowest, "b" (bool) varies fastest
        assert_eq!(values[0], r(0, false));
        assert_eq!(values[1], r(0, true));
        assert_eq!(values[2], r(1, false));
        assert_eq!(values[3], r(1, true));
    }

    #[test]
    fn record_values_count_matches_length() {
        let dom = GroundDomain::Record(vec![
            Field {
                name: Name::user("x"),
                value: domain_int_ground!(1..3),
            },
            Field {
                name: Name::user("y"),
                value: Moo::new(GroundDomain::Bool),
            },
        ]);
        let count = dom.values().unwrap().count();
        let length = dom.length().unwrap();
        assert_eq!(count as u64, length);
    }
}
