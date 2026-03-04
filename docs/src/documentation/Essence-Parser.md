[//]: # (Author: Leia McAlister-Young)
[//]: # (Last Updated: 07/05/2025)

# Overview
The parser converts incoming Essence programs to Conjure Oxide to the [Model Object](https://conjure-cp.github.io/conjure-oxide/docs/conjure_core/ast/struct.Model.html) that the rule engine takes in. The relevant parts of the Model object are the SymbolTable and Expression objects. The symbol table is essentially a list of the variables and their corresponding domains. The Expression object is a recursive object that hold all the constraints of the problem, nested into one object. The parser has two main parts. The first the `tree-sitter-essence` crate, which is a general Essence parser using the library tree-sitter. The second part is the `conjure_essence_parser` crate which is Rust code that uses the grammar to parse Essence programs and convert them into the above-mentioned Model object.

# Tree Sitter Grammar
[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) is a parsing library that creates concrete syntax trees from programs in various languages. It contains many languages already, but Essence is unfortunately not one of them. Therefore, this crate contains a JavaScript grammar for Essence, which tree-sitter uses to create a parser. The parser is not specific to Conjure Oxide as the grammar merely describes the general Essence language, but it is used in Conjure Oxide and currently covers only parts of the Essence language that Conjure Oxide deals with and has tests written for. Therefore, it is not extensive and relatively simple in structure. 

## General Structure
At the top level, there can be either find statements, letting statements, and constraint statements. Find statements consist of the keyword `find`, one or more variables, and then a domain of either type boolean or integer. Letting statements have the keyword `letting`, one or more variables, and an expression or domain to assign to those variables. Constraints contain the keyword `such that` and one or more logical or numerical expressions which include variables and constants. 

## Expression Hierarchy 
Expressions in the grammar are broken down into boolean expressions, comparison expressions, and arithmetic expressions. This separation helps enforce semantic constraints inherent to the language. For example, expressions like `x + 3 = y` are allowed because an arithmetic expression is permitted on either side of a comparison, but chained comparisons like `x = y = 3` are disallowed, since a comparison expression cannot itself contain another comparison expression as an operand. This also helps ensure the top-most expression in a constraint evaluates to a boolean (so `such that x + 3` wouldn't be valid). There are also `atom` expressions such as constants, identifiers, and structured values (tuples, matrices, or slices), which are allowed as operands to most expressions since they might be booleans. This does mean, however, that a constraint like `such that y` would be valid even though `y` might be an integer. Quantifier expressions are also separated into boolean and arithmetic quantifiers for this reason (so `such that allDif{[a, b]}` is valid but `such that min{[a,b]}` isn't).

The precedence levels throughout the grammar are based on the Essence prime operator precedence table found in Appendix B of the [Savile Row Manual](https://arxiv.org/pdf/2201.03472). This is important to ensure that nested or complicated expressions such as `(2*x) + (3*y) = 12` are parsed in the correct order, as this will determine the structure of the Expression object in the Model.

## [Issue #763](https://github.com/conjure-cp/conjure-oxide/issues/763)
Currently, the grammar allows for any combination of letters, numbers, and underscores to be parsed as a variable identifiers. This includes reserved keywords of the Essence language such as 'find' or 'letting'. This is incorrect and such keyword shouldn't be allowed as variables. A solution to this problem hasn't been found. Below are notes about possible solutions.
- Tree-sitter doesn't support lookahead assertions so grammar rules cannot exclude specific patterns or words. 
- Defining rules for each keyword (with higher precedence than the identifier rule) has been brought up but it doesn't stop keywords from being parsed as variables within a rule searching for identifiers (more detail in linked issue).
- It is possible to manually check all identifier nodes in the parse tree within the Rust program against a list of keywords (or by defining rules for keywords and allowing identifiers to be parsed as them or a valid identifier). This would allow for rejecting Essence programs that merely use a keyword as an identifier and have no other errors but wouldn't allow for correct parsing of errors such as the one in line 2 of the program below. The extra '=' in line two causes 'such' in line 3 to be parsed as an identifier and thus the whole of line 3 is parsed as an error, even though it is valid. If 'such' could be excluded as a valid identifier, the error would be only in line 2.

```Essence
find x,y,z : int(0..5)
such that x = y =
such that z = 3
```

# Rust Parser
This is the second part of the parser and is contained in the [conjure_essence_parser](https://conjure-cp.github.io/conjure-oxide/docs/conjure_essence_parser/index.html) crate. The primary function is `parse_essence_file_native`, shown below, which takes in the path to the input and the context and returns the Model or an error. 

```Rust
pub fn parse_essence_file_native(
    path: &str,
    context: Arc<RwLock<Context<'static>>>,
) -> Result<Model, EssenceParseError> {...}
```

Within that function, the source code is read from the input and the tree-sitter grammar is used to parse that code and produce a parse tree. From there, a Model object is created and the `SymbolTable` and `Expression` fields are populated by traversing and extracting information from the parse tree. 

## General Structure and utils
The top level nodes (children of the root node), are either extras (comments or language labels), find statements, letting statements, or constraints. Find and letting statements provide info that is added to the SymbolTable while constraints are added to the Expression. 

In general, `kind()` is used to determine which rule a node represents and the corresponding function or logic is then applied to that node. Child nodes are found using their field names or indexes and the `named_children()` function is used to iterate over the named child nodes of a node. The function `child_expr` returns the Expression parsed from the first named child of the given node. 

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

## Constraints
Adding constraints to the overall constraints `Expression` requires nesting `Expression` objects in a consistent and clear manner. Each constraint is parsed using the `parse_expression` function, which returns an `Expression` object, and is then added to the model using the `add_constraints` function. The `parse_expression` function takes a node as a parameter and is recursive. It matches the node to its 'kind', recursively parses the children of the node, then returns an `Expression` object that correctly packages up the expressions returned from the parsing of the children nodes. 

This structure allows for easy addition of more complex constraints as the grammar evolves. As more rules are added to the grammar, the corresponding code block must be added to the match statement in the `parse_expression` function.

### Example
The constraint `x = 3` would be represented by a `comparison_expr` node, which is defined in the grammar as:
```Javascript
comparison_expr: $ => prec(0, prec.left(seq(
      field("left", choice($.boolean_expr, $.arithmetic_expr)), 
      field("operator", $.comparative_op),
      field("right", choice($.boolean_expr, $.arithmetic_expr))
    ))),
```
In the parse_expression function, the left expression is parsed using the `child_expr` function. The returned Expression would represent the variable `x` and be saved as `expr1`. The operator is extracted from the source code and saved as `op_type`. The right expression node is then found using its field name 'right' and parsed. The returned Expression would represent the integer `3` and be saved as `expr2`.

```Rust
let expr1 = child_expr(constraint, source_code, root)?;
            let op = constraint.child_by_field_name("operator").ok_or(format!(
                "Missing operator in expression {}",
                constraint.kind()
            ))?;
            let op_type = &source_code[op.start_byte()..op.end_byte()];
            let expr2_node = constraint.child_by_field_name("right").ok_or(format!(
                "Missing second operand in expression {}",
                constraint.kind()
            ))?;
            let expr2 = parse_expression(expr2_node, source_code, root)?;

            match op_type {...}
```

Depending on the operator type (in this case `=`), the left and right expressions are added to an Expression of the relevant type (in this case `Eq`), which is returned.

```Rust
"=" => Ok(Expression::Eq(
                    Metadata::new(),
                    Box::new(expr1),
                    Box::new(expr2),
                )),
```