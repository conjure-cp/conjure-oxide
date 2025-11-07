<!-- maturity: draft
authors: Georgii Skorokhod, Hanaa Khan
created: 03-11-25
---- -->

<!-- TODO edit more -->

# Expression rewriting, Rules and RuleSets

> NOTE: Once the rewrite engine API is finalized, we should possibly make a separate page for it

## Overview

Conjure uses Essence, a high-level DSL for constraints modelling, and converts it into a solver-specific representation of the problem.
To do that, we parse the Essence file into an AST, then use a rule engine to walk the expression tree and rewrite constraints into a format that is accepted by the solver before passing the AST to a solver adapter. 

The high-level process is as follows:
1. Start with a deterministically ordered list of rules
2. For each node in the expression tree:
    - Find all rules that can be applied to it
    - If there are none, keep traversing the tree. Otherwise:
        - Take the rules with the highest priority
        - If there is only one, apply it
        - If there are multiple, use some strategy to choose a rule
          (The rule selection logic is separate from the rewrite engine itself)
          For testing, we currently just choose the first rule
3. When there are no more rules to apply, the rewrite is complete

We want the rewrite process to:
- be **flexible** - instead of hard coding the rules, we want an easy way to extend the list of rules and to decide which rules to use, both for ourselves and for any users who may wish to use conjure-oxide in their projects
- be **deterministic** (in a loose sense of the term) - for a given input, set of rules, and a given set of answers to all rule selection questions (see above), the rewriter must always produce the same output
- happen in a single step, instead of doing multiple passes over the model (like it is done in Saville Row currently)

## Rules 

---

### Overview

Rules are the fundamental part of the rewrite engine.
They consist of:
- A unique **name**
- An application function which takes an `Expression` and either rewrites it or errors if the rule is not applicable.
  (checking applicability and applying the rule are not separated to avoid code duplication and inefficiency - their logic is the same)

We may also store other metadata in the `Rule` struct, for example the names of the `RuleSet`'s that it belongs to

```rust
pub struct Rule<'a> {
    pub name: &'a str,
    pub application: fn(&Expression) -> Result<Expression, RuleApplicationError>,
    pub rule_sets: &'a [(&'a str, u8)], // (name, priority). At runtime, we add the rule to rulesets
}
```

### Registering Rules

The main way to register rules is by defining their application function and decorating it with the `#[register_rule()]` macro.
When this macro is invoked, it creates a static `Rule` object and adds it to a global rule registry. Rules may be registered from the `conjure_oxide` crate, or any downstream crate (so, users may define their own rules)

Currently, the `register_rule` macro has the following syntax:
```
#[register_rule(("<RuleSet name>", <Rule priority within the ruleset>))]
```

#### Example

```rust
use conjure_core::ast::Expression;
use conjure_core::rule::RuleApplicationError;
use conjure_rules::register_rule;

#[register_rule(("RuleSetName", 10))]
fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
   Ok(expr.clone())
}
```

### Getting Rules from the registry

Rules may be retrieved using the following functions:

```rust
pub fn get_rule_by_name(name: &str) -> Option<&'static Rule<'static>>
```

```rust
pub fn get_rules() -> Vec<&'static Rule<'static>>
```

- `get_rules()` returns a vector of static references to `Rule` structs
- `get_rule_by_name()` returns a static reference to a specific rule, if it exists

## Rule Sets

---

Rule sets group some `Rule`s together and map them to their priorities.
The `rewrite` function takes a set of `RuleSet`'s and uses it to resolve a final list of rules, ordered by their priority.

The RuleSet object contains the following fields:

- `name` The name of the rule set.
- `order` The order of the rule set.
- `rules` A map of rules to their priorities. This is evaluated lazily at runtime.
- `solvers` The solvers that this RuleSet applies for
- `solver_families` The solver families that this RuleSrt applies for

> NOTE: A `RuleSet` would apply if EITHER of the following is true:
>  - The target solver belongs to its list of `solvers`
>  - The target solver belongs to a family that is listed in `solver_families`, even if it is not explicitly named in `solvers`

It provides the following public methods:

- `get_dependencies() -> &HashSet<&'static RuleSet>` Get the dependency `RuleSet`s of this `RuleSet` (evaluating them lazily if necessary)
- `get_rules() -> &HashMap<&'a Rule<'a>, u8>` Get a map of rules to their priorities (performing lazy evaluation - "reversing the arrows" - if necessary)

### Registering Rule Sets

Like `Rule`s, `RuleSet`s may be registered from anywhere within the `conjure_oxide` crate or any downstream crate.
They are registered using the `register_rule_set!` macro using the following syntax:

```
register_rule_set!("<Rule set name>", <order>, ("<name of dependency RuleSet>", ...), (<list of solver families>), (<list of solvers>));
```

