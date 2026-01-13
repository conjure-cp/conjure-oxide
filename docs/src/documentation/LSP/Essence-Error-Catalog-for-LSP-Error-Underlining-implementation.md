[//]: # (Author: Soph Morgulchik)
[//]: # (Last Updated: 31/10/2025)

# Syntax Errors 
(Will produce an invalid CST)

### 1. Missing Token (e.g variable, Domain, Expression)
Description: expected token is absent 
Detection: Empty node or a missing child node. (MISSING node is inserted)
Example: missing variable name 
```text
find: bool
```

Example: missing Expression
```text
letting x be 
```

### 2. Missing Keyword 
Description: a token appears where a different token/keyword is expected 
Detection: ERROR node containing all the tokens where that failed to match the grammar

Example: missing colon, `int` is unexpected (here `x` and `int` will be children of the ERROR node)
```text
find x int
```

### 3. Unexpected Token 
Description: a token appearing after a valid construct is complete
Detection: ERROR node containing the extra token (ERROR node will likely be the last child of a valid construct)
Example: second `)` is extra 
```text
find x: int(1..2))
```

### 4. Invalid Token 
Description: token is malformed or not part of the grammar 
Detection: look for an ERROR node with the invalid token 
Example: `@int` is not a valid token (ERROR node with `@`)
```text
find x: @int
```
Conjur does not identify `int` at all, just says "Skipped token", "Missing Domain"
Tree-Sitter in conjure-oxide sees `int` as a valid token but has an ERROR node with `@` before.
So in Conjure the same error as Missing Token but might make sense to separate due to different CST structures. 

### 5. Unclosed Delimiter 
Description: unmatched or missing closing bracket, parenthesis, brace 
Detection: MISSING node ")"
Example: missing `)` 
```text
find x: int(1..2
```
Conjure allows it but since out grammar enforces it Tree-Sitter produces an error. 

### 6. Unsupported Statements  
Description: statement not recognised by Essence grammar 
Detection: ERROR node with the unrecognised statement 
Example:  
```text
find x: int(1..5)
print x 
```
Conjure doesn't recognise `print x` as a separate invalid statement.

### 7. General Syntax Error
All the other cases that cause an ERROR node in the CST tree. 

# Semantic Errors 
(Will produce a valid CST and AST, now can only detected at runtime, e.g typecheking)

### 1. Keywords as Identifiers  
Description: token's name is not allowed  
Detection: compare the variable names against the set of keywords 
Example: Keyword "bool" used as a variable name
```text
find bool: bool
```
```text
find x: letting
```
Conjure doesn't allow it but Tree-Sitter does so we will have to check as part of semantic checks. 

### 2. Omitted Declaration 
Description: variable not declared 
Detection: save the declared variables and check if the ones used in expressions are in the declared set 
Example: x was not declared before 
```text
find y: int (1..4)
such that x = 5
```

### 3. Invalid Domain
Description: logically or mathematically invalid bounds/domain + infinite domain

Example: a bigger value before smaller 
```text
find x: int(10..5)
```
Conjure doesn't flag as error just says no solutions found. 

Example: infinite domain 
```text
find x: int(1..) 
```
Might want to restrict infinite domain with the grammar so it could be a syntax error. 

### 4. Type Mismatch
Description: attempt to do an operation on illegal types 

Example: cannot add integer and boolean 
```text
letting y be true 
find x: int (5..10)
such that 5 + y = 6
```

### 5. Unsafe division
Description: cannot divide/modulo by zero

Example: cannot divide by zero
```text
find x: int(5..10)
such that x/0 = 3
```
Conjure allows, proceeds to run the solver but just outputs no solutions. We should disallow. 

### 6. Invalid indexing in tuples and matrices 
Description: Tuples and matrices are indexed from . Negative, zero or index out of bounds is invalid

Example: s tuple index of out bounds
```text
letting s be tuple(0,1,1,0)
letting t be tuple(0,0,0,1)
find a : bool such that a = (s[5] = t[1]) $ true
```


# Possible semantic errors per statement type 

(All syntactic errors are relevant for each)
## Declarations 
### Find Statement 
(Declaring decision variables)

Format: _"find"_ **Name** _":"_ **Domain** 

Semantic Errors: Keyword as Token, Invalid Domain

 ### Letting Statement
(Declaring aliases)

Formats: _"letting"_ **Name** _"be"_ **Expression** | _"letting"_ **Name** _"be"_ _"domain"_ **Domain** 

Semantic Errors: All except Omitted Declaration 

### Constraints 
Format: _"such that"_ list(**Expression**, _","_)

Semantic Errors: All except for Invalid Domain 


