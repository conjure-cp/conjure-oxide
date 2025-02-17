searchState.loadedDescShard("conjure_oxide", 0, "The result of applying a rule to an expression. Contains …\nContains the error value\nRepresents a computational model containing variables, …\nContains the success value\nRepresents the result of applying a rule to an expression …\nA rule with a name, application function, and rule sets.\nA structure representing a set of rules with a name, …\nGets symbols added by this reduction\nApplies side-effects (e.g. symbol table updates)\nGets symbols changed by this reduction\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nGet all rule sets Returns a <code>Vec</code> of static references to …\nGet the dependencies of this rule set, evaluating them …\nSearches recursively in <code>../tests/integration</code> folder for an …\nSearches for an <code>.essence</code> file at the given filepath, then …\nGet a rule by name. Returns the rule with the given name …\nGet a rule set by name. Returns the rule set with the …\nGet all rule sets for a given solver family. Returns a <code>Vec</code> …\nBuild a list of rules to apply (sorted by priority) from a …\nGet the rules of this rule set, evaluating them lazily if …\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe name of the rule set.\nCreates a new model.\nRepresents a reduction with no side effects on the model.\nRegister a rule with the given rule sets and priorities.\nRegister a rule set with the given name, dependencies, and …\nThis module contains the rewrite rules for Conjure Oxides …\nA high-level API for interacting with constraints solvers.\nThe solver families that this rule set applies to.\nThe global symbol table for this model as a reference.\nThe global symbol table for this model as a mutable …\nThe symbol table for this model as a pointer.\nGet the dependencies of this rule set, including itself\nRepresents a reduction that also modifies the symbol table.\nRepresents a reduction that also adds a top-level …\n<code>|x|</code> - absolute value of <code>x</code> <strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput, SAT.\nAn <code>Atom</code> is an indivisible expression, such as a literal or …\nDeclaration of an auxiliary variable.\nAn expression representing “A is valid as long as B is …\nRepresents a decision variable within a computational …\n<strong>Supported by:</strong> JsonInput.\nRepresents different types of expressions used to define …\nEnsures that x=|y| i.e. x is the absolute value of y.\n<code>ineq(x,y,k)</code> ensures that x &lt;= y + k.\nEnsures that x =-y, where x and y are atoms.\nEnsures that x*y=z.\nEnsures that sum(vec) &gt;= x.\nEnsures that sum(vec) &lt;= x.\n<code>w-literal(x,k)</code> ensures that x == k, where x is a variable …\n<code>weightedsumgeq(cs,xs,total)</code> ensures that cs.xs &gt;= total, …\n<code>weightedsumleq(cs,xs,total)</code> ensures that cs.xs &lt;= total, …\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\nEnsures that <code>a-&gt;b</code> (material implication). <strong>Supported by:</strong> …\n<strong>Supported by:</strong> JsonInput.\nA literal value, equivalent to constants in Conjure.\n<strong>Supported by:</strong> JsonInput.\nA name generated by Conjure-Oxide.\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\nEnsures that floor(x/y)=z. Always true when y=0.\nEnsures that x%y=z. Always true when y=0.\nEnsures that <code>x**y = z</code>.\n<code>reify(constraint,r)</code> ensures that r=1 iff <code>constraint</code> is …\n<code>reifyimply(constraint,r)</code> ensures that <code>r-&gt;constraint</code>, where …\nBinary subtraction operator\nRepresents a computational model containing variables, …\nA reference to an object stored in the [<code>SymbolTable</code>].\nNegation: <code>-x</code> <strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput.\n<strong>Supported by:</strong> JsonInput, SAT.\n<strong>Supported by:</strong> JsonInput, SAT.\n<strong>Supported by:</strong> JsonInput.\nThe top of the model\nDivision after preventing division by zero, usually with a …\nModulo after preventing mod 0, usually with a bubble\n<code>UnsafePow</code> after preventing undefinedness\n<strong>Supported by:</strong> JsonInput.\nThe global symbol table, mapping names to their …\nDivision with a possibly undefined value (division by 0) …\nModulo with a possibly undefined value (mod 0) <strong>Supported </strong>…\nUnsafe power<code>x**y</code> (possibly undefined)\nA name given in the input model.\nReturn an unoptimised domain that is the result of …\nReturns the possible values of the expression, recursing …\nExtends the symbol table with the given symbol table, …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns an arbitrary variable name that is not in the …\nTrue iff self and other are both atomic and identical.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nIterates over symbol table entries in scope.\nIterates over entries in the local symbol table only.\nTrue if the expression is an associative and commutative …\nChecks whether this expression is safe.\nCreates an empty symbol table.\nFunctions for pretty printing Conjure models.\nLooks up the return type for name if it has one and is in …\nLooks up the return type for name if has one and is in the …\nSerde serialization/ deserialization helpers.\nReturn a list of all possible i32 values in the domain if …\nCreates an empty symbol table with the given parent.\nA specific kind of declaration.\nThis declaration as a domain letting, if it is one.\nThis declaration as a mutable domain letting, if it is one.\nThis declaration as a value letting, if it is one.\nThis declaration as a mutable value letting, if it is one.\nThis declaration as a decision variable, if it is one.\nThis declaration as a mutable decision variable, if it is …\nThe domain of this declaration, if it is known.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe kind of this declaration.\nCreates a new declaration.\nCreates a new domain letting declaration.\nCreates a new value letting declaration.\nCreates a new decision variable declaration.\nRepresents a computational model containing variables, …\nA model that is de/serializable using <code>serde</code>.\nReturns the argument unchanged.\nInitialises the model for rewriting.\nCalls <code>U::from(self)</code>.\nPretty prints, in essence syntax, the declaration for the …\nPretty prints a <code>Vec&lt;Expression&gt;</code> as if it were a …\nPretty prints a <code>Vec&lt;Expression&gt;</code> as if it were a top level …\nPretty prints, in essence syntax, the declaration for the …\nPretty prints, in essence syntax, the variable declaration …\nPretty prints a <code>Vec&lt;T&gt;</code> in a vector like syntax.\nA type that can be created with default values and an id.\nA type with an [<code>ObjectId</code>].\nA unique id, used to distinguish between objects of the …\nDe/Serialize an <code>Rc&lt;RefCell&lt;T&gt;&gt;</code> as the id of the inner …\nDe/Serialize an <code>Rc&lt;RefCell&lt;T&gt;&gt;</code> as its inner value <code>T</code>.\nCreates a new default value of type <code>T</code>, but with the given …\nReturns the argument unchanged.\nReturns the argument unchanged.\nThe id of this object.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nSomething with a return type\nReturns the default rule sets, excluding solver specific …\nChecks if the conjure executable is present in PATH and if …\nThe result of applying a rule to an expression. Contains …\nContains the error value\nContains the success value\nRepresents the result of applying a rule to an expression …\nRepresents errors that can occur during the model …\nA rule with a name, application function, and rule sets.\nHolds a rule and its priority, along with the rule set it …\nA structure representing a set of rules with a name, …\nReturns the argument unchanged.\nReturns the argument unchanged.\nGet all rule sets Returns a <code>Vec</code> of static references to …\nReturns a copied <code>Vec</code> of all rules registered with the …\nGet a rule by name. Returns the rule with the given name …\nGet a rule set by name. Returns the rule set with the …\nGet all rule sets for a given solver family. Returns a <code>Vec</code> …\nBuild a list of rules to apply (sorted by priority) from a …\nGet rules grouped by priority from a list of rule sets.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe name of the rule set.\nRegister a rule with the given rule sets and priorities.\nRegister a rule set with the given name, dependencies, and …\nResolves the final set of rule sets to apply based on …\nRewrites the given model by applying a set of rules to all …\nA naive, exhaustive rewriter for development purposes. …\nThe solver families that this rule set applies to.\nCollection of static elements that are gathered into a …\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nRetrieve a contiguous slice containing all the elements …\nSimplify an expression to a constant if possible Returns: …\nThe search was complete (i.e. the solver found all …\nThe search was incomplete (i.e. it was terminated before …\nReturned from SolverAdaptor when solving is successful.\nAn abstract representation of a constraints solver.\nA common interface for calling underlying solver APIs …\nThe type for user-defined callbacks for use with Solver.\nErrors returned by Solver on failure.\nAn iterator over the variants of SolverFamily\nSolver adaptors.\nAdds the solver adaptor name and family (if they exist) to …\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nGet the solver family that this solver adaptor belongs to\nGets the name of the solver adaptor for pretty printing.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nModifying a model during search.\nRuns the solver on the given model.\nRuns the solver on the given model, allowing modification …\nStates of a <code>Solver</code>.\nA SolverAdaptor for interacting with the Kissat SAT solver.\nA SolverAdaptor for interacting with Minion.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nAn unspecified error has occurred.\nA ModelModifier provides an interface to modify a model …\nThe requested modification to the model has failed.\nA <code>ModelModifier</code> for a solver that does not support …\nThe desired operation is supported by this solver adaptor, …\nThe desired operation is not supported for this solver …\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nThe state returned by <code>Solver</code> if solving has not been …\nThe state returned by <code>Solver</code> if solving has been …\nCannot construct this from outside this module.\nCannot construct this from outside this module.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nCalls <code>U::from(self)</code>.\nExecution statistics.\nThe status of the search\nReturns the argument unchanged.\nCalls <code>U::from(self)</code>.\nRecursively sorts the keys of all JSON objects within the …\nSort the “variables” field by name. We have to do this …")