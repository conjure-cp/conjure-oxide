## Rule Sets

While developing rule-based transformations for conjure oxide, it is useful to understand the structure of the rulesets and the *types* of rules that can be used in conjure oxide. Let us first look at how rules actually function, not programmatically, but in an abstract sense. An understanding of (and some experience of) functional programming is incredibly helpful[^1]. Also useful is an understanding of the idea behind graph machines[^2] and an understanding of the difference between function results and side effects[^3].

In conjure oxide, the rules are functions that take in the expression tree and the symbol table as arguments and return a function result[^4], meaning that the original expression tree and symbol table are only modified through side effects of `rule` functions[^5].

There are quite a few advantages to this system, including a few small quality-of-life things such as the ability to write more descriptive errors and to pattern match on function results. The most significant, however, is the ability to express rules as functions that can do whatever they need to do as long as they return a failure or a success. This means that an application failure can be treated as a recoverable result rather than a crash.

Each function is self-contained[^6], meaning that the only things preserved are the initial symbol table and the expression, along with the result being passed along the call stack. The most significant, however, is that this allows for code to be written in a way that enforces that errors are handled. This is quite useful in general because it means that the codebase can leverage Rust’s type system to eliminate edge cases everywhere.

The rule application is able to use this to avoid a situation where the rule engine fails unexpectedly and loses a large amount of work, or worse, a situation where a rule seems to have been applied and is visible in the trace but has not actually affected the tree because it failed[^7].

Now that we know how rules are called, we can move on to how the rules themselves are designed. Broadly, there are two categories that we can divide the rules into: **Transformation Rules** and **Representation Rules**. This is because, despite being applied in the same step in the process, they have different purposes from each other, but all rules of either type share a common goal.

### Representation Rules

Essence (which is the input language of conjure oxide) defines domains that are not present in the type systems of the different solvers’ input languages, meaning they need to be encoded in some way into the input languages. The encoding is done using *representation rules*.

Representation rules implement shared behaviour using a trait[^8]. The representation rules must change Essence variables into the target solver’s input language while preserving all of the relevant information. To actually do this, write a struct that implements the relevant trait. The rule engine can then use this to encode unsupported decision variable types.

### Transformation Rules

After all of the variables in the symbol table have been encoded into the relevant types, the next thing to do is to convert all of the constraints in the conjure oxide AST into a form that has a one-to-one parallel with the input language of the target solver.

This done is using transformation rules, each of which is registered with a 'priority' and can then be used by the rule engine to convert the AST into a format that can be used by the solver adaptor. 

---

[^1]: Take a look at Graham Hutton's *Programming in Haskell (2nd Ed)*, and Miran Lipovača's *Learn You a Haskell for Great Good!* for a lighter read  
[^2]: https://amelia.how/posts/the-gmachine-in-detail.html  
[^3]: Read the section of this text on 'Functional Rust'  
[^4]: In Rust, this is `std::Result<T, E>`  
[^5]: Particularly nice in Rust due to the way it does ownership and error handling  
[^6]: Extra memory is freed when the function exits  
[^7]: Unless `unwrap` is used  
[^8]: See project documentation
