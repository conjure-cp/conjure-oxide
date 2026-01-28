## SAT Booleans

The most basic form of a constraint is a boolean expression. SAT solvers work with boolean variables, but require input in Conjunctive Normal Form (which will be referred to as CNF). This means that any boolean expression must be converted to a series of clauses composed entirely of atoms (true/false values or variables). Here's an example:

Original Expression:

$$((A \land B) \to (C \lor \lnot D)) \land E$$

In CNF:

$$(\lnot A \lor \lnot B \lor C \lor \lnot D) \land (E)$$

The simplest approach to this conversion is by repeatedly applying rules (de morgans, double negation, distributivity, etc) until the expression is in CNF. This works well for smaller expressions but slows down massively when dealing with larger expressions (like the once generated when dealing with integers). To solve this, we use Tseytin transformations (see <https://en.wikipedia.org/wiki/Tseytin_transformation>) - these transformations allow us to convert an operation straight into CNF by introducing new auxiliary variables. This method makes rule applications significantly faster but with the downside of producing longer CNF with more variables to solve to give to the solver. Modern SAT solvers, however, are very efficient and this extra performance cost is negligible compared to the time saved in conversion.
