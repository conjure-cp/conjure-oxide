use super::Name;
use funcmap::{FuncMap, TryFuncMap};
use std::cmp::Ordering;
use std::collections::VecDeque;

use polyquine::Quine;
use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Uniplate};

/// A named field of a record or variant.
/// Used in [AbstractLiteral::Record] / [AbstractLiteral::Variant] and
/// in corresponding domains
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash, Quine, FuncMap, TryFuncMap)]
#[path_prefix(conjure_cp::ast)]
pub struct Field<T> {
    pub name: Name,
    pub value: T,
}

impl<T: Eq> PartialOrd<Self> for Field<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Eq> Ord for Field<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

// Uniplate instance copy and pasted from cargo expand

// derive macro doesn't work as this has a generic type (for the same reasons as AbstractLiteral) ~nd60

impl<T> Uniplate for Field<T>
where
    T: Biplate<Field<T>>,
{
    fn uniplate(
        &self,
    ) -> (
        ::uniplate::Tree<Field<T>>,
        Box<dyn Fn(::uniplate::Tree<Field<T>>) -> Field<T>>,
    ) {
        let _name_copy = self.name.clone();
        let (tree_value, ctx_value) = <T as Biplate<Field<T>>>::biplate(&self.value);
        let children = ::uniplate::Tree::Many(VecDeque::from([tree_value, ::uniplate::Tree::Zero]));
        let ctx = Box::new(move |x: ::uniplate::Tree<Field<T>>| {
            let ::uniplate::Tree::Many(xs) = x else {
                panic!()
            };
            let tree_value = xs[0].clone();
            let value = ctx_value(tree_value);
            Field {
                name: _name_copy.clone(),
                value,
            }
        });
        (children, ctx)
    }
}

// want to be able to go anywhere U can go
// (I'll follow U wherever U will go)
impl<To, U> Biplate<To> for Field<U>
where
    U: Biplate<Field<U>> + Biplate<To>,
    To: Uniplate,
{
    fn biplate(&self) -> (uniplate::Tree<To>, Box<dyn Fn(uniplate::Tree<To>) -> Self>) {
        use uniplate::Tree;

        if std::any::TypeId::of::<To>() == std::any::TypeId::of::<Field<U>>() {
            // To ==From => return One(self)

            unsafe {
                // SAFETY: asserted the type equality above
                let self_to = std::mem::transmute::<&Field<U>, &To>(self).clone();
                let tree = Tree::One(self_to);
                let ctx = Box::new(move |x| {
                    let Tree::One(x) = x else {
                        panic!();
                    };

                    std::mem::transmute::<&To, &Field<U>>(&x).clone()
                });

                (tree, ctx)
            }
        } else if std::any::TypeId::of::<To>() == std::any::TypeId::of::<Name>() {
            // return name field, as well as any names inside the value
            let self2: Field<U> = self.clone();
            let f_name: Name = self2.name;
            let f_val: U = self2.value;

            let (tree_val, ctx_val) = <U as Biplate<To>>::biplate(&f_val);

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
                    let value = ctx_val(tree_val);

                    // reconstruct things
                    Field { name, value }
                });

                (tree, ctx)
            }
        } else {
            // walk into To ignoring name field, as Name can only biplate into Name

            let self2: Field<U> = self.clone();
            let f_name: Name = self2.name;
            let f_val: U = self2.value;

            let (tree_val, ctx_val) = <U as Biplate<To>>::biplate(&f_val);

            let tree = Tree::Many(VecDeque::from([tree_val]));

            let ctx = Box::new(move |x| {
                // deconstruct tree into tree_name and tree_val
                let Tree::Many(xs) = x else {
                    panic!();
                };

                let tree_val = xs[0].clone();

                // reconstruct things
                Field {
                    name: f_name.clone(),
                    value: ctx_val(tree_val),
                }
            });

            (tree, ctx)
        }
    }
}
