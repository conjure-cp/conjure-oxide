[//]: # (Author: lilian-contius)
[//]: # (Last Updated: 22/04/2025)

Introductory notes on the use of "<-" in generators, and the logic behind and() and or() comprehensions. Followed by horizontal set rules. These are representation-independent rules in conjure-oxide that are used to rewrite models. 

# Notes:

* "<-" is part of comprehension notation that defines an expression generator
  * the left hand side has the type of a member of the right hand side
  * It is used to loop over elements of a set, primarily within and() and or() comprehensions

* and() - for-all quantifier
  * essentially a series of conjunctions (a ∧ b ∧ .. ∧ z)
  * states that the body of the contained comprehension must hold **for all** elements specified by the generators and conditions. 

* or() - existential quantifier
  * essentially a series of disjunctions (a ∨ b ∨ .. ∨ z)
  * states that the body of the contained comprehension must hold **for at least one** element specified by the generators and conditions. 


# Horizontal Rules

## eq_to_subset_eq (boolean)

```
A = B ~~> A subsetEq B /\ B subsetEq A
```

rule for set equality, checks if two sets are equal, i.e. they contain the same elements

1. identifies pattern: "x = y"
2. checks that x and y have set return type
3. translates equality into conjunction of two subset-equalities
* i.e. "x = y" becomes " "x subsetEq y" AND "y subsetEq x"

### Code:
```rust
fn eq_to_subset_eq(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Eq(_, a, b)
            if matches!(a.as_ref().return_type(), Set(_))
                && matches!(b.as_ref().return_type(), Set(_)) =>
        {
            let expr1 = SubsetEq(Metadata::new(), a.clone(), b.clone());
            let expr2 = SubsetEq(Metadata::new(), b.clone(), a.clone());
            Ok(Reduction::pure(And(
                Metadata::new(),
                Moo::new(matrix_expr![expr1, expr2]),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}

```

## neq_not_eq_sets (boolean)
```
A != B ~~> !(A = B)
```

rule for set inequality

1. identifies pattern: "x != y"
2. checks that x and y have set return type
3. translates inequality to "Not" expression wrapping an equality
* i.e. "x != y" becomes "!(x = y)", which can then be rewritten using eq_to_subset_eq rule above.

### Code:
```rust
fn neq_not_eq_sets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Neq(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            Ok(Reduction::pure(Expr::Not(
                Metadata::new(),
                Moo::new(Expr::Eq(Metadata::new(), b.clone(), a.clone())),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}
```

## subseteq_set (boolean)
```
A subsetEq B ~~> and([i in B | i <- A])
```

rule for subsetEq, checks if one set is contained in another, **they may be equal** 

1. identifies pattern: "x subsetEq y"
2. translates x is subsetEq of y to all elements in x are in y 
* i.e. "x subsetEq y" becomes "for all i in x, i in y"

> NOTE: Although this rule is in the main branch, it is not implemented correctly and it will be fixed pending major changes to the rule engine.

## subset_to_subset_eq_neq (boolean)
```
A subset B ~~> A subsetEq B /\ A != B
```

rule for subset, checks if one set is **strictly** contained in another, they cannot be equal

1. identifies pattern: "a subset b"
2. checks that a and b are sets
3. translates a is subset of b to a is subsetEq of b, and a is not equal to b
* i.e. "a subset b" becomes " "a subsetEq b" AND "a neq b" "

### Code:
```rust 
fn subset_to_subset_eq_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Subset(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            let expr1 = Expr::SubsetEq(Metadata::new(), a.clone(), b.clone());
            let expr2 = Expr::Neq(Metadata::new(), a.clone(), b.clone());
            Ok(Reduction::pure(Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![expr1, expr2]),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}
```

## supset_to_subset (boolean)
```
A supset B ~~> B subset A
```

rule for superset, checks if one set **strictly** contains another, they cannot be equal

1. identifies pattern: "a supset b"
2. checks that a and b are sets
3. translates a is superset of b to b is subset of a

### Code:
```rust
fn supset_to_subset(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Supset(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            Ok(Reduction::pure(Expr::Subset(
                Metadata::new(),
                b.clone(),
                a.clone(),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}

```

## supset_eq_to_subset_eq (boolean)
```
A supsetEq B ~~> B subsetEq A
```

rule for supsetEq, checks if one set contains another, **they may be equal** 

1. identifies pattern: "x supsetEq y"
2. checks that x and y are sets
3. translates x is supsetEq of y to y is subsetEq of x

### Code:
```rust
fn supset_eq_to_subset_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SupsetEq(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            Ok(Reduction::pure(Expr::SubsetEq(
                Metadata::new(),
                b.clone(),
                a.clone(),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}
```


## Set-intersect (describes a new set)

rule for set intersection. defines that an element is in the intersection of two sets when it is in both sets. similar structure as comprehension literal rule, only used within generators or conditions

1. attempts to match generator to pattern and expression: pattern "_quantified variable_ <-" and expression with a modifier operator (if present) applied to a set/multiset/relation "s"
2. attempts to match s to "x intersect y"
3. checks x is a set, multiset, function, or relation
4. replaces generator with same pattern and modifier (if present) applied to x, and adds the condition that the relevant quantified variable must be in y.
* i.e. "i <- x intersect y" becomes "i <- x, i in y"

### Code:
```haskell
theRule (Comprehension body gensOrConds) = do
         (gocBefore, (pat, iPat, expr), gocAfter) <- matchFirst gensOrConds $ \ goc -> case goc of
             Generator (GenInExpr pat@(Single iPat) expr) ->
                 return (pat, iPat, matchDefs [opToSet,opToMSet,opToRelation] expr)
             _ -> na "rule_Intersect"
         (mkModifier, s)    <- match opModifier expr
         (x, y)             <- match opIntersect s
         tx                 <- typeOf x
         case tx of
             TypeSet{}      -> return ()
             TypeMSet{}     -> return ()
             TypeFunction{} -> return ()
             TypeRelation{} -> return ()
             _              -> failDoc "type incompatibility in intersect operator"
         let i = Reference iPat Nothing
         return
             ( "Horizontal rule for set intersection"
             , return $
                 Comprehension body
                     $  gocBefore
                     ++ [ Generator (GenInExpr pat (mkModifier x))
                        , Condition [essence| &i in &y |]
                        ]
                     ++ gocAfter
             )
```

### Example: 
* set-intersect rule appears within model C = A intersect B, after applying set-equals, set-subsetEq, set-in
* q4 is quantified in a former step, it is an element in C
* see Context #1, q5 is quantified by or() - existence quantifier, translates to "there exists a q5 in A intersect B such that q5 equals q4"
* translates "q5 <- A intersect B" to "q5 <- A, q5 in B", i.e. body of or() comprehension must apply to at least one member (q5) of A, with the additional condition that q5 must be in B.
```
Picking the first option: Question 1: [q5 = q4 | q5 <- A intersect B]
                               Context #1: or([q5 = q4 | q5 <- A intersect B])
                               ...
     Answer 1: set-intersect: Horizontal rule for set intersection
               [q5 = q4 | q5 <- A intersect B] ~~> [q5 = q4 | q5 <- A, q5 in B]
```

## union_set (describes a new set)
```
[ return_expr | i <- A union B, qualifiers...] -> flatten([[ return_expr | i <- A, qualifiers...], [ return_expr | i <- B, !(i in A), qualifiers...]; int(1..2)])
```

rule for set union. defines that an element is in the union of two sets when it is in at least one of the sets.

1. attempts to match expression generator that generates from a union expression
2. creates two comprehensions, one that generates elements from the first set and one that generates elements from the second set, with an additional condition to prevent double counting
3. both comprehensions are combined in a matrix and put into a flatten expression
* both comprehensions have same return expression and other qualifiers from original comprehension

### Code:
```rust
fn union_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for qualifier in &comp.qualifiers {
                if let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier {
                    let gen_decl = ptr.clone();

                    // match on expression being of form A union B
                    let Some((a, b)) = (match ptr.as_quantified_expr() {
                        Some(expr_guard) => match &*expr_guard {
                            Expr::Union(_, a, b) => Some((a.clone(), b.clone())),
                            _ => None,
                        },
                        None => None,
                    }) else {
                        continue;
                    };

                    // [ return_expr | i <- A, guards...] part
                    let (comprehension1, _) = replace_expression_generator_source(
                        comp.as_ref(),
                        &gen_decl,
                        a.clone().into(),
                    );

                    // [ return_expr | i <- B, !(i in A), guards...] part
                    let (mut comprehension2, b_ptr) =
                        replace_expression_generator_source(comp.as_ref(), &gen_decl, b.into());

                    // add the condition !(i in A)
                    comprehension2
                        .qualifiers
                        .push(ComprehensionQualifier::Condition(Expr::Not(
                            Metadata::new(),
                            Moo::new(Expr::In(
                                Metadata::new(),
                                Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(b_ptr))),
                                a,
                            )),
                        )));

                    return Ok(Reduction::pure(Expr::Flatten(
                        Metadata::new(),
                        None,
                        Moo::new(into_matrix_expr!(vec![
                            Expr::Comprehension(Metadata::new(), comprehension1.into()),
                            Expr::Comprehension(Metadata::new(), comprehension2.into())
                        ])),
                    )));
                }
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
```


## difference_set (describes a new set)
```
[ return_expr | i <- A - B, qualifiers...] -> [ return_expr | i <- A, !(i in B), qualifiers...]
```

rule for set difference. defines that an element is in the difference of two sets when it is in the former but not in the latter.

1. attempts to match expression generator that generates from a difference expression "x - y"
2. replace the generator's expression with the first set "x"
3. adds the condition that the relevant quantified variable must not be in the second set "y

### Code:
```rust
fn difference_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for qualifier in &comp.qualifiers {
                if let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier {
                    let gen_decl = ptr.clone();

                    // match on expression being of form A - B
                    let Some((a, b)) = (match ptr.as_quantified_expr() {
                        Some(expr_guard) => match &*expr_guard {
                            Expr::Minus(_, a, b) => Some((a.clone(), b.clone())),
                            _ => None,
                        },
                        None => None,
                    }) else {
                        continue;
                    };

                    // [ return_expr | i <- A, !(i in B), guards...]
                    let (mut comprehension, a_ptr) =
                        replace_expression_generator_source(comp.as_ref(), &gen_decl, a.into());

                    // add the condition !(i in B)
                    comprehension
                        .qualifiers
                        .push(ComprehensionQualifier::Condition(Expr::Not(
                            Metadata::new(),
                            Moo::new(Expr::In(
                                Metadata::new(),
                                Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(a_ptr))),
                                b,
                            )),
                        )));

                    return Ok(Reduction::pure(Expr::Comprehension(
                        Metadata::new(),
                        comprehension.into(),
                    )));
                }
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
```


## Set-in (boolean)

rule for set membership, checks whether something is an element of a set

1. identifies pattern: "x in s"
2. checks s is a set
3. introduces quantified variable to go through set s and check whether any of its elements equal x
* i.e. "x in s" becomes "there exists i in s such that i = x"

### Code:
```haskell
     theRule p = do
         (x,s)     <- match opIn p
         TypeSet{} <- typeOf s
         -- do not apply this rule to quantified variables
         -- or else we might miss the opportunity to apply a more specific vertical rule
         if referenceToComprehensionVar s
             then na "rule_In"
             else return ()
         return
             ( "Horizontal rule for set-in."
             , do
                  (iPat, i) <- quantifiedVar
                  return [essence| exists &iPat in &s . &i = &x |]
             )
```

### Example: 
* here we are checking for membership of q4 in B union C
* q4 is a quantified variable in larger context
* q5 is the quantified variable introduced by the set-in rule.
```
Picking the first option: Question 1: q4 in B union C
     Answer 1: set-in: Horizontal rule for set-in.
               q4 in B union C ~~> or([q5 = q4 | q5 <- B union C])
```

## Set-card (int)

rule for set cardinality, counts number of elements in a set

1. identifies pattern: |s|
2. checks s is a set
3. if it exists, returns the set's domain's size attribute
4. otherwise introduces quantified variable "iPat" to iterate through set, incrementing the sum for each new element, returns sum.

### Code:
```haskell
rule_Card = "set-card" `namedRule` theRule where
     theRule p = do
         s         <- match opTwoBars p
         case s of
             Domain{} -> na "rule_Card"
             _        -> return ()
         TypeSet{} <- typeOf s
         return
             ( "Horizontal rule for set cardinality."
             , do
                 mdom <- runMaybeT $ domainOf s
                 case mdom of
                     Just (DomainSet _ (SetAttr (SizeAttr_Size n)) _) -> return n
                     _ -> do
                         (iPat, _) <- quantifiedVar
                         return [essence| sum &iPat in &s . 1 |]
             )
```

### Example: 
* set-card rule appears within model |A| = |B|
* here q3 is the quantified variable called "iPat" above
```
Picking the first option: Question 1: |A|
                               Context #1: |A| = |B|
     Answer 1: set-card: Horizontal rule for set cardinality.
               |A| ~~> sum([1 | q3 <- A])
```