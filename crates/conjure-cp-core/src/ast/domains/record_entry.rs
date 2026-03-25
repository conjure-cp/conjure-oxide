use crate::ast::{Domain, DomainOpError, GroundDomain, Moo, Name};
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uniplate::{Biplate, Uniplate};

/// The only 2 things that make sense inside RecordEntry
pub trait IsDomain:
    Clone + Eq + PartialEq + Uniplate + Biplate<RecordEntry<Self>> + 'static
{
}
impl IsDomain for GroundDomain {}
impl IsDomain for Domain {}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct RecordEntry<T>
where
    T: IsDomain,
{
    pub name: Name,
    pub domain: Moo<T>,
}

impl RecordEntry<Domain> {
    pub fn resolve(self) -> Result<RecordEntry<GroundDomain>, DomainOpError> {
        Ok(RecordEntry {
            name: self.name,
            domain: self.domain.resolve()?,
        })
    }
}

impl From<RecordEntry<GroundDomain>> for RecordEntry<Domain> {
    fn from(entry: RecordEntry<GroundDomain>) -> Self {
        Self {
            name: entry.name,
            domain: entry.domain.into(),
        }
    }
}

impl TryFrom<RecordEntry<Domain>> for RecordEntry<GroundDomain> {
    type Error = DomainOpError;

    fn try_from(entry: RecordEntry<Domain>) -> Result<Self, Self::Error> {
        let dom_gd: GroundDomain = entry.domain.try_into()?;
        Ok(Self {
            name: entry.name,
            domain: Moo::new(dom_gd),
        })
    }
}

// Copied verbatim from the impl for RecordValue; credit TAswan and nd60
impl<T> Uniplate for RecordEntry<T>
where
    T: IsDomain,
{
    fn uniplate(
        &self,
    ) -> (
        ::uniplate::Tree<RecordEntry<T>>,
        Box<dyn Fn(::uniplate::Tree<RecordEntry<T>>) -> RecordEntry<T>>,
    ) {
        let _name_copy = self.name.clone();
        let (tree_domain, ctx_domain) = <T as Biplate<RecordEntry<T>>>::biplate(&self.domain);
        let children = ::uniplate::Tree::Many(::std::collections::VecDeque::from([
            tree_domain,
            ::uniplate::Tree::Zero,
        ]));
        let ctx = Box::new(move |x: ::uniplate::Tree<RecordEntry<T>>| {
            let ::uniplate::Tree::Many(xs) = x else {
                panic!()
            };
            let tree_domain = xs[0].clone();
            let domain = Moo::new(ctx_domain(tree_domain));
            RecordEntry {
                name: _name_copy.clone(),
                domain,
            }
        });
        (children, ctx)
    }
}

// want to be able to go anywhere U can go
// .. I'll follow `U` way down to Ur deepest low
//    I'll always be around wherever life takes `U`..
impl<To, U> Biplate<To> for RecordEntry<U>
where
    U: IsDomain + Biplate<To>,
    To: Uniplate,
{
    fn biplate(&self) -> (uniplate::Tree<To>, Box<dyn Fn(uniplate::Tree<To>) -> Self>) {
        use uniplate::Tree;

        if std::any::TypeId::of::<To>() == std::any::TypeId::of::<RecordEntry<U>>() {
            // To ==From => return One(self)

            unsafe {
                // SAFETY: asserted the type equality above
                let self_to = std::mem::transmute::<&RecordEntry<U>, &To>(self).clone();
                let tree = Tree::One(self_to);
                let ctx = Box::new(move |x| {
                    let Tree::One(x) = x else {
                        panic!();
                    };

                    std::mem::transmute::<&To, &RecordEntry<U>>(&x).clone()
                });

                (tree, ctx)
            }
        } else if std::any::TypeId::of::<To>() == std::any::TypeId::of::<Name>() {
            // return name field, as well as any names inside the domain
            let self2: RecordEntry<U> = self.clone();
            let f_name: Name = self2.name;
            let f_val: Moo<U> = self2.domain;

            let (tree_val, ctx_val) = <Moo<U> as Biplate<To>>::biplate(&f_val);

            unsafe {
                // SAFETY: asserted previously that To == Name
                let f_name_to = std::mem::transmute::<&Name, &To>(&f_name).clone();
                let tree_name = Tree::One(f_name_to);
                let tree = Tree::Many(VecDeque::from([tree_name, tree_val]));

                let ctx = Box::new(move |x| {
                    // deconstruct tree into tree_name and tree_val
                    let Tree::Many(xs) = x else {
                        panic!();
                    };

                    let tree_name = xs[0].clone();
                    let tree_val = xs[1].clone();

                    let Tree::One(name) = tree_name else {
                        panic!();
                    };

                    // SAFETY: asserted previously that To == Name
                    let name = std::mem::transmute::<&To, &Name>(&name).clone();
                    let domain = ctx_val(tree_val);

                    // reconstruct things
                    RecordEntry { name, domain }
                });

                (tree, ctx)
            }
        } else {
            // walk into To ignoring name field, as Name can only biplate into Name

            let self2: RecordEntry<U> = self.clone();
            let f_name: Name = self2.name;
            let f_val: Moo<U> = self2.domain;

            let (tree_val, ctx_val) = <Moo<U> as Biplate<To>>::biplate(&f_val);

            let tree = Tree::Many(VecDeque::from([tree_val]));

            let ctx = Box::new(move |x| {
                // deconstruct tree into tree_name and tree_val
                let Tree::Many(xs) = x else {
                    panic!();
                };

                let tree_val = xs[0].clone();

                // reconstruct things
                RecordEntry {
                    name: f_name.clone(),
                    domain: ctx_val(tree_val),
                }
            });

            (tree, ctx)
        }
    }
}
