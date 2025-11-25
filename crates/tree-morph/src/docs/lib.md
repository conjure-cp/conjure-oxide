# tree-morph

**Tree-morph is a library that helps you perform boilerplate-free generic tree transformations.**

## Quick Example

In this simple example, we use tree-morph to calculate mathematical expressions using multiplication, addition and squaring.

```rust
use tree_morph::prelude::*;
use uniplate::Uniplate;


#[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
#[uniplate()]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Sqr(Box<Expr>, Box<Expr>),
    Val(i32),
}
```
Tree-morph makes use of the [Uniplate crate](https://crates.io/crates/uniplate/0.2.1), which allows for boilerplate-free tree traversals. By recursively nesting these four expressions, we can build any mathematical expression involving addition, multiplication and raising to integer powers. For example, the expression ``(1+2)^2`` can be written as:

```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
let my_expression = Expr::Sqr(
    Box::new(Expr::Add(
        Box::new(Expr::Val(1)),
        Box::new(Expr::Val(2))
    ))
);
```
Now we know how to create expressions, we have to also create rules that transform expressions. The following functions provide addition and multiplication rules for our tree.
```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
fn rule_eval_add(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
   if let Expr::Add(a, b) = subtree {
       if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
           return Some(Expr::Val(a_v + b_v));
       }
   }
   None
}

fn rule_eval_mul(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
   if let Expr::Mul(a, b) = subtree {
       if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
           return Some(Expr::Val(a_v * b_v));
       }
   }
   None
}
```
We will talk about the the ``Commands<Expr, i32>`` and ``meta`` inputs later, but for now we view the add/mul rules as checking if a tree node has the right enum variant (in this case ``Expr::Add`` or ``Expr::Mul``) and children (two ``Expr::Val`` variants), and adding/multiplying if possible, otherwise retuning ``None``.

We can defining the squaring rule in a similar way.
```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
fn rule_expand_sqr(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
   if let Expr::Sqr(expr) = subtree {
       return Some(Expr::Mul(
           Box::new(*expr.clone()),
           Box::new(*expr.clone())
       ));
   }
   None
}
```
Now we have everything in place to start using tree-morph to apply our transformation rules to evaluate expressions. The following ``#[test]`` block checks that ``my_expression`` does indeed hold a value of ``9``.

```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
# fn rule_eval_add(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
#     if let Expr::Add(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             return Some(Expr::Val(a_v + b_v));
#         }
#     }
#     None
# }
#
# fn rule_eval_mul(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
#     if let Expr::Mul(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             return Some(Expr::Val(a_v * b_v));
#         }
#     }
#     None
# }
# fn rule_expand_sqr(cmds: &mut Commands<Expr, i32>, subtree: &Expr, meta: &i32) -> Option<Expr> {
#     if let Expr::Sqr(expr) = subtree {
#         return Some(Expr::Mul(
#             Box::new(*expr.clone()),
#             Box::new(*expr.clone())
#         ));
#     }
#     None
# }
#[test]
fn check_my_expression() {
   let my_expression = Expr::Sqr(Box::new(Expr::Add(
       Box::new(Expr::Val(1)),
       Box::new(Expr::Val(2)),
   )));

   let (result, _) = morph(
       vec![rule_fns![rule_eval_add, rule_eval_mul, rule_expand_sqr]],
       tree_morph::helpers::select_panic,
       my_expression.clone(),
       0,
   );
   assert_eq!(result, Expr::Val(9));
}
```
The ``morph`` function is the core function of the crate, and handles the bulk application of transformation rules to the tree. We input the set of rules, a decision function, and some metadata, returning a ``(tree, metadata)`` tuple. Running ``cargo test --test file_name`` in a directory containing the file with this code will verify that tree-morph is indeed doing what it should do! In the following sections we explore some of tree-morph's features in more depth.

## Metadata
Metadata refers to additional contextual information passed along during the tree transformation process. A simple usage of metadata is counting the number of transformation steps a process takes. We keep track of metadata via the ``Commands`` struct, which is used to capture any other side-effects to the transformation process apart from the pure rule changes. If, in our above example, we wanted to count the number of addition rule changes applies, we would first need to create a new struct to capture the metadata.

```rust
// --snip--
struct Meta {
num_applications_addition: i32,
}
// --snip--
```
Also, until now the ``Commands`` object has held types ``<Expr, i32>``; in general, a commands object holds types ``<T, M>``, where `T` is the tree type, and `M` is the metadata type. To include our new struct ``Meta``, we need to adjust the types accordingly.
```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
struct Meta {
num_applications_addition: i32,
}
fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    // --snip--
None
}

fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
   // --snip--
None
}

fn rule_expand_sqr(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
   // --snip--
None
}
```
Now the function types make sense, to add one to ``num_applications_addition`` each time an addition rule is applied, we just need to add a metadata command to the ``Commands`` queue each time that a successful rule application is undertaken.

```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
# struct Meta {
# num_applications_addition: i32,
# }
fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    if let Expr::Add(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1)); //new
            return Some(Expr::Val(a_v + b_v));
        }
    }
    None
}
```
Now, each time that the addition rule is successfully applied, the value of num_applications_addition will increase by 1! We may also
choose to place the commands before the ``if`` block, as side-effects are only evaluated upon a successful rule update. This can make the code in the block a little easier to read. The following is
completely equivalent to the above code.

```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
# struct Meta {
# num_applications_addition: i32,
# }
fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
    cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1)); //new location
    if let Expr::Add(a, b) = subtree {
        if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
            return Some(Expr::Val(a_v + b_v));
        }
    }
    None
}
```
The following test block verifies that two addition operations take place.
```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
# struct Meta {
# num_applications_addition: i32,
# }
# fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Add(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1)); //new
#             return Some(Expr::Val(a_v + b_v));
#         }
#     }
#     None
# }
# fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Mul(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             return Some(Expr::Val(a_v * b_v));
#         }
#     }
#     None
# }
# fn rule_expand_sqr(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Sqr(expr) = subtree {
#         return Some(Expr::Mul(
#             Box::new(*expr.clone()),
#             Box::new(*expr.clone())
#         ));
#     }
#     None
# }
#[test]
fn number_of_operations() {
    let my_expression = Expr::Sqr(Box::new(Expr::Add(
        Box::new(Expr::Val(1)),
        Box::new(Expr::Val(2)),
    )));

    let metadata = Meta {
        num_applications_addition: 0,
    };
    let (result, metaresult) = morph(
        vec![rule_fns![rule_eval_add, rule_eval_mul, rule_expand_sqr]],
        tree_morph::helpers::select_panic,
        my_expression.clone(),
        metadata,
    );
    assert_eq!(metaresult.num_applications_addition, 2);
}
```
Running ``cargo test --test file_name`` in a directory containing the file with this code will verify that indeed two addition operations are successfully undertaken.

But wait! The original problem was solving ``(1+2)^2``, why have two addition operations been recorded? The answer is in how we grouped the rules together in the ``morph`` function.

## Rule Groups

In the ``morph`` function, in the first input, rust expects a **rule group** object of type ``Vec<Vec<R>>``, where `R` is the `Rule` trait. We can use the macro ``rule_fns!`` to simultaneously handle giving functions the ``Rule`` trait and putting them inside vectors. Rule groups are a powerful feature of tree-morph, and allow for a priority system whereby some rules are attempted before others. For example, if we have the rule grouping ``vec![vec![rule1,rule2],vec![rule3]]``, ``morph`` will start by visiting the first node and trying to apply rule1 and rule2. If unsuccessful, ``morph`` will then move onto the second node, trying rule1 and rule2 again. Note that since rule3 is in a lower priority grouping, tree-morph will not try rule3 on the first node until rule1 and rule2 have been attempted on the **entire** tree.

It is now clear why ``assert_eq!(metaresult.num_applications_addition, 2);`` holds above. Because rule ``rule_expand_sqr`` was in the same grouping as all the other rules, tree-morph applied the rule before the addition node was ever reached. To increase the efficiency the solving algorithm, we can assign the ``rule_expand_sqr`` with a lower priority.
```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
# struct Meta {
# num_applications_addition: i32,
# }
# fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Add(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1)); //new
#             return Some(Expr::Val(a_v + b_v));
#         }
#     }
#     None
# }
# fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Mul(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             return Some(Expr::Val(a_v * b_v));
#         }
#     }
#     None
# }
# fn rule_expand_sqr(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Sqr(expr) = subtree {
#         return Some(Expr::Mul(
#             Box::new(*expr.clone()),
#             Box::new(*expr.clone())
#         ));
#     }
#     None
# }
#[test]
fn number_of_operations() {
#      let my_expression = Expr::Sqr(Box::new(Expr::Add(
#          Box::new(Expr::Val(1)),
#          Box::new(Expr::Val(2)),
#      )));
#
#      let metadata = Meta {
#          num_applications_addition: 0,
#      };
    // --snip--
    let (result, metaresult) = morph(
        vec![
            rule_fns![rule_eval_add, rule_eval_mul],
            rule_fns![rule_expand_sqr],
        ], //new
        tree_morph::helpers::select_panic,
        my_expression.clone(),
        metadata,
    );
    // --snip--
}
```
Now that ``rule_expand_sqr`` has a lower priority, the addition operation will be applied first, and hence ``metaresult.num_applications_addition`` should equal 1. If we make the following change to the test, we can verify this directly.
```rust
# use tree_morph::prelude::*;
# use uniplate::Uniplate;
#
# #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
# #[uniplate()]
# enum Expr {
#     Add(Box<Expr>, Box<Expr>),
#     Mul(Box<Expr>, Box<Expr>),
#     Sqr(Box<Expr>),
#     Val(i32),
# }
# struct Meta {
# num_applications_addition: i32,
# }
# fn rule_eval_add(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Add(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             cmds.mut_meta(Box::new(|m: &mut Meta| m.num_applications_addition += 1)); //new
#             return Some(Expr::Val(a_v + b_v));
#         }
#     }
#     None
# }
# fn rule_eval_mul(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Mul(a, b) = subtree {
#         if let (Expr::Val(a_v), Expr::Val(b_v)) = (a.as_ref(), b.as_ref()) {
#             return Some(Expr::Val(a_v * b_v));
#         }
#     }
#     None
# }
# fn rule_expand_sqr(cmds: &mut Commands<Expr, Meta>, subtree: &Expr, meta: &Meta) -> Option<Expr> {
#     if let Expr::Sqr(expr) = subtree {
#         return Some(Expr::Mul(
#             Box::new(*expr.clone()),
#             Box::new(*expr.clone())
#         ));
#     }
#     None
# }
#[test]
fn number_of_operations() {
#      let my_expression = Expr::Sqr(Box::new(Expr::Add(
#          Box::new(Expr::Val(1)),
#          Box::new(Expr::Val(2)),
#      )));
#
#      let metadata = Meta {
#          num_applications_addition: 0,
#      };
    // --snip--
    let (result, metaresult) = morph(
        vec![
            rule_fns![rule_eval_add, rule_eval_mul],
            rule_fns![rule_expand_sqr],
        ], //new
        tree_morph::helpers::select_panic,
        my_expression.clone(),
        metadata,
    );
       assert_eq!(metaresult.num_applications_addition, 1); //new, only one addition performed
}
```
## Selector Functions
The second input in the ``morph`` function is a **selector function** from the ``tree_morph::helpers`` crate. Selector functions are what tree-morph uses if there is ever ambiguity in what rule to apply. Ambiguity can arise when two rules are both applicable at the same time, and have the same priority as assigned via rule groupings. We have various ways to deal with this in tree-morph, and in our example we have used ``select_panic``, which causes rust to `panic!` if there is ever ambiguity.

## Commands
We have previously shown how a ``Commands`` struct can be used to store metadata-updating rules. It is also possible to store entire **tree transformations** too. This might be useful for scenarios in which you want a tree transformation to occur immediately after some successful rule change.

## Acknowledgements
Finish....
