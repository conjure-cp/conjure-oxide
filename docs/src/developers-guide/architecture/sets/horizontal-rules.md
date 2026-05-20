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

## Set-union (describes a new set)

rule for set union. defines that an element is in the union of two sets when it is in at least one of the sets. similar structure as comprehension literal rule, only used within generators or conditions 

1. attempts to match generator to pattern and expression: pattern "_quantified variable_ <-" and expression with a modifier operator (if present) applied to a set/multiset/relation "s"
2. attempts to match s to "x union y"
3. checks x is a set, multiset, function, or relation
4. creates abstract matrix containing two comprehensions: 
* both comprehensions have body from original comprehension
* both comprehensions have a generator with same pattern on same quantified variable. 
* expressions in generators consist of same modifier operator (if present) applied to only set x and only set y respectively
* for set y, the additional condition "i not in x" is added to prevent double counting (relevant for sums)

### Code:
```haskell
     theRule (Comprehension body gensOrConds) = do
         (gocBefore, (pat, iPat, expr), gocAfter) <- matchFirst gensOrConds $ \ goc -> case goc of
             Generator (GenInExpr pat@(Single iPat) expr) -> return (pat, iPat, matchDef opToSet expr)
             _ -> na "rule_Union"
         (mkModifier, s)    <- match opModifier expr
         (x, y)             <- match opUnion s
         tx                 <- typeOf x
         case tx of
             TypeSet{}      -> return ()
             TypeMSet{}     -> return ()
             TypeFunction{} -> return ()
             TypeRelation{} -> return ()
             _              -> failDoc "type incompatibility in union operator"
         let i = Reference iPat Nothing
         return
             ( "Horizontal rule for set union"
             , return $ make opFlatten $ AbstractLiteral $ AbsLitMatrix
                 (DomainInt TagInt [RangeBounded 1 2])
                 [ Comprehension body
                     $  gocBefore
                     ++ [ Generator (GenInExpr pat (mkModifier x)) ]
                     ++ gocAfter
                 , Comprehension body
                     $  gocBefore
                     ++ [ Generator (GenInExpr pat (mkModifier y))
                        , Condition [essence| !(&i in &x) |]
                        ]
                     ++ gocAfter
                 ]
             )
```

### Example:
* set-union rule appears within model C = A union B, after applying set-equals, set-subsetEq, set-in
* q4 is quantified in a former step, it is an element in C (Context #3)
* see Context #1, q5 is quantified by or() - existence quantifier, translates to "there exists a q5 in A union B such that q5 equals q4"
* translates "[q5 = q4 | q5 <- A union B]" to return the matrix "flatten([[q5 = q4 | q5 <- A], [q5 = q4 | q5 <- B, !(q5 in A)]; int(1..2)])"
* "q5 = q4" is the body of the comprehension, the pattern "q5 <-" is applied to both A and B in the respective entries of the matrix
* additional condition "!(q5 in A)" for the second comprehension, to prevent double counting
  * i.e. check that the body "q5 = q4", applies to at least one member (q5) in A or in B.
```
Picking the first option: Question 1: [q5 = q4 | q5 <- A union B]
                               Context #1: or([q5 = q4 | q5 <- A union B])
                               Context #3: and([or([q5 = q4 | q5 <- A union B]) | q4 : int(0..6), C_Occurrence[q4]])
     Answer 1: set-union: Horizontal rule for set union
               [q5 = q4 | q5 <- A union B]
               ~~>
               flatten([[q5 = q4 | q5 <- A], [q5 = q4 | q5 <- B, !(q5 in A)];
                            int(1..2)])
```

## Set-difference (describes a new set)

rule for set difference. defines that an element is in the difference of two sets when it is in the former but not in the latter. similar structure as comprehension literal rule, only used within generators or conditions 

1. attempts to match generator to pattern and expression: pattern "_quantified variable_ <-" and expression with a modifier operator (if present) applied to a set/multiset/relation "s"
2. attempts to match s to "x - y"
3. checks x is a set, multiset, function, or relation
4. returns comprehension with same body as original, same gocBefore, gocAfter. Generator is replaced by original generator applied only to x, with additional condition "i not in y"

### Code:
```haskell
     theRule (Comprehension body gensOrConds) = do
         (gocBefore, (pat, iPat, expr), gocAfter) <- matchFirst gensOrConds $ \ goc -> case goc of
             Generator (GenInExpr pat@(Single iPat) expr) -> return (pat, iPat, expr)
             _ -> na "rule_Difference"
         (mkModifier, s)    <- match opModifier expr
         (x, y)             <- match opMinus s
         tx                 <- typeOf x
         case tx of
             TypeSet{}      -> return ()
             TypeMSet{}     -> return ()
             TypeFunction{} -> return ()
             TypeRelation{} -> return ()
             _              -> failDoc "type incompatibility in difference operator"
         let i = Reference iPat Nothing
         return
             ( "Horizontal rule for set difference"
             , return $
                 Comprehension body
                     $  gocBefore
                     ++ [ Generator (GenInExpr pat (mkModifier x))
                        , Condition [essence| !(&i in &y) |]
                        ]
                     ++ gocAfter
             )
```

### Example: 
* set-difference rule appears within model C = A - B, after applying set-equals, set-subsetEq, set-in
* q4 is quantified in a former step, it is an element in C (Context #3)
* see Context #1, q5 is quantified by or() - existence quantifier, translates to "there exists a q5 in A - B such that q5 equals q4"
* translates "q5 <- A - B" to "q5 <- A, !(q5 in B)"
```
Picking the first option: Question 1: [q5 = q4 | q5 <- A - B]
                               Context #1: or([q5 = q4 | q5 <- A - B])
                               Context #3: and([or([q5 = q4 | q5 <- A - B]) | q4 : int(0..6), C_Occurrence[q4]])
     Answer 1: set-difference: Horizontal rule for set difference
               [q5 = q4 | q5 <- A - B] ~~> [q5 = q4 | q5 <- A, !(q5 in B)]
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