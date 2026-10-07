## Min/Max Constraints for Direct and Order Encoding
 
The Min and Max constraints are implemented in Savile Row's Style:
 
"For min(V ) = z we have V [1] = z ∨ V [2] = z . . . and z ≤ V [1] ∧ z ≤ V [2] . . .. Max is similar to min with ≤ replaced by ≥. The constraint element(V, x) = z becomes ∀i : (x ̸= i ∨ V [i] = z)." [Cite]
 
In slightly more human language, To say that z is the minimum of some vector 'v' is really just saying that  z is both:
- less than or equal each value in the vector
- a member of v - that is, it must be equal to one or more elements in v.
 
This works nicely because both of these conditions map nicely into disjunctions, and they can be put inside a conjunction. These characteristics of the encoding ensure that all solvers that support Integers. Since support has been established for integer representations and inequality relations is SAT, it can also be solved using SAT solvers.
 
Maximum can be decomposed into a very similar format: the maximum element of some vector v must be :
- A member of v
- greater than or equal to all elements in v
