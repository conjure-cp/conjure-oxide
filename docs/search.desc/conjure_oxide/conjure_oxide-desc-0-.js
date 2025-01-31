searchState.loadedDescShard("conjure_oxide", 0, "The result of applying a rule to an expression. Contains …\nContains the error value\nRepresents a computational model containing variables, …\nContains the success value\nRepresents the result of applying a rule to an expression …\nA rule with a name, application function, and rule sets.\nA structure representing a set of rules with a name, …\nAdds a decision variable to the model.\nGets symbols added by this reduction\nApplies side-effects (e.g. symbol table updates)\nGets symbols changed by this reduction\nExtends the models symbol table with the given symbol …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns an arbitrary variable name that is not in the …\nGet the dependencies of this rule set, evaluating them …\nGets the domain of <code>name</code> if it exists and has one.\nSearches recursively in <code>../tests/integration</code> folder for an …\nSearches for an <code>.essence</code> file at the given filepath, then …\nGet a rule by name. Returns the rule with the given name …\nGet a rule set by name. Returns the rule set with the …\nGet all rule sets Returns a <code>Vec</code> of static references to …\nGet all rule sets for a given solver family. Returns a <code>Vec</code> …\nReturns a copied <code>Vec</code> of all rules registered with the …\nGet the rules of this rule set, evaluating them lazily if …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe name of the rule set.\nCreates a new model.\nOrder of the RuleSet. Used to establish a consistent order …\nRepresents a reduction with no side effects on the model.\nRegister a rule with the given rule sets and priorities.\nRegister a rule set with the given name, priority, and …\nThis module contains the rewrite rules for Conjure Oxides …\nA high-level API for interacting with constraints solvers.\nThe solver families that this rule set applies to.\nThe global symbol table for this model.\nThe global symbol table for this model, as a mutable …\nGet the dependencies of this rule set, including itself\nRepresents a reduction that also modifies the symbol table.\nRepresents a reduction that also adds a top-level …\n<code>|x|</code> - absolute value of <code>x</code> <strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput, SAT.\nAn <code>Atom</code> is an indivisible expression, such as a literal or …\nDeclaration of an auxiliary variable.\nAn expression representing “A is valid as long as B is …\nRepresents a decision variable within a computational …\n<strong>Supported by:</strong> JsonInput.\nRepresents different types of expressions used to define …\nEnsures that x=|y| i.e. x is the absolute value of y.\n<code>ineq(x,y,k)</code> ensures that x &lt;= y + k.\nEnsures that x =-y, where x and y are atoms.\nEnsures that x*y=z.\nEnsures that sum(vec) &gt;= x.\nEnsures that sum(vec) &lt;= x.\n<code>w-literal(x,k)</code> ensures that x == k, where x is a variable …\n<code>weightedsumgeq(cs,xs,total)</code> ensures that cs.xs &gt;= total, …\n<code>weightedsumleq(cs,xs,total)</code> ensures that cs.xs &lt;= total, …\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\nEnsures that <code>a-&gt;b</code> (material implication). <strong>Supported by:</strong> …\n<strong>Supported by:</strong> JsonInput.\nA literal value, equivalent to constants in Conjure.\n<strong>Supported by:</strong> JsonInput.\nA name generated by Conjure-Oxide.\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\nEnsures that floor(x/y)=z. Always true when y=0.\nEnsures that x%y=z. Always true when y=0.\nEnsures that <code>x**y = z</code>.\n<code>reify(constraint,r)</code> ensures that r=1 iff <code>constraint</code> is …\n<code>reifyimply(constraint,r)</code> ensures that <code>r-&gt;constraint</code>, where …\nBinary subtraction operator\nA reference to an object stored in the <code>SymbolTable</code>.\nNegation: <code>-x</code> <strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput, SAT.\n<strong>Supported by:</strong> JsonInput, SAT.\n<strong>Supported by:</strong> JsonInput.\nDivision after preventing division by zero, usually with a …\nModulo after preventing mod 0, usually with a bubble\n<code>UnsafePow</code> after preventing undefinedness\n<strong>Supported by:</strong> JsonInput.\nThe global symbol table, mapping names to their …\nDivision with a possibly undefined value (division by 0) …\nModulo with a possibly undefined value (mod 0) <strong>Supported </strong>…\nUnsafe power<code>x**y</code> (possibly undefined)\nA name given in the input model.\nAdds a value letting to the symbol table as <code>name</code>.\nAdds a decision variable to the symbol table as <code>name</code>.\nReturn an unoptimised domain that is the result of …\nReturns the possible values of the expression, recursing …\nGets the domain of <code>name</code> if it exists and has a domain.\nGets the domain of <code>name</code> as a mutable reference if it …\nExtends the symbol table with the given symbol table, …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns an arbitrary variable name that is not in the …\nReturns a reference to the value letting with the given …\nReturns a mutable reference to the value letting with the …\nReturns a reference to the decision variable with the …\nReturns a mutable reference to the decision variable with …\nTrue iff self and other are both atomic and identical.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nTrue if the expression is an associative and commutative …\nChecks whether this expression is safe.\nReturns an iterator over the names and definitions of all …\nReturns a mutable iterator over the names and definitions …\nReturns an iterator over the names and definitions of all …\nReturns an iterator over the names in the symbol table.\nReturns an iterator over the names in the symbol table.\nCreates an empty symbol table.\nFunctions for pretty printing Conjure models.\nGets the type of <code>name</code> if it exists and has a type.\nUpdates a value letting to the symbol table as <code>name</code>, or …\nUpdates a decision variable to the symbol table as <code>name</code>, …\nReturn a list of all possible i32 values in the domain if …\nPretty prints a <code>Vec&lt;Expression&gt;</code> as if it were a …\nPretty prints a <code>Vec&lt;Expression&gt;</code> as if it were a top level …\nPretty prints, in essence syntax, the declaration for the …\nPretty prints, in essence syntax, the variable declaration …\nPretty prints a <code>Vec&lt;T&gt;</code> in a vector like syntax.\nReturns the default rule sets, excluding solver specific …\nChecks if the conjure executable is present in PATH and if …\nThe result of applying a rule to an expression. Contains …\nContains the error value\nContains the success value\nRepresents the result of applying a rule to an expression …\nRepresents errors that can occur during the model …\nA rule with a name, application function, and rule sets.\nA structure representing a set of rules with a name, …\nReturns the argument unchanged.\nGet a rule by name. Returns the rule with the given name …\nConvert a list of rule sets into a final map of rules to …\nGet a rule set by name. Returns the rule set with the …\nGet all rule sets Returns a <code>Vec</code> of static references to …\nGet all rule sets for a given solver family. Returns a <code>Vec</code> …\nReturns a copied <code>Vec</code> of all rules registered with the …\nGet a final ordering of rules based on their priorities …\nCalls <code>U::from(self)</code>.\nThe name of the rule set.\nOrder of the RuleSet. Used to establish a consistent order …\nRegister a rule with the given rule sets and priorities.\nRegister a rule set with the given name, priority, and …\nResolves the final set of rule sets to apply based on …\nRewrites the given model by applying a set of rules to all …\nA naive, exhaustive rewriter for development purposes. …\nThe solver families that this rule set applies to.\nCollection of static elements that are gathered into a …\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nRetrieve a contiguous slice containing all the elements …\nSimplify an expression to a constant if possible Returns: …\nThe search was complete (i.e. the solver found all …\nThe search was incomplete (i.e. it was terminated before …\nReturned from SolverAdaptor when solving is successful.\nAn abstract representation of a constraints solver.\nA common interface for calling underlying solver APIs …\nThe type for user-defined callbacks for use with Solver.\nErrors returned by Solver on failure.\nAn iterator over the variants of SolverFamily\nSolver adaptors.\nAdds the solver adaptor name and family (if they exist) to …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nGet the solver family that this solver adaptor belongs to\nGets the name of the solver adaptor for pretty printing.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nModifying a model during search.\nRuns the solver on the given model.\nRuns the solver on the given model, allowing modification …\nStates of a <code>Solver</code>.\nA SolverAdaptor for interacting with the Kissat SAT solver.\nA SolverAdaptor for interacting with Minion.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nAn unspecified error has occurred.\nA ModelModifier provides an interface to modify a model …\nThe requested modification to the model has failed.\nA <code>ModelModifier</code> for a solver that does not support …\nThe desired operation is supported by this solver adaptor, …\nThe desired operation is not supported for this solver …\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe state returned by <code>Solver</code> if solving has not been …\nThe state returned by <code>Solver</code> if solving has been …\nCannot construct this from outside this module.\nCannot construct this from outside this module.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nExecution statistics.\nThe status of the search\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nRecursively sorts the keys of all JSON objects within the …\nSort the “variables” field by name. We have to do this …")