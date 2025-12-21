[//]: # (Author: lilian-contius)
[//]: # (Last Updated: 22/04/2025)

Introductory notes on the use of "<-" in generators, and the logic behind and() and or() comprehensions. Followed by horizontal set rules. These are representation-independent rules in conjure that are used to rewrite models. 

# Notes:

* "<-" is called an expression projection
  * it creates a generator called GenInExpr
  * the left hand side has the type of a member of the right hand side
  * It is used to loop over elements of a set, primarily within and() and or() comprehensions
  * see [Expression Projection](https://conjure.readthedocs.io/en/latest/bits/keyword/expr_projection.html) for more information

* and() - for-all quantifier
  * essentially a series of conjunctions (a ∧ b ∧ .. ∧ z)
  * states that the body of the contained comprehension must hold **for all** elements specified by the generators and conditions. 

* or() - existence quantifier
  * essentially a series of disjunctions (a ∨ b ∨ .. ∨ z)
  * states that the body of the contained comprehension must hold **for at least one** element specified by the generators and conditions. 

* example combining these three concepts:
  * taken from the last section of this page (shown both before and after vertical rules are applied):
```
and([ or([ q5 = q4 | q5 <- A, or([ q6 = q5 | q6 <- B ]) ]) | q4 <- C ])
and([C_Occurrence[q4] -> or([A_Occurrence[q5] ∧ B_Occurrence[q5] ∧ q5 = q4 | q5 : int(0..6)]) | q4 : int(0..6)])
```
this translates to: 
```
∀q4 ϵ C: ∃ q5 ϵ A∶(q5=q4) ∧ (∃ q6 ϵ B∶q6=q5 )
```

# Horizontal Rules

## Set-Comprehension-Literal

identifies set literal in model and converts to matrix literal containing the same elements of the same type as the set.

1. takes in Comprehension containing a "body" and generators or conditions "gensOrConds"
2. matches "gensOrConds" to a tuple containing the Generator "(pat,expr)", between two generators or conditions, "gocBefore" and "gocAfter"
3. identifies generator containing pattern and expression, attempts to match expression to a set or multiset
4. stores elements of set literal in a list of relevant type "tau"
5. creates matrix literal of type "tau" containing same elements as set literal
6. returns original comprehension with same body, gocBefore, gocAfter, middle Generator with same pattern but matrix literal replaces set literal 

### Code:
```haskell
     theRule (Comprehension body gensOrConds) = do
         (gocBefore, (pat, expr), gocAfter) <- matchFirst gensOrConds $ \ goc -> case goc of
             Generator (GenInExpr pat@Single{} expr) -> return (pat, matchDefs [opToSet, opToMSet] expr)
             Generator (GenInExpr pat@AbsPatSet{} expr) -> return (pat, matchDefs [opToSet, opToMSet] expr)
             _ -> na "rule_Comprehension_Literal"
         (TypeSet tau, elems) <- match setLiteral expr
         let outLiteral = make matrixLiteral
                             (TypeMatrix (TypeInt TagInt) tau)
                             (DomainInt TagInt [RangeBounded 1 (fromInt (genericLength elems))])
                             elems
         return
             ( "Comprehension on set literals"
             , return $ Comprehension body
                      $  gocBefore
                      ++ [Generator (GenInExpr pat outLiteral)]
                      ++ gocAfter
             )
```

### Example:
* set-comprehension-literal rule appears within model: "letting A be {1,5,3}, find B : set of int(0..6), such that B subsetEq A"
* here the body of the comprehension is "q3 = q2", the gensOrConds is the single Generator "q3 <- {1,3,5}"
* the set literal expression {1,3,5} is replaced with the matrix literal [1,3,5; int(1..3)], the pattern "q3 <-" and the body are left unaffected.
* context:
  * q2 is a quantified variable in larger scope (Context #3) -- using Occurrence representation here, used to iterate over elements in B
  * q3 is the quantified variable used to check that each q2 is an element from A = {1,3,5}
```
Picking the first option: Question 1: [q3 = q2 | q3 <- {1, 3, 5}]
                               Context #1: or([q3 = q2 | q3 <- {1, 3, 5}])
                               Context #3: and([or([q3 = q2 | q3 <- {1, 3, 5}]) | q2 : int(0..6), B_Occurrence[q2]])
     Answer 1: set-comprehension-literal: Comprehension on set literals
               [q3 = q2 | q3 <- {1, 3, 5}]
               ~~>
               [q3 = q2 | q3 <- [1, 3, 5; int(1..3)]]
```

## Set-eq (boolean)

rule for set equality, checks if two sets are equal, i.e. they contain the same elements

1. identifies pattern: "x eq y"
2. checks that x and y are sets
3. translates equality into conjunction of two subset-equalities
* i.e. "x eq y" becomes " "x subsetEq of y" AND "y subsetEq of x" ", using Set-subsetEq rule

### Code:
```Haskell
     theRule p = do
         (x,y)     <- match opEq p
         TypeSet{} <- typeOf x
         TypeSet{} <- typeOf y
         return
             ( "Horizontal rule for set equality"
             , return $ make opAnd $ fromList
                 [ make opSubsetEq x y
                 , make opSubsetEq y x
                 ]
             )
```

### Example:
```
Picking the first option: Question 1: A = B union C
     Answer 1: set-eq: Horizontal rule for set equality
               A = B union C ~~> A subsetEq B union C /\ B union C subsetEq A
```

## Set-neq (boolean)

rule for set inequality

1. identifies pattern: "x != y"
2. checks that x and y are sets
3. translates inequality to existence of an element that is either in x but not in y, or that is in y but not in x
* i.e. "x != y" becomes "there exists i such that i in x and i not in y, or i in y and i not in x"

### Code:
```haskell
     theRule [essence| &x != &y |] = do
         TypeSet{} <- typeOf x
         TypeSet{} <- typeOf y
         return
             ( "Horizontal rule for set dis-equality"
             , do
                  (iPat, i) <- quantifiedVar
                  return [essence|
                          (exists &iPat in &x . !(&i in &y))
                          \/
                          (exists &iPat in &y . !(&i in &x))
                      |]
             )
```

### Example: 
* checking that the set A is not equal to set B
* here q4 is the quantified variable, referred to as "i" above.
* "exists" statement is translated using the existence quantifier or(), quantifying over q4 members of A.
* The expression projection "<-" creates a generator, in which the lhs has the type of a member of the rhs. 
This means the body of the or() comprehension must apply to at least one member (q4) of A (see [Notes](https://github.com/conjure-cp/conjure-oxide/wiki/Conjure-Horizontal-Set-Rules#notes)).
```
Picking the first option: Question 1: A != B union C
     Answer 1: set-neq: Horizontal rule for set dis-equality
               A != B union C
               ~~>
               or([!(q4 in B union C) | q4 <- A]) \/
               or([!(q4 in A) | q4 <- B union C])
```

## Set-subsetEq (boolean)

rule for subsetEq, checks if one set is contained in another, **they may be equal** 

1. identifies pattern: "x subsetEq y"
2. checks that x and y are sets
3. translates x is subsetEq of y to all elements in x are in y 
* i.e. "x subsetEq y" becomes "for all i in x, i in y"

### Code:
```haskell
     theRule p = do
         (x,y)     <- match opSubsetEq p
         TypeSet{} <- typeOf x
         TypeSet{} <- typeOf y
         return
             ( "Horizontal rule for set subsetEq"
             , do
                  (iPat, i) <- quantifiedVar
                  return [essence| forAll &iPat in &x . &i in &y |]
             )
```

### Example: 
* here q3 is the quantified variable, referred to as "i" above.
* and() is the universal quantifier, quantifying over q3. and([q3 in B | q3 <- A]) translates to "for all q3 in A, q3 is in B"
* The expression projection "<-" creates a generator, in which the lhs has the type of a member of the rhs. 
This means the body of the and() comprehension must apply to all members (q3) of A. (see [Notes](https://github.com/conjure-cp/conjure-oxide/wiki/Conjure-Horizontal-Set-Rules#notes)).
```
Picking the first option: Question 1: A subsetEq B
     Answer 1: set-subsetEq: Horizontal rule for set subsetEq
               A subsetEq B ~~> and([q3 in B | q3 <- A])
```

## Set-subset (boolean)

rule for subset, checks if one set is **strictly** contained in another, they cannot be equal

1. identifies pattern: "a subset b"
2. checks that a and b are sets
3. translates a is subset of b to a is subsetEq of b, and a is not equal to b
* i.e. "a subset b" becomes " "a subsetEq b" AND "a neq b" ", using rules Set-subsetEq and Set-neq

### Code:
```haskell 
     theRule [essence| &a subset &b |] = do
         TypeSet{} <- typeOf a
         TypeSet{} <- typeOf b
         return
             ( "Horizontal rule for set subset"
             , return [essence| &a subsetEq &b /\ &a != &b |]
             )
     theRule _ = na "rule_Subset"
```

### Example:
```
Picking the first option: Question 1: A subset B
     Answer 1: set-subset: Horizontal rule for set subset
               A subset B ~~> A subsetEq B /\ A != B
```

## Set-supset (boolean)
rule for superset, checks if one set **strictly** contains another, they cannot be equal

1. identifies pattern: "a supset b"
2. checks that a and b are sets
3. translates a is superset of b to b is subset of a, and applies subset rule.

### Code:
```haskell
     theRule [essence| &a supset &b |] = do
         TypeSet{} <- typeOf a
         TypeSet{} <- typeOf b
         return
             ( "Horizontal rule for set supset"
             , return [essence| &b subset &a |]
             )
     theRule _ = na "rule_Supset"
```

### Example:
```
Picking the first option: Question 1: A supset B
     Answer 1: set-supset: Horizontal rule for set supset
               A supset B ~~> B subset A
```

## Set-supsetEq (boolean)

rule for supsetEq, checks if one set contains another, **they may be equal** 

1. identifies pattern: "x supsetEq y"
2. checks that x and y are sets
3. translates x is supsetEq of y to y is subsetEq of x, and applies subsetEq rule.

### Code:
```haskell
     theRule [essence| &a supsetEq &b |] = do
         TypeSet{} <- typeOf a
         TypeSet{} <- typeOf b
         return
             ( "Horizontal rule for set supsetEq"
             , return [essence| &b subsetEq &a |]
             )
     theRule _ = na "rule_SupsetEq"
```

### Example: 
```
Picking the first option: Question 1: A supsetEq B
     Answer 1: set-subsetEq: Horizontal rule for set supsetEq
               A supsetEq B ~~> B subsetEq A
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

## Set-max-min (int)
rule for finding maximum and minimum of a set of integers
* contains two sub-rules, one for max, one for min.
* if set is literal, converts to list and finds max. 
* otherwise creates quantified variable and uses max() operation in Essence.
* minimum rule works analogously

### Code:
```haskell
     theRule [essence| max(&s) |] = do
         TypeSet (TypeInt _) <- typeOf s
         return             
             ( "Horizontal rule for set max"
             , case () of
                 _ | Just (_, xs) <- match setLiteral s, length xs > 0 -> return $ make opMax $ fromList xs
                 _ -> do
                     (iPat, i) <- quantifiedVar
                     return [essence| max([&i | &iPat <- &s]) |]
             )
     theRule [essence| min(&s) |] = do
         TypeSet (TypeInt _) <- typeOf s
         return
             ( "Horizontal rule for set min"
             , case () of
                 _ | Just (_, xs) <- match setLiteral s, length xs > 0 -> return $ make opMin $ fromList xs
                 _ -> do
                     (iPat, i) <- quantifiedVar
                     return [essence| min([&i | &iPat <- &s]) |]
             )
     theRule _ = na "rule_MaxMin"
```

### Example:
```
Picking the first option: Question 1: max(A)
                               Context #1: max(A) = max(B)
     Answer 1: set-max-min: Horizontal rule for set max
               max(A) ~~> max([q3 | q3 <- A])
```
### Set-literal example:
```
        letting A be {5, 6, 3}
        find i: int(0..10)
        such that i = min(A)
        branching on [i]
        such that
        such that true(i)
        
Picking the first option: Question 1: min(A)
                               Context #1: i = min(A)
     Answer 1: full-evaluate: Full evaluator
               min(A) ~~> 3
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