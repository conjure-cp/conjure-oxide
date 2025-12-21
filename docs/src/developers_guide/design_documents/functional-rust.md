## Functional Rust

Consider the following oft-quoted statement about Rust:

*Rust is blazingly fast and memory-efficient: with no runtime or garbage collector. Rust's rich type system and ownership model guarantee memory-safety and thread-safety — enabling you to eliminate many classes of bugs at compile-time.*

Taken specifically from the Rust Foundation’s[^1] page talking about why Rust is a good language to use.

But let us focus on the implications of this statement on Conjure Oxide specifically. The key details that one needs to know here are that, despite arguably being imperative, Rust adopts many functional programming concepts in its design[^2]. This is significant because the Conjure Oxide codebase makes extensive use of these functional programming concepts.

Making use of them allows the codebase to leverage Rust’s type system to eliminate error cases through coding style, making the code more robust. It also makes the code easier to write, because Rust’s safety system enforces function return types. 

### Result

By using the `Result` type, functions can explicitly state success or failure. This also allows functions to call each other linearly in a safe manner[^3].

This enables the code to be written so that functions can do essentially whatever they need to do as side effects, as long as they record a success or failure result. The error type allows error propagation in a more sophisticated way than exit codes, while still being more efficient than exceptions. This structure is used in a lot of places in this codebase, because it allows for functions to be treated uniformly most anywhere[^4], meaning they can be called anywhere by any originator (or caller) function without having to resort to unsafe rust and sticking to the type safety scheme.

This also allows for the use of the `?` operator[^5], which means that errors can be progogated up the call stack lazily. 

Consider the following example, written imperatively in Java-like pseudocode:

```java

public void fnReturnsError(a,b) {
    ...Some Code...
    // might throw error1
    int foo1 = maybeReturnsMyError1(a);
    ...Some More Code...
    // might throw error2
    int foo2 = maybeReturnsMyError2(b);
    ...Wow, when will this code end...
    // might throw error3
    int foo3 = maybeReturnsMyError3(a,b);
}
```

Alternatively, consider a lower-level language like C, which is far more comparable to Rust in its uses and applications, where there are no 'error types' and errors behave like either enums or just integral exit codes.

In a C-like language, a similar example would look like this:

```C
int returnAnExitCode(int a, int b) {
    int exit_code = 0;
    ...Some Code Here Too...
    int exit_code = maybeReturnsMyExitCode1(a);
    ...Some More Code...
    int exit_code = maybeReturnsMyExitCode2(b);
    ...Why, if it isn't yet more code ...
    int exit_code = maybeReturnsMyExitCode3(a,b);

    return exit_code
}
```

This means that, like rust, an error is not an exception getting `thrown', but a value that is being returned. However, unlike rust, they require these exit codes to be set and returned at the end.

This also means that the function calls must be done on some top-level data structure, and that global structure needs to be accessed at the end rather than being able to simply access the top-level data structure through the returned value. 

In this type of application, Rust offers two advantages:

1. The `Return` type, even though it records a success/failure, can be pattern-matched using the `match` construct to access the data structure which the functions are performing side effects on.
2. The aforementioned `?` operator can be used to force the code to return an error type as soon as it is 'raised' -- that is, returned by one of the functions in the call stack. 

This example would look like this in rust-like functional (pseudo)code:

```rust
```

### Side Effects

Throughout the above section, references are made to changing things through 'function side effects'. Let us dive deeper into what this actually means.

Functions are fairly complex, but here is what Alonzo Church has to say about what they do, taken from his paper _The Calculi of Lambda Conversion_:

*A function is a rule of correspondence by which when anything is given (as argument) another thing (the value of the function for that argument) may be obtained. That is, a function is an operation which may be applied on one thing (the argument) to yield another thing (the value of the function).*

In quite an abstract sense, this passage establishes that a Function is simply a mapping from a set of inputs to a set of outputs. 

Now, knowing this, the term _side effect_ also begins to make sense -- any persistent effects of a function which are not in the returned value are side effects. In a general sense, this is things like writing to files, printing and so on. More specifically in conjure oxide, almost all processing is done by way of side effects. While this makes sense even in an imperative context, imperative code can still have some functions chained together in ways that are only possible if the data structure being affected by them is actually returned by them. In rust, specifically when this side-effect-only style of programming is used, programs end up looking quite a bit more concise and readable. 

Now, having all of this knowledge in the back of your head, you will understand why the following things must be kept in mind:

1. Make the greatest effort to treat Rust as a functional language when programming in this (and indeed any) codebase. Not only does this lead to cleaner, more concise and (arguably) more readable code, it actually helps avoid errors and edge cases.
2. Learn to leverage the Rust type and safety system instead of wrestling with by writing code that uses features in the language like `Result<T,E>` and `Option<T>`. This may involve learning where to use these instead of doing things that one cannot do in imperative languages like C. 
3. Rust code is only properly 'safe' if it uses the type system properly, meaning it is a good idea to avoid things like returning null, unwrapping `Result` instances[^6].


---

[^1]: Who are, of course, an entirely unbiased source  
[^2]: Rust’s original implementation language was OCaml, which is functional  
[^3]: Similar to calling functions in dynamically typed languages like Python, but with enforced types
[^4]: This is because, at their core, all functions are essentially of the type (..) -> ReturnType. Making the return type standard allows for all functions to be of similar types.
[^5]: Which immediately propogates the error up through the call stack in rust. 
[^6]: This is what caused the infamous cloudflare outage.