## Types of SAT Encodings

There are three different types of SAT Encodings planned in conjure oxide. Of these, only Logarithmic Encodings have been implemented thus far. The three types are these:

- Direct Encodings  
- Logarithmic Encodings  
- Order Encodings  

Each type of encoding has pros and cons, and a different one may be selected based on the type of constraint problem.

### Logarithmic Encoding

The base principle is quite simple: encode an integer as a bitvector. This allows us to represent integers as a series of boolean constraints -- one for each bit. 

For example, the integer '6' can be represented in binary as '0110'. We can then represent this as (P = 0 ∨ 1, Q = 0 ∨ 1, R = 0 ∨ 1, S = 0 ∨ 1). The connection that is missing, however, is that this isn't actually a representation of a constraint problem, but of a solution to a problem.  

### Direct Encoding

Direct encodings are the most straightforward type of encoding - it involves creating a vector of boolean variables, one corresponding to each member of the domain. Only one of these variables can be true at a time, and it is the one corresponding to the value of the integer. 

### Order Encoding

Order follows the same principle as direct encoding, but instead of each boolean variable 'specifying' a value in the way that direct encodings do, each bit specifies whether the integer corresponding to it is less than or equal to the integer variable's value.