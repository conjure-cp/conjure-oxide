
<!-- maturity: draft
authors: 
created: 
---- -->

# Set Rules

As sets are not a type that exists in Essence Prime, they require representation in another form, such as with a matrix. Due to the variety of different set operators that exist, we first attempt to apply horizontal set rules to rewrite operations involving sets into comprehensions. This makes it easier to later apply vertical rules, as any set decision variable will only exist inside of a comprehension generator.

Horizontal rules apply any set operations, such as intersections or union, that involve comparing or manipulating several sets. Once these operations are complete, sets can be unfolded using vertical rules. 

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

This rule matches against `a subset b` where `a` and `b` are expressions with the return type `Set`. The expression is rewritten into `and([(a subsetEq b),(a != b); int(1..2)])`

## supset_to_subset

This rule matches against `a supset b` where `a` and `b` are expressions with the return type `Set`. The expression is rewritten into `(b subset a)`

## supset_eq_to_subset_eq

This rule matches against `a supsetEq b` where `a` and `b` are expressions with the return type `Set`. The expression is rewritten into `(b subsetEq a)`

## union_set

This rule matches against any comprehension that contains a generator of the form `i <- A union B`, where `A` and `B` are expressions with the return type `Set`. The expression is rewritten into two comprehensions combined using `flatten`, one iterating over `A` and the other iterating over `B` but excluding any elements already in `A`.

The rewritten expression for some comprehension `[ return_expr | i <- A union B, qualifiers...]` looks like `flatten([[ return_expr | i <- A, qualifiers...], [ return_expr | i <- B, !(i in A), qualifiers...]; int(1..2)])`

The generator can be in any position within the comprehension's list of qualifiers. Only one such generator is rewritten at a time.

## intersect
This rule matches against `a intersect b` where `a` and `b` are expressions with the return type `Set`. It returns an abstract comprehension that uses one of the sets as a generator and checks for equality with members of the other set. 

## rule_in

[ return_expr | qualifiers..., i <- A union B, qualifiers...] -> flatten([[ return_expr | qualifiers..., i <- A, qualifiers...], [ return_expr | qualifiers.., i <- B, !(i in A), qualifiers...]])
