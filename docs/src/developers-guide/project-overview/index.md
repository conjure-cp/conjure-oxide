<!-- !TODO: this will need more work -->

# What is Conjure-Oxide
Conjure-Oxide is a constraint modelling tool which is implemented in Rust. Of note, it is a reimplementation of the Conjure automated constraint modelling tool and the Savile Row Modelling Assistant, functionally merged and rewritten in Rust - largely because Rust is an efficient high-performance language. Conjure-Oxide takes in a problem written in a high level constraint language, which is called Essence, and creates a model from this. This model is then transformed into a solver-friendly form, and solved using one of the available back-ends. At the moment, these solver back-ends include Minion (a Constraint Satisfiability Problem (CSP) solver) and in-development translation and bindings to a SAT (Propositional Satisfiability Problem) solver. To put it simply, Conjure-Oxide is a compiler to different modelling languages, the result of which is passed into the respective solver. These solvers act as back-ends and must be given bindings to Conjure-Oxide, but are not actually part of it themselves.
# How is the model made 'solver-friendly'? 
Conjure-Oxide has a rule engine, which applies rules onto the model passed in as Essence, to lower the level of the language into one which can be parsed and solved by the back-end solvers. This engine applies rewriting rules, such as \(x = - y\) to \(x + y = 0\), to transform the model. As Essence is a fairly high-level language, with compatibility for sets and matrices and a number of keywords, there are a lot of rewrite rules which may be needed before the model can be passed into a solver. This translation is one of the key roles of Conjure-Oxide, and this rule application refines the language from a higher level to a lower level. The exact rules applied and the degree of simplification varies on the language requirement of the solver: for example, Minion has higher-level language compatibility than a SAT solver[^1]. 

[^1]: Ian P. Gent, Chris Jefferson, and Ian Miguel. 2006. MINION: A Fast, Scalable, Constraint Solver. In Proceedings of the 2006 conference on ECAI 2006: 17th European Conference on Artificial Intelligence August 29 -- September 1, 2006, Riva del Garda, Italy. IOS Press, NLD, 98–102. 

# Conjure and Savile Row
Also relevant to the testing of Conjure-Oxide is Conjure and Savile Row [^2]. This is because as Conjure-Oxide is a merge and reimplementation of these programs, the testing can be performed comparatively against Conjure and Savile Row. When this document uses the term "Conjure suite", it refers to both Conjure and Savile Row, being used in combination, generally using the Minion solver. Where there is deviation from this, it will be explicitly clarified.

[^2]: Hussain, B. S. 2017. Model Selection and Testing for an Automated Constraint Modelling Toolchain; Thesis

## What is Conjure?
Conjure is a constraint modelling tool, implemented in Haskell. It uses a rule engine similarly to Conjure-Oxide to simplify from Essence to Essence' (a subset of Essence which is slightly lower-level), and this "refinement" produces the lower-level model to pass to Savile Row. 
## What is Savile Row?
Savile Row is a constraint modelling assistant which translates from Essence' to a solver-friendly language. Alongside translating the model to the target language, it can reformat the model with an additional set of rules. This provides a performance increase by improving the model itself. Functionally, the better a model is, the faster a solver will solve it.
## What is Minion?
Minion is a Constraint Satisfiability Problem solver, which has bindings to both Conjure suite and Conjure-Oxide. Minion is a fast, scalable, CSP solver, designed to be capable of solving difficult models while still being time optimised. This is the main solver in use by both the Conjure suite and by Conjure-Oxide, due to its flexibility and efficiency.
