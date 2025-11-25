<!-- TODO: Edit this -->

> *"Omit needless words"*
>
> (c) William Strunk

# Key Points 

## Resources 

See:
- the [Rust doc style conventions](https://github.com/rust-lang/rfcs/blob/master/text/1574-more-api-documentation-conventions.md)

## Do's

Our documentation is inline with the code. The reader is likely to skim it while scrolling through the file and trying to understand what it does. So, our doc strings should:

- Be as brief and clear as possible
- Ideally, fit comfortably on a standard monitor without obscuring the code
- Contain key information about the method / structure, such as:
  - A single sentence explaining its purpose
  - A brief overview of arguments / return types
  - Example snippets for non-trivial methods
- Explain any details that are **not** obvious from the method signature, such as:
  - Details of the method's "contract" that can't be easily encoded in the type system
    > (E.g: *"The input must be a sorted slice of positive integers"*; *"The given expression will be modified in-place"*)
  - Non-trivial situations where the method may `panic!` or cause unintended behaviour
    > (E.g: *"Panics if the connection terminates while the stream is being read"*)
  - Any `unsafe` things a method does
    > (E.g: *"We type-cast the given pointer with `mem::transmute`. This is safe because..."*)
  - Special cases

## Don'ts

Documentation generally should **not**:

- Repeat itself
- Use long, vague, or overly complex sentences
- Re-state things that are obvious from the types / signature of the method
- Explain implementation details
- Explain high-level architectural decisions
  (Consider making a wiki page or opening an RFC issue instead!)

And finally...
Please, don't ask ChatGPT to document your code for you!
I know that writing documentation can be tedious, but you can always:

- Write a one-sentence doc string for now and come back to it later 
- Ask others if you don't quite understand what a method does

# Types and Tests are Documentation

Documentation is great, but we should also use the type system and other rust features to our advantage!

- A lot of things (e.g: error conditions, thread safety, state) can be encoded in the types of arguments / return values.
This is usually better than just `panic!`-ing and adding a doc string to explain why.

- Tests are also a great way to illustrate the behaviour of your code and any special cases - and they also help with catching bugs!

# Example Snippets

For non-trivial methods and user-facing API's, it may be useful to include an example.
Examples should be minimal but complete snippets of code that illustrate a method's behaviour.

If you wrap your example in a code block:

```markdown
```rust ... ```
```

Our CI will even run it and complain if the example does not compile / contains an error!

However, don't feel obliged to include an example for every method!
For simple methods they may not be necessary.

# Examples 

⚠️ Good but a bit wordy:

```rust
/// Checks if the OPTIMIZATIONS environment variable is set to "1".
///
/// # Returns
/// - true if the environment variable is set to "1".
/// - false if the environment variable is not set or set to any other value.
fn optimizations_enabled() -> bool {
    match env::var("OPTIMIZATIONS") {
        Ok(val) => val == "1",
        Err(_) => false, // Assume optimizations are disabled if the environment variable is not set
    }
}
```

✅ Since everything else is obvious from the signature, we can just say:

```rust
/// Checks if the OPTIMIZATIONS environment variable is set to "1"
fn optimizations_enabled() -> bool { ... }
```

⚠️ Not bad, but sounds a bit robotic

```markdown
# Side-Effects
- When the model is rewritten, related data structures such as the symbol table (which tracks variable names and types)
  or other top-level constraints may also be updated to reflect these changes. These updates are applied to the returned model,
  ensuring that all related components stay consistent and aligned with the changes made during the rewrite.
- The function collects statistics about the rewriting process, including the number of rule applications
  and the total runtime of the rewriter. These statistics are then stored in the model's context for
  performance monitoring and analysis.
```

✅ Same idea but shorter

```markdown
# Side-Effects
- Rules can apply side-effects to the model (e.g. adding new constraints or variables).
  The original model is cloned and a modified copy is returned.
- Rule engine statistics (e.g. number of rule applications, run time) are collected and stored in the new model's context.
```


⚠️ A bit too detailed

```markdown
# Parameters
- `expression`: A reference to the [`Expression`] that will be evaluated against the given rules. This is the main
   target for rule transformations and is expected to remain unchanged during the function execution.
- `model`: A reference to the [`Model`] that provides context for rule evaluation, such as constraints and symbols.
  Rules may depend on information in the model to determine if they can be applied.
- `rules`: A vector of references to [`Rule`]s that define the transformations to be applied to the expression.
  Each rule is applied independently, and all applicable rules are collected.
- `stats`: A mutable reference to [`RewriterStats`] used to track statistics about rule application, such as
  the number of attempts and successful applications.
```

✅ Just describing the meaning of arguments will do

(Details of the rewriting process belong on the wiki, and details of underlying types such as `Model` or `Expression` are already documented next to their implementations)

```markdown
- `expression`: A reference to the [`Expression`] to evaluate.
- `model`: A reference to the [`Model`] for access to the symbol table and context.
- `rules`: A vector of references to [`Rule`]s to try.
- `stats`: A mutable reference to [`RewriterStats`] used to track the number of rule applications and other statistics.
```

---

*This section had been taken from the 'Documentation Style' page of the conjure-oxide wiki*