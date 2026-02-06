## Creating Transformation Rules

The logic behind a SAT encoding will vary depending on the expression, but any transformation rule should be constructed in a particular way (regardless of which encoding type is being used):

* Input validation - this is standard for any rule, the rule must check the input expression is the target expression for this rule. Some rules may also further check that the sub-components of the expression are also valid.
* Extract the raw data - the raw data from the input expression needs to be extracted, for integer operations this means extracting the operand bit vectors and ranges.
* Create the new expression with CNF clauses - the returned expression should be constructed using only boolean expressions; the AST has expressions for most of the common boolean operations, but the dedicated `tseytin_...` functions should be used within SAT rules. This is because tseytin transformations directly create the CNF clauses for a boolean operation, significantly reducing the number of rule applications needed to get a solver-ready input. Tseytin transformations add new CNF clauses and create new auxillary variables so a mutable clauses vector and symbol table should be passed to any transformation to modify in-place. Some rules may encapsulate this process in another function, this is helpful for common operations that may be used within multiple rules.
* Domain propogation (Integers only) - for integers, the range of values a `SATInt` integer can take is stored using a closed range in the form of a tuple. When an operation returns a new `SATInt` the range must be updated to reflect the new range of the returned integer. The length of the bitvector should also be changed to reflect this change.
* Return the result - any transformation rule involving SAT encodings should use `Reduction::cnf(..)` to return the created expression and new CNF clauses and symbol table.

### Example - negation of log integers
Here is a worked example; this is the rule that encodes the negation operation between two log-encoded `SATInt`s
```rust
/// Converts negation of a SATInt to a SATInt
///
/// ```text
/// -SATInt(a) ~> SATInt(b)
///
/// ```
#[register_rule(("SAT", 4100))]
fn cnf_int_neg(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (min, max)) = expr.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_log_int_operands(vec![expr.as_ref().clone()], None)?;
    let [bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let result = tseytin_negate(bits, bits.len(), &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (-max, -min),
        ),
        new_clauses,
        new_symbols,
    ))
}
```
#### Input validation & extracting the raw data
Extraction and validation tend to be closely related so many functions perform both at once.

The first step is simply checking that the input is actually a negation expression.
```rust
fn cnf_int_neg(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };
    ...
```
Next, we extract the range for the integer we are negating.
```rust
    ...
    let Expr::SATInt(_, _, _, (min, max)) = expr.as_ref() else {
        return Err(RuleNotApplicable);
    };
    ...
```
Now, we extract the input bitvectors. The logic for this is held in `validate_log_int_operands`, which also checks that the input is formed correctly and actually uses log-encoding.
```rust
    ...
    let binding = validate_log_int_operands(vec![expr.as_ref().clone()], None)?;
    let [bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };
    ...
```
#### Creating the new expression
Negation is an integer operation, it takes an integer input and returns an integer output. This means the result should be stored as a log-encoded `SATInt`. For this specific rule, the logic is encapsulated in a separate function (as integer negation is a common operation within other, more complex operations) you can see that we created new structs for the tseytin transformation to modify.
```rust
    ...
    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let result = tseytin_negate(bits, bits.len(), &mut new_clauses, &mut new_symbols);
    ...
```

#### Domain propogation & returning the result
For negation, caluclating the new domain is very simple:
For 
$` x \in [a, b] `$ The new range is $` -x \in [-b, -a]`$. So here the domain propogation is calulcated within the ouput.

```rust
    ...
    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (-max, -min),
        ),
        new_clauses,
        new_symbols,
    ))
}
```