If a bracketed list is omited, the corresponding list would be empty. However, you must not break the order.
For example:
```rust
register_rule_set!("MyRuleSet", 10) // This is legal (rule set will have no dependencies, solvers or solver families)
register_rule_set!("MyRuleSet", 10, ("DependencyRS")) // Also legal
register_rule_set!("MyRuleSet", 10, (SolverFamily::CNF)) // This is illegal because dependencies come before solver families
register_rule_set!("MyRuleSet", 10, (), (SolverFamily::CNF)) // But this is fine
```

#### Example

```rust
register_rule_set!("MyRuleSet", 10, ("DependencyRuleSet", "AnotherRuleSet"), (SolverFamily::CNF), (SolverName::Minion));
```

#### Adding Rules to RuleSets

Notice that we do not add any rules to the `RuleSet` when we register it.
Instead, the `Rule` contains the names of the RuleSets that it needs to be added to.

At runtime, when we first request the `rules` from a `RuleSet`, it retrieves a list of all the rules that reference it by name from the registry, and stores static references to the rules along with their priorities. 

This is done to allow us to statically initialize `Rule`s and `RuleSet`s in a decentralized way across multiple files and store them in a single registry. Dynamic data structures (like `Vec` or `HashMap`) cannot be initialized at this stage (Rust has no "life before `main()`"), so we have to initialize them lazily at runtime.

![image](https://github.com/conjure-cp/conjure-oxide/assets/64529579/6d63547c-6ba0-4eeb-b6ad-0f6ef46f4c43)

Internally, we would sometimes refer to this lazy initialization as "reversing the arrows".

### Getting RuleSets from the registry

Similarly to `Rule`s, `RuleSet`s may be retrieved using the following functions:

```rust
pub fn get_rule_set_by_name(name: &str) -> Option<&'static RuleSet<'static>>
```

```rust
pub fn get_rule_sets() -> Vec<&'static RuleSet<'static>>
```

## Resolving a final list of `Rule`s

---

Our `rewrite` function takes an `Expression` along with a list of `RuleSet`s to apply:

```rust
pub fn rewrite<'a>(
    expression: &Expression,
    rule_sets: &Vec<&'a RuleSet<'a>>,
) -> Result<Expression, RewriteError>
```

Before we start rewriting the AST, we must first resolve the final list of rules to apply.

This is done via the following steps:
1. Add all given RuleSet's to a set of rulesets
2. Recursively look up all their dependencies by name and add them to the set as well
3. Once we have a final set of `RuleSet`s:

    1. Construct a `HashMap<&Rule, priority>)`. This will hold our final mapping of rules to priorities
    2. Loop over all the rules of every `RuleSet`
    3. If a rule is not yet present in the final `HashMap`, add it and its priority within the `RuleSet`
    4. If it is already present:

        - Compare the order of the current `RuleSet` and the one that the rule originally came from
        - If the new `RuleSet` has a higher order, update the `Rule`'s priority

4. Once all rules have been added to the `HashMap`:

    1. Take all its keys and put them in a vector
    2. Sort it by rule priority
    3. In the case that two rules have the same priority, sort them lexicographically by `name`

In the end, we should have a final deterministically ordered list of rules and a `HashMap` that maps them to their priorities.
Now, we can proceed with rewriting.

### Why all this weirdness?

#### Rule ordering

We want to always have a single deterministic ordering of `Rule`s. This way, for a given set of rules, the `select_rule` strategy would always give the same result. 

Think of it as a multiple choice quiz: if we want to know that the same numbers in the answer sheet actually correspond to the same set of answers, we must make sure that all students get the questions in the same order.

This is why we sort `Rule`s by priority, and then use their name (which is guaranteed to be unique) as a tie breaker.

Normally, one would just construct a vector of `Rule`s and use it as the final ordering, but we cannot do that, because rules are registered in a decentralized way across many files, and when we get them from the rule registry, they are not guaranteed to be in any specific order

#### RuleSet ordering

As part of resolving the list of rules to use, we need to take rules from multiple `RuleSet`s and put these rules and their priorities in a `HashMap`. However, the `RuleSet`s may overlap (i.e. contain the same rules but with different priorities), and we want to make sure that, for a given set of `RuleSet`s, the final rule priorities will always be the same.

Normally, this would not be an issues - entries in the `HashMap` would be added and updated as needed as we loop over the `RuleSet`s. However, since the `RuleSet`s are stored in a decentralised registry and are not guaranteed to come in any particular order (i.e. this order may change every time we recompile the project), we need to ensure that the order in which the entries are added to the `HashMap` (and thus the final rule priorities) doesn't change.

To achieve this, we use the following algorithm:

1. Loop over all given `RuleSet`s
2. Loop over all the rules in a `RuleSet`
    - If the rule is not present in the `HashMap`, add it
    - If the rule is already there:
        - If this `RuleSet` has a higher `order` then the one that the rule came from, update its priority
        - Otherwise, don't change anything

> NOTE: The `order` of a `RuleSet` should not be thought of as a "priority" and does not affect the priorities of the rules in it.
> It only provides a consistent order of operations when resolving the final set of rules