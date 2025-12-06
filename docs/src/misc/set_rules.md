
<!-- maturity: draft
authors: 
created: 
---- -->

# Set Rules

As sets are not a type that exists in Essence Prime, they require representation in another form, such as with a matrix. Due to the variety of different set operators that exist, we first attempt to apply horizontal set rules to rewrite operations involving sets into comprehensions. This makes it easier to later apply vertical rules, as any set decision variable will only exist inside of a comprehension generator.

Broadly speaking, there are two categories that we can divide set operations into: those that return booleans and those that return sets. 

<!-- maybe some diagram showing the "flow" of set expression rewriting by type could be useful-->

All of the Boolean set operations involving equality, subsets or supersets are rewritten in terms of combinations of subseteq. This leaves the model with only two boolean set operations, the `in` operator and the `subsetEq` operator, which are easy to turn into comprehensions. 

This process also guarantees that the other class of set operations, those returning sets, will only exist within the generator of a comprehension. This means that only one rule each is required for rewriting the `union` and `intersect` operators.

<!-- describe how each individual rule works-->

## eq_to_subset_eq

This rule matches against `a = b`, where `a` and `b` are expressions with the return type `Set`. The expression is rewritten into `and([(a subsetEq b),(b subsetEq a);int(1..2)])`

## neq_not_eq_sets

This rule matches against `a != b` where `a` and `b` are expression with the return type `Set`. The expression is rewritten into `!((a = b))`

## subset_to_subset_eq_neq

## supset_to_subset

## supset_eq_to_subset_eq

## union_set

## rule_in

[ return_expr | qualifiers..., i <- A union B, qualifiers...] -> flatten([[ return_expr | qualifiers..., i <- A, qualifiers...], [ return_expr | qualifiers.., i <- B, !(i in A), qualifiers...]])
