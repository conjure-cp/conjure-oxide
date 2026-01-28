[//]: # (Author: lilian-contius)
[//]: # (Last Updated: 22/04/2025)

Vertical rules for Occurrence, which are applied once all possible horizontal rules have been applied at a given stage. Followed by an example of rewriting a model in Occurrence representation, using both horizontal and vertical rules. 

* see [Conjure Horizontal Set Rules](https://github.com/conjure-cp/conjure-oxide/wiki/Conjure-Horizontal-Set-Rules) for representation-independent horizontal set rules.

# Vertical rules for Occurrence-representation of sets

* Occurrence representation creates a matrix that has the length of the maximum integer value contained in the sets. E.g. a set {1,5,11} will be represented by a matrix of length 11. Each entry in the matrix is a boolean, representing whether that integer is contained in the set. in the matrix for the set {1,5,11}, the first, fifth and eleventh entry will all be True, all others will be False. 

## Set-comprehension{Occurrence}

Comprehension rule for Occurrence. Uses the "<-" expression projection (see [Horizontal Rules - Notes](https://github.com/conjure-cp/conjure-oxide/wiki/Conjure-Horizontal-Set-Rules#notes)). 

1. identifies a comprehension consisting of a "body" followed by "generators or conditions"
2. matches the generator to an expression of the form "iPat <- s", iPat, later called "i" is a quantified variable
3. checks that s is a set, and that it has Occurrence representation
4. retrieves matrix m representing this set, and its domain
5. returns Comprehension with same body, original generator is replaced by a generator specifying the domain of the quantified variable, followed by a condition that the ith entry of the Occurrence matrix is true. 
* i.e. it loops over the given domain and checks that element "i" is present in the set. 
### Code:
```Haskell
    theRule (Comprehension body gensOrConds) = do
        (gocBefore, (pat, iPat, s), gocAfter) <- matchFirst gensOrConds $ \ goc -> case goc of
            Generator (GenInExpr pat@(Single iPat) s) -> return (pat, iPat, matchDefs [opToSet,opToMSet] s)
            _ -> na "rule_Comprehension"
        TypeSet{}            <- typeOf s
        Set_Occurrence       <- representationOf s
        [m]                  <- downX1 s
        DomainMatrix index _ <- domainOf m
        let i = Reference iPat Nothing
        return
            ( "Vertical rule for set-comprehension, Occurrence representation"
            , return $
                Comprehension body
                    $  gocBefore
                    ++ [ Generator (GenDomainNoRepr pat index)
                       , Condition [essence| &m[&i] |]
                       ]
                    ++ gocAfter
            )
    theRule _ = na "rule_Comprehension"
```

### Example:
* here the quantified variable is q3, the set is A, the domain of the matrix is int(0..6). the GenInExpr "q3 <- A" is rewritten using the domain and the Occurrence matrix. 
* the larger context is an and() comprehension representing the statement "for all q3 in A: q3 in B", where "for all q3 in A" comes from the GenInExpr (see [Horizontal Rules - Notes](https://github.com/conjure-cp/conjure-oxide/wiki/Conjure-Horizontal-Set-Rules#notes))
* this is rewritten as "for all q3 in the domain int(0..6), for which the q3th index of A_Occurrence is true: q3 in B" 
```
Picking the first option: Question 1: [q3 in B | q3 <- A]
                              Context #1: and([q3 in B | q3 <- A])
    Answer 1: set-comprehension{Occurrence}: Vertical rule for set-comprehension, Occurrence representation
              [q3 in B | q3 <- A]
              ~~>
              [q3 in B | q3 : int(0..6), A_Occurrence[q3]]
```

## Set-in{Occurrence}

Set-in rule for Occurrence representation. In Occurrence representation, an element "i" is in a set if and only if the "ith" entry of the Occurrence matrix is true. 
* This rule is not necessary, "in" can be implemented using the comprehension rule above. 

1. identifies a pattern "x in s"
2. checks s is a set of type Occurrence and retrieves its Occurrence representation matrix, m.
3. returns the value of m at x. 

### Code:
```haskell
    theRule p = do
        (x, s)         <- match opIn p
        TypeSet{}      <- typeOf s
        Set_Occurrence <- representationOf s
        [m]            <- downX1 s
        return
            ( "Vertical rule for set-in, Occurrence representation"
            , return $ make opIndexing m x
            )
```

### Example:
* here we are checking for containment of a quantified variable q3 in a set B
* the statement "q3 in B" is replaced with the value of B_Occurrence at index q3. 
```
Picking the first option: Question 1: q3 in B
                              Context #1: [q3 in B | q3 : int(0..6), A_Occurrence[q3]]
    Answer 1: set-in{Occurrence}: Vertical rule for set-in, Occurrence representation
              q3 in B ~~> B_Occurrence[q3]
```

# General Example: Rewriting the generalised model  "C = A intersect B"

### Original Essence Model:

```
find A,B,C : set of int(0..6)  
such that C = A intersect B
```

### Conjure rewriting the problem, step by step:

1. first set equals is converted to conjunction of subsetEq, applying horizontal Set-eq rule
```
    Answer 1: set-eq: Horizontal rule for set equality
              C = A intersect B
              ~~>
              C subsetEq A intersect B /\ A intersect B subsetEq C
```
2. dealing with the left hand side first, subsetEq is converted into a comprehension, applying horizontal Set-subsetEq rule.
The expression projection "<-" (see [Horizontal Rules - Notes](https://github.com/conjure-cp/conjure-oxide/wiki/Conjure-Horizontal-Set-Rules#notes)) means the body of the and() comprehension must apply to all members (q4) of C
```
Picking the first option: Question 1: C subsetEq A intersect B
                              Context #1: [C subsetEq A intersect B, A intersect B subsetEq C; int(1..2)]
    Answer 1: set-subsetEq: Horizontal rule for set subsetEq
              C subsetEq A intersect B ~~> and([q4 in A intersect B | q4 <- C])
```
3. the inside of the comprehension is simplified, using the definition of the "<-" expression projection in Occurrence representation. (see previous section "Set-comprehension{Occurrence}")
"q4 <- C" is converted to: all q4 in the length of the matrix (0 to 6) for which the q4th entry in the Occurrence matrix is true. 
```
Picking the first option: Question 1: [q4 in A intersect B | q4 <- C]
                              Context #1: and([q4 in A intersect B | q4 <- C])
                              Context #3: and([q4 in A intersect B | q4 <- C]) /\ A intersect B subsetEq C
    Answer 1: set-comprehension{Occurrence}: Vertical rule for set-comprehension, Occurrence representation
              [q4 in A intersect B | q4 <- C]
              ~~>
              [q4 in A intersect B | q4 : int(0..6), C_Occurrence[q4]] 
```
4. turning "q4 in A intersect B" into a comprehension, applying horizontal Set-in rule. again using the expression projection "<-", to signify that the body of the or() comprehension must apply to at least one member (q5) of A intersect B 
```
Picking the first option: Question 1: q4 in A intersect B
                              Context #1: [q4 in A intersect B | q4 : int(0..6), C_Occurrence[q4]]
                              Context #3: [and([q4 in A intersect B | q4 : int(0..6), C_Occurrence[q4]]), A intersect B subsetEq C; int(1..2)]
    Answer 1: set-in: Horizontal rule for set-in.
              q4 in A intersect B ~~> or([q5 = q4 | q5 <- A intersect B])
```
5. replacing "q5 <- A intersect B" generator with "q5 <- A, q5 in B", applying horizontal Set-intersect rule
```
Picking the first option: Question 1: [q5 = q4 | q5 <- A intersect B]
                              Context #1: or([q5 = q4 | q5 <- A intersect B])
                              Context #3: and([or([q5 = q4 | q5 <- A intersect B]) | q4 : int(0..6), C_Occurrence[q4]])
                              Context #5: and([or([q5 = q4 | q5 <- A intersect B]) | q4 : int(0..6), C_Occurrence[q4]]) /\ A intersect B subsetEq C
    Answer 1: set-intersect: Horizontal rule for set intersection
              [q5 = q4 | q5 <- A intersect B] ~~> [q5 = q4 | q5 <- A, q5 in B]
```
6. as in step 3., "<-" is replaced with its definition in Occurrence representation. (see previous section "Set-comprehension{Occurrence}")
```
Picking the first option: Question 1: [q5 = q4 | q5 <- A, q5 in B]
                              Context #1: or([q5 = q4 | q5 <- A, q5 in B])
                              Context #3: and([or([q5 = q4 | q5 <- A, q5 in B]) | q4 : int(0..6), C_Occurrence[q4]])
                              Context #5: and([or([q5 = q4 | q5 <- A, q5 in B]) | q4 : int(0..6), C_Occurrence[q4]]) /\ A intersect B subsetEq C
    Answer 1: set-comprehension{Occurrence}: Vertical rule for set-comprehension, Occurrence representation
              [q5 = q4 | q5 <- A, q5 in B]
              ~~>
              [q5 = q4 | q5 : int(0..6), A_Occurrence[q5], q5 in B]
``` 
 
7. "q5 in B" is replaces with vertical, Occurrence-specific rule for set-in. simply checks the boolean value of the q5th entry in the occurrence matrix. 
```
Picking the first option: Question 1: q5 in B
                              Context #1: [q5 = q4 | q5 : int(0..6), A_Occurrence[q5], q5 in B]
                              Context #3: [or([q5 = q4 | q5 : int(0..6), A_Occurrence[q5], q5 in B]) | q4 : int(0..6), C_Occurrence[q4]]
                              Context #5: [and([or([q5 = q4 | q5 : int(0..6), A_Occurrence[q5], q5 in B]) | q4 : int(0..6), C_Occurrence[q4]]),
                                           A intersect B subsetEq C;
                                               int(1..2)]
    Answer 1: set-in{Occurrence}: Vertical rule for set-in, Occurrence representation
              q5 in B ~~> B_Occurrence[q5]
```
8. restructuring ("inlining conditions") inside or and and statements in turn:
```
Picking the first option: Question 1: [q5 = q4 | q5 : int(0..6), A_Occurrence[q5], B_Occurrence[q5]]
                              Context #1: or([q5 = q4 | q5 : int(0..6), A_Occurrence[q5], B_Occurrence[q5]])
                              Context #3: and([or([q5 = q4 | q5 : int(0..6), A_Occurrence[q5], B_Occurrence[q5]]) | q4 : int(0..6), C_Occurrence[q4]])
                              Context #5: and([or([q5 = q4 | q5 : int(0..6), A_Occurrence[q5], B_Occurrence[q5]]) | q4 : int(0..6), C_Occurrence[q4]]) /\
                                          A intersect B subsetEq C
    Answer 1: inline-conditions: Inlining conditions, inside or
              [q5 = q4 | q5 : int(0..6), A_Occurrence[q5], B_Occurrence[q5]]
              ~~>
              [A_Occurrence[q5] /\ B_Occurrence[q5] /\ q5 = q4 | q5 : int(0..6)] 

Picking the first option: Question 1: [or([A_Occurrence[q5] /\ B_Occurrence[q5] /\ q5 = q4 | q5 : int(0..6)])
                                           | q4 : int(0..6), C_Occurrence[q4]]
                              Context #1: and([or([A_Occurrence[q5] /\ B_Occurrence[q5] /\ q5 = q4 | q5 : int(0..6)]) | q4 : int(0..6), C_Occurrence[q4]])
                              Context #3: and([or([A_Occurrence[q5] /\ B_Occurrence[q5] /\ q5 = q4 | q5 : int(0..6)]) | q4 : int(0..6), C_Occurrence[q4]]) /\
                                          A intersect B subsetEq C
    Answer 1: inline-conditions: Inlining conditions, inside and
              [or([A_Occurrence[q5] /\ B_Occurrence[q5] /\ q5 = q4
                       | q5 : int(0..6)])
                   | q4 : int(0..6), C_Occurrence[q4]]
              ~~>
              [C_Occurrence[q4] ->
               or([A_Occurrence[q5] /\ B_Occurrence[q5] /\ q5 = q4
                       | q5 : int(0..6)])
                   | q4 : int(0..6)] 
```
9. the process is repeated for the right hand side of the conjunction in step 1, leading to a similar but simpler expression due to the order of operations.

### Final Rewritten Model:

```
find A_Occurrence: matrix indexed by [int(0..6)] of bool
    find B_Occurrence: matrix indexed by [int(0..6)] of bool
    find C_Occurrence: matrix indexed by [int(0..6)] of bool
    branching on [A_Occurrence, B_Occurrence, C_Occurrence]
    such that
        and([C_Occurrence[q4] -> or([A_Occurrence[q5] /\ B_Occurrence[q5] /\ q5 = q4 | q5 : int(0..6)]) | q4 : int(0..6)]),
        and([A_Occurrence[q6] /\ B_Occurrence[q6] -> C_Occurrence[q6] | q6 : int(0..6)])
```