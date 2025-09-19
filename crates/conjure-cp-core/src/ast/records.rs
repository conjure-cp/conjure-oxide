use std::collections::VecDeque;

use super::literals::AbstractLiteralValue;
use super::{Domain, Name};
use serde::{Deserialize, Serialize};

use polyquine::Quine;
use uniplate::{Biplate, Uniplate};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct RecordEntry {
    pub name: Name,
    pub domain: Domain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct RecordValue<T: AbstractLiteralValue> {
    pub name: Name,
    pub value: T,
}

// Uniplate instance copy and pasted from cargo expand

// derive macro doesn't work as this has a generic type (for the same reasons as AbstractLiteral) ~nd60

impl<T> Uniplate for RecordValue<T>
where
    T: AbstractLiteralValue,
{
    fn uniplate(
        &self,
    ) -> (
        ::uniplate::Tree<RecordValue<T>>,
        Box<dyn Fn(::uniplate::Tree<RecordValue<T>>) -> RecordValue<T>>,
    ) {
        let _name_copy = self.name.clone();
        let (tree_value, ctx_value) = <T as Biplate<RecordValue<T>>>::biplate(&self.value);
        let children = ::uniplate::Tree::Many(::std::collections::VecDeque::from([
            tree_value,
            ::uniplate::Tree::Zero,
        ]));
        let ctx = Box::new(move |x: ::uniplate::Tree<RecordValue<T>>| {
            let ::uniplate::Tree::Many(xs) = x else {
                panic!()
            };
            let tree_value = xs[0].clone();
            let value = ctx_value(tree_value);
            RecordValue {
                name: _name_copy.clone(),
                value,
            }
        });
        (children, ctx)
    }
}

// want to be able to go anywhere U can go
// (I'll follow U wherever U will go)
impl<To, U> Biplate<To> for RecordValue<U>
where
    U: AbstractLiteralValue + Biplate<To>,
    To: Uniplate,
{
    fn biplate(&self) -> (uniplate::Tree<To>, Box<dyn Fn(uniplate::Tree<To>) -> Self>) {
        use uniplate::Tree;

        if std::any::TypeId::of::<To>() == std::any::TypeId::of::<RecordValue<U>>() {
            // To ==From => return One(self)

            unsafe {
                // SAFETY: asserted the type equality above
                let self_to = std::mem::transmute::<&RecordValue<U>, &To>(self).clone();
                let tree = Tree::One(self_to);
                let ctx = Box::new(move |x| {
                    let Tree::One(x) = x else {
                        panic!();
                    };

                    std::mem::transmute::<&To, &RecordValue<U>>(&x).clone()
                });

                (tree, ctx)
            }
        } else if std::any::TypeId::of::<To>() == std::any::TypeId::of::<Name>() {
            // return name field, as well as any names inside the value
            let self2: RecordValue<U> = self.clone();
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
                    RecordValue { name, value }
                });

                (tree, ctx)
            }
        } else {
            // walk into To ignoring name field, as Name can only biplate into Name

            let self2: RecordValue<U> = self.clone();
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
                RecordValue {
                    name: f_name.clone(),
                    value: ctx_val(tree_val),
                }
            });

            (tree, ctx)
        }
    }
}
