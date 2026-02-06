[//]: # (Author: Leia McAlister-Young)
[//]: # (Last Updated: 15/12/2025)

# Essence Parser
The parser converts incoming Essence programs in Conjure Oxide to the [Model Object](https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/ast/struct.Model.html) that the rule engine takes in. The relevant parts of the Model object are the SymbolTable and Expression objects. The symbol table is essentially a list of the variables and their corresponding domains. The Expression object is a recursive object that holds all the constraints of the problem, nested into one object. The parser has two main parts. The first is the `tree-sitter-essence` crate, which is a general Essence parser using the library tree-sitter. The second part is the `conjure-cp-essence-parser` crate which is Rust code that uses the grammar to parse Essence programs and convert them into the above-mentioned Model object.

# Tree Sitter Grammar
[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) is a parsing library that creates concrete syntax trees from programs in various languages. It contains many languages already, but Essence is unfortunately not one of them. Therefore, the [tree-sitter-essence](https://github.com/conjure-cp/conjure-oxide/tree/main/crates/tree-sitter-essence) crate contains a JavaScript grammar for Essence, which tree-sitter uses to create a parser. The parser is not specific to Conjure Oxide as the grammar merely describes the general Essence language, but it is used in and developed for Conjure Oxide so currently covers only parts of the Essence language that Conjure Oxide deals with and has tests written for. The grammar is based on [this Essence documentation](https://conjure.readthedocs.io/en/latest/essence.html).

## General Structure
At the top level, there can be either find statements, letting statements, or constraint statements. Find statements consist of the keyword `find`, one or more variables, and then a domain. Letting statements have the keyword `letting`, one or more variables, and an expression or domain to assign to those variables. Constraints contain the keyword `such that` and one or more logical or numerical expressions which include variables and constants. 

## Domains
Domains in Essence specify the set of possible values that a variable can take. The grammar currently supports the following types of domains:

- **`bool_domain`**: Boolean values (`bool`)
- **`int_domain`**: Integer values, which can be unbounded (`int`) or bounded by ranges (`int(1..10)`, `int(1..5, 10..15)`)
- **`tuple_domain`**: Tuples of multiple domains (`(int(1..5), bool)`)
- **`matrix_domain`**: Multi-dimensional arrays indexed by domains (`matrix indexed by [int(1..5)] of int(0..9)`)
- **`record_domain`**: Named records with typed fields (`record {x: int, y: bool}`)
- **`set_domain`**: Sets with optional size constraints (`set of int(1..10)`, `set (minSize 2, maxSize 5) of bool`)
- **`variable_domain`**: Domain references using identifiers, allowing domains to be parameterized or defined elsewhere

## Expression Hierarchy 
Expressions in the grammar are broken down into boolean expressions, comparison expressions, arithmetic expressions, and atoms. This separation helps enforce semantic constraints inherent to the language. For example, expressions like `x + 3 = y` are allowed because an arithmetic expression is permitted on either side of a comparison, but chained comparisons like `x = y = 3` are disallowed, since a comparison expression cannot itself contain another comparison expression as an operand. This also helps ensure the top-most expression in a constraint evaluates to a boolean (so `such that x + 3` wouldn't be valid). 

Atoms are the fundamental building blocks and represent constants, identifiers, and structured values (tuples, matrices, records, comprehensions, or slices). Atoms are separate from boolean and arithmetic expressions in the grammar hierarchy, though they can be used as operands in most expression contexts. This means a constraint like `such that y` would be valid syntactically even though `y` might be an integer - type checking happens at a later stage. List combining expressions are also separated into boolean and arithmetic variants for this reason (so `such that allDiff([a, b])` is valid but `such that min([a,b])` isn't).

The precedence levels throughout the grammar are based on the Essence Prime operator precedence table found in Appendix B of the [Savile Row Manual](https://arxiv.org/pdf/2201.03472). This is important to ensure that nested or complicated expressions such as `(2*x) + (3*y) = 12` are parsed in the correct order, as this will determine the structure of the Expression object in the Model.

## Limitations of the Tree-sitter Grammar

### Keyword Exclusion
Currently, the grammar allows for any combination of letters, numbers, and underscores to be parsed as variable identifiers. This includes reserved keywords of the Essence language such as 'find' or 'letting'. This is incorrect and such keywords shouldn't be allowed as variables. Tree-sitter doesn't support lookahead assertions or negative matches, so grammar rules cannot exclude specific patterns or words from identifier rules. Checking for keywords as variables happens during semantic checking (see Diagnostics section).

### Error Detection
Error detection from tree-sitter is unpredictable and can be confusing, as it simply creates ERROR nodes in the parse tree without detailed information about what was invalid. Similar errors will produce ERROR nodes in different places and impact the surrounding tree strucure in varying ways, depending on the context. More sophisticated error detection and messsaging has been implemented through Diagnostics.

# Rust Parser
This is the second part of the parser and is contained in the [conjure-cp-essence-parser](https://github.com/conjure-cp/conjure-oxide/tree/main/crates/conjure-cp-essence-parser) crate. The primary function is `parse_essence_file_native`, shown below, which takes in the path to the input and the context and returns the Model or an error. 

```Rust
pub fn parse_essence_file_native(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {...}
```

Within that function, the source code is read from the input and the tree-sitter grammar is used to parse that code and produce a parse tree. From there, a Model object is created and the `SymbolTable` and `Expression` fields are populated by traversing and extracting information from the parse tree.

## General Structure
The top level nodes (children of the root node), are either extras (comments or language labels), find statements, letting statements, or constraints. Find and letting statements provide info that is added to the SymbolTable while constraints are added to the Expression.

The parser crate is organized into multiple modules, each handling a specific aspect of Essence parsing:

- **`parse_model.rs`**: Entry point containing `parse_essence_file_native`. Handles top-level statement parsing (find, letting, constraints) and orchestrates the overall parsing process.
- **`expression.rs`**: Core expression parsing logic. Handles boolean expressions, comparisons, arithmetic operations, and dispatches to specialized parsers.
- **`atom.rs`**: Parses atomic expressions - constants, identifiers, tuples, matrices, records, set literals, and indexing/slicing operations.
- **`domain.rs`**: Domain parsing including integer domains (ranges), boolean domains, and matrix domains. Handles both ground domains (fully specified) and unresolved domains (containing references).
- **`find.rs`**: Parses find statements, extracting variable names and their domains for the symbol table.
- **`letting.rs`**: Parses letting statements, which can bind variables to either domains or expressions.
- **`comprehension.rs`**: Parses comprehensions, quantifiers (forAll, exists), and aggregate expressions (sum, product, min, max, etc.). This is a complex module handling generator expressions and nested quantification.
- **`abstract_literal.rs`**: Parses abstract literal structures like matrices, tuples, and records.

### Layered Dispatch Pattern

The parser is designed with a hierarchical dispatch architecture where each module has a main entry point function that delegates to more specific functions based on the tree-sitter node type. This creates layers of abstraction:

1. **Top-level dispatcher**: Examines the node's `kind()` to determine which category of parser to call (e.g., expression, domain, statement)
2. **Category-level parsers**: Further dispatch to specialized functions based on the specific variant (e.g., `parse_binary_expression`, `parse_atom`, `parse_int_domain`)
3. **Specialized parsers**: Handle the actual parsing logic for specific node types

This layered approach mirrors the structure of the tree-sitter grammar itself, where rules contain choices of subrules. This pattern keeps the codebase maintainable by separating concerns and making additions straightforward.

### General utils
`kind()` is used to determine which rule a node represents and the corresponding function or logic is then applied to that node. Child nodes are found using their field names or indexes and the `named_children()` function is used to iterate over the named child nodes of a node. The function `child_expr` returns the Expression parsed from the first named child of the given node.

### Extracting from the source code (identifiers and constants)
The tree-sitter nodes have a start and end byte indicating where the node corresponds to in the source code. For variable identifiers, constants, and operators, these bytes are necessary to extract the actual values from the source code. 

For example, the following code appears in the `parse_find_statement` function and is used to extract the specific variable name from the source code, which is represented simply by a tree-sitter node (named `variable` in this case).

```Rust
let variable_name = &source_code[variable.start_byte()..variable.end_byte()];
```

Another example is when parsing expressions, the node representing the operator is not always 'named', meaning it is not its own rule in the grammar and rather specified directly in the rule (ex. `exponent: $ => seq($.arithmetic_expr, "**", $.arithmetic_expr)` (simplified)), or the operator one of multiple choices in a rule (ex. `mulitcative_op: $ => choice("*", "/", "%")`). In this situation, the same method as for the variable identifiers is used:

```Rust
let op = constraint.child_by_field_name("operator").ok_or(format!(
        "Missing operator in expression {}",
        constraint.kind()
    ))?;
let op_type = &source_code[op.start_byte()..op.end_byte()];
```

## Find and Letting Statements
Find and letting statements are parsed relatively intuitively. For find statements, the list of variables is iterated over and each is added to a hash map as the key. Then, the domain is parsed and added as the value for each variable it applies to. Once all variables and domains are parsed, the hash map is returned and the caller function iterates over it and adds each pair to the `SymbolTable`. For letting statements, a new `SymbolTable` is created. The variables are again iterated over and added to the table. Letting statements can either specify a domain or an expression for the variables so the type of node is checked and either parsed as an expression or domain before being added to the table. The `SymbolTable` is returned and the caller function adds it to the existing `SymbolTable` of the `Model`.

## Ground vs Unresolved Domains
The parser supports both **ground domains** (fully specified, e.g., `int(1..10)`) and **unresolved domains** (containing variable references, e.g., `int(1..n)`). For unresolved domains, the variable pointer must be retrieved from the existing `SymbolTable`, hich was described in the previous section. Unresolved domains are resolved during later stages of processing once all variables are known. 

## Constraints
Adding constraints to the overall constraints `Expression` requires nesting `Expression` objects. Each constraint is parsed and then added to the model using the `add_constraints` function. 

Following the layered dispatch pattern, expressions are organized into types (again mirroring the grammar):

- **`parse_expression`** in `expression.rs`: The main entry point that dispatches to specialized parsers based on node type
- **`parse_binary_expression`**: Handles binary operations
- ** `parse_arithmetic_expression`**: Handles arithmetic operations
- ** `parse_comparison_expression`**: Handles comparison operations
- **`parse_atom`** in `atom.rs`: Handles atomic expressions
- **`parse_comprehension`** in `comprehension.rs`: Handles comprehensions, quantifiers, and aggregates

## Comprehensions, Quantifiers, and Aggregates

Comprehensions are a significant feature in Essence that require special parsing logic beyond the standard node-to-expression pattern. They are parsed by the `comprehension.rs` module using the specialized `ComprehensionBuilder` type that manages the complex scoping rules inherent to comprehensions.

Comprehensions create their own scope with local variables (generators). The parsing process involves:

1. **Creating a ComprehensionBuilder**: Initialized with the parent symbol table
2. **Setting up scoped symbol tables**: The builder creates two child symbol tables:
   - **Generator symbol table**: Contains generator variables as regular variables (for parsing conditions)
   - **Return expression symbol table**: Contains generator variables as "givens" (for parsing the return expression)
3. **Parsing generators**: Each generator (`var : domain` or `var <- collection`) is parsed and added to both symbol tables
4. **Parsing conditions**: Conditions are parsed using the generator symbol table, allowing them to reference generator variables
5. **Parsing the return expression**: Parsed using the return expression symbol table where generators appear as givens
6. **Building the comprehension**: The builder combines all components into a `Comprehension` object

Example: For `[x + 1 | x : int(1..5), x > 2]`:
- Generator `x : int(1..5)` is added to both symbol tables
- Condition `x > 2` is parsed with `x` in scope as a variable
- Return expression `x + 1` is parsed with `x` in scope as a given
- The result is wrapped in a `Comprehension` expression

### Quantifiers and Aggregates

Quantifier expressions (`forAll`, `exists`) and aggregate expressions (`sum`, `min`, `max`) follow the same comprehension-based parsing approach through `parse_quantifier_or_aggregate_expr`. They:

1. Use `ComprehensionBuilder` to set up scoped symbol tables
2. Parse generators to add variables to the scope
3. Parse the body expression using the return expression symbol table
4. Build a comprehension with an appropriate AC operator kind (`And` for `forAll`, `Sum` for `sum`, etc.)
5. Wrap the comprehension in the corresponding expression type (`Expression::And`, `Expression::Sum`, etc.)

This unified approach ensures consistent scoping behavior across all comprehension-style constructs in Essence.

# Testing

## Integration Testing
The native parser (`parse_essence_file_native`) is used by default for all integration tests in the `tests-integration` crate, which include a range of tests for every feature currently supported. Also incuded in the integraiton testing suite is roundtrip testing. Some of the roundtrip tests test the parser's error messages.

To explicitly disable the native parser for a specific test, add a `config.toml` file in the test directory with:
```toml
enable_native_parser = false
```

This will cause the test to fall back to the legacy parser instead.

## Parser-Specific Tests
The `conjure-cp-essence-parser` crate contains its own unit tests that specifically test parser functionality. These tests can be run with:
```bash
cargo test -p conjure-cp-essence-parser
```

For more details on parser testing, see the parser testing documentation.

